use aws_sdk_bedrockruntime::primitives::Blob;
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use crate::AppState; // Ensure your AppState has the Bedrock client

// --- 1. Request Schemas (Inbound from Client) ---

#[derive(Deserialize)]
pub struct OpenAiEmbeddingRequest {
    /// Supports a single string or an array of strings
    pub input: Vec<String>,
}

// --- 2. Nova 2 Schemas (Outbound to AWS) ---

#[derive(Serialize)]
struct NovaRequest<'a> {
    #[serde(rename = "taskType")]
    task_type: &'static str,
    #[serde(rename = "singleEmbeddingParams")]
    params: NovaParams<'a>,
}

#[derive(Serialize)]
struct NovaParams<'a> {
    #[serde(rename = "embeddingPurpose")]
    embedding_purpose: &'static str,
    #[serde(rename = "embeddingDimension")]
    dimension: u32,
    text: NovaText<'a>,
}

#[derive(Serialize)]
struct NovaText<'a> {
    #[serde(rename = "truncationMode")]
    truncation_mode: &'static str,
    value: &'a str, // Borrowed from the request to avoid allocation
}

// --- 3. Nova 2 Response Schemas (Inbound from AWS) ---

#[derive(Deserialize)]
struct NovaResponse {
    embeddings: Vec<NovaEmbeddingEntry>,
    #[serde(rename = "inputTextTokenCount")]
    token_count: u64,
}

#[derive(Deserialize)]
struct NovaEmbeddingEntry {
    embedding: Vec<f32>, // Efficiently stores the numbers directly
}

// --- 4. OpenAI-Compatible Response Schemas (Outbound to Client) ---

#[derive(Serialize)]
pub struct OpenAiEmbeddingResponse {
    pub object: &'static str,
    pub data: Vec<OpenAiEmbeddingData>,
    pub model: &'static str,
    pub usage: OpenAiUsage,
}

#[derive(Serialize)]
pub struct OpenAiEmbeddingData {
    pub object: &'static str,
    pub embedding: Vec<f32>,
    pub index: usize,
}

#[derive(Serialize)]
pub struct OpenAiUsage {
    pub prompt_tokens: u64,
    pub total_tokens: u64,
}

// --- 5. Optimized Handler ---

pub async fn handle_embeddings(
    State(state): State<AppState>,
    Json(payload): Json<OpenAiEmbeddingRequest>,
) -> Result<Json<OpenAiEmbeddingResponse>, StatusCode> {
    
    // Extract first input string slice without cloning
    let input_text = payload.input.first().ok_or_else(|| {
        tracing::error!("Empty input array received");
        StatusCode::BAD_REQUEST
    })?;

    // Prepare Nova payload - uses references to avoid new string allocations
    let nova_payload = NovaRequest {
        task_type: "SINGLE_EMBEDDING",
        params: NovaParams {
            embedding_purpose: "GENERIC_INDEX",
            dimension: 3072,
            text: NovaText {
                truncation_mode: "END",
                value: input_text,
            },
        },
    };

    // Serialize to bytes directly
    let request_body = serde_json::to_vec(&nova_payload).map_err(|e| {
        tracing::error!("Serialization error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Invoke Bedrock via AWS SDK
    let res = state.client
        .invoke_model()
        .model_id("amazon.nova-2-multimodal-embeddings-v1:0")
        .content_type("application/json")
        .accept("application/json")
        .body(Blob::new(request_body))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("AWS Bedrock Dispatch Error: {e:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Parse the response into our typed struct (fastest deserialization)
    let out: NovaResponse = serde_json::from_slice(res.body().as_ref()).map_err(|e| {
        tracing::error!("Failed to parse Nova response: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Take ownership of the vector (moving it, not cloning it)
    let embedding_entry = out.embeddings.into_iter().next().ok_or_else(|| {
        tracing::error!("Nova returned empty embedding list");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Construct the final typed response
    let response = OpenAiEmbeddingResponse {
        object: "list",
        model: "amazon.nova-2-multimodal-embeddings-v1:0",
        data: vec![OpenAiEmbeddingData {
            object: "embedding",
            embedding: embedding_entry.embedding, // Moves the Vec<f32>
            index: 0,
        }],
        usage: OpenAiUsage {
            prompt_tokens: out.token_count,
            total_tokens: out.token_count,
        },
    };

    Ok(Json(response))
}