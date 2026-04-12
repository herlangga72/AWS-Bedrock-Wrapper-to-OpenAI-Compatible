//! Embedding handler using AWS Nova 2

use crate::domain::embedding::{
    NovaRequest, NovaResponse, OpenAiEmbeddingData, OpenAiEmbeddingRequest,
    OpenAiEmbeddingResponse, OpenAiUsage,
};
use crate::shared::app_state::AppState;

use aws_sdk_bedrockruntime::primitives::Blob;
use axum::{extract::State, http::StatusCode, Json};

const NOVA_MODEL_ID: &str = "amazon.nova-2-multimodal-embeddings-v1:0";

pub async fn handle_embeddings(
    State(state): State<AppState>,
    Json(payload): Json<OpenAiEmbeddingRequest>,
) -> Result<Json<OpenAiEmbeddingResponse>, StatusCode> {
    let input_text = payload.input.first().ok_or_else(|| {
        tracing::error!("Empty input array received");
        StatusCode::BAD_REQUEST
    })?;

    let nova_payload = NovaRequest::new(input_text);

    let request_body = serde_json::to_vec(&nova_payload).map_err(|e| {
        tracing::error!("Serialization error: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let res = state
        .client
        .invoke_model()
        .model_id(NOVA_MODEL_ID)
        .content_type("application/json")
        .accept("application/json")
        .body(Blob::new(request_body))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("AWS Bedrock Dispatch Error: {e:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let out: NovaResponse = serde_json::from_slice(res.body().as_ref()).map_err(|e| {
        tracing::error!("Failed to parse Nova response: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let embedding_entry = out.embeddings.into_iter().next().ok_or_else(|| {
        tracing::error!("Nova returned empty embedding list");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let response = OpenAiEmbeddingResponse {
        object: "list",
        model: NOVA_MODEL_ID,
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
