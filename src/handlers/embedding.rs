use crate::AppState;
use aws_sdk_bedrockruntime::primitives::Blob;
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
// --- 1. Request Schemas ---

#[derive(Deserialize)]
pub struct OpenAiEmbeddingRequest {
    pub input: Vec<String>,
}

// --- 2. Nova 2 Schemas ---

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
    value: Cow<'a, str>,
}

// --- 3. AWS Response Schemas ---

#[derive(Deserialize)]
struct NovaResponse {
    embeddings: Vec<NovaEmbeddingEntry>,
    #[serde(rename = "inputTextTokenCount")]
    token_count: u64,
}

#[derive(Deserialize)]
struct NovaEmbeddingEntry {
    embedding: Vec<f32>,
}

// --- 4. OpenAI Result Schemas ---

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

// --- 5. Handler ---

pub async fn handle_embeddings(
    State(state): State<AppState>,
    payload: Json<OpenAiEmbeddingRequest>,
) -> Result<Json<OpenAiEmbeddingResponse>, StatusCode> {
    // OPTIMIZATION:
    // 1. Extract the FIRST string instead of clearing everything.
    //    Unlike clearing the whole vector, this doesn't require a mutable borrow
    //    of the payload.input for later operations.
    let input_text = payload.input.first().ok_or_else(|| {
        tracing::error!("Empty input array received");
        StatusCode::BAD_REQUEST
    })?;

    // 2. CRITICAL FIX: Clone the string now while the borrow is valid.
    //    This creates a String value that is independent of the payload.
    //    This allows us to discard the Vec reference completely.
    let input_string_clone = input_text.clone();

    // 3. Build Nova Payload
    let nova_payload = NovaRequest {
        task_type: "SINGLE_EMBEDDING",
        params: NovaParams {
            embedding_purpose: "GENERIC_INDEX",
            dimension: 3072,
            text: NovaText {
                truncation_mode: "END",
                // Now we use the clone instead of Cow::Borrowed(input_text)
                value: Cow::Owned(input_string_clone),
            },
        },
    };

    // 4. Serialize
    let request_body = serde_json::to_vec(&nova_payload).map_err(|e| {
        tracing::error!("Serialization error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 5. Invoke AWS SDK
    let res = state
        .client
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

    // 6. Parse AWS Response
    let out: NovaResponse = serde_json::from_slice(res.body().as_ref()).map_err(|e| {
        tracing::error!("Failed to parse Nova response: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let embedding_entry = out.embeddings.into_iter().next().ok_or_else(|| {
        tracing::error!("Nova returned empty embedding list");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 7. Response
    let response = OpenAiEmbeddingResponse {
        object: "list",
        model: "amazon.nova-2-multimodal-embeddings-v1:0",
        data: vec![OpenAiEmbeddingData {
            object: "embedding",
            embedding: embedding_entry.embedding,
            index: 0,
        }],
        usage: OpenAiUsage {
            prompt_tokens: out.token_count,
            total_tokens: out.token_count,
        },
    };

    Ok(Json(response))
}
