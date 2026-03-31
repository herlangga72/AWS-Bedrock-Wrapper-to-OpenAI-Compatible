use aws_sdk_bedrockruntime::primitives::Blob;
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use crate::AppState;

// --- 1. Schemas (Grouped for zero-cost serialization) ---

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
    value: &'a str,
}

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

#[derive(Deserialize)]
pub struct OpenAiEmbeddingRequest {
    pub input: Vec<String>,
}

pub async fn handle_embeddings(
    State(state): State<AppState>,
    Json(payload): Json<OpenAiEmbeddingRequest>,
) -> Result<Json<OpenAiEmbeddingResponse>, StatusCode> {
    
    let input_text = payload.input.get(0).ok_or(StatusCode::BAD_REQUEST)?;

    let mut buffer = Vec::with_capacity(1024); 
    serde_json::to_writer(&mut buffer, &NovaRequest {
        task_type: "SINGLE_EMBEDDING",
        params: NovaParams {
            embedding_purpose: "GENERIC_INDEX",
            dimension: 3072,
            text: NovaText {
                truncation_mode: "END",
                value: input_text,
            },
        },
    }).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = state.client
        .invoke_model()
        .model_id("amazon.nova-2-multimodal-embeddings-v1:0")
        .content_type("application/json")
        .body(Blob::new(buffer))
        .send()
        .await
        .map_err(|e| {
            tracing::error!(err = %e, "Bedrock dispatch failed");
            StatusCode::BAD_GATEWAY
        })?;

    let mut out: NovaResponse = serde_json::from_slice(response.body().as_ref())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if out.embeddings.is_empty() {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let embedding_vec = out.embeddings.swap_remove(0).embedding;

    Ok(Json(OpenAiEmbeddingResponse {
        object: "list",
        model: "amazon.nova-2-multimodal-embeddings-v1:0",
        data: vec![OpenAiEmbeddingData {
            object: "embedding",
            embedding: embedding_vec,
            index: 0,
        }],
        usage: OpenAiUsage {
            prompt_tokens: out.token_count,
            total_tokens: out.token_count,
        },
    }))
}