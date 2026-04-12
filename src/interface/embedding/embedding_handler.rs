//! Embedding handler using AWS Nova 2

use crate::domain::embedding::{
    NovaRequest, NovaResponse, OpenAiEmbeddingData, OpenAiEmbeddingRequest,
    OpenAiEmbeddingResponse, OpenAiUsage,
};
use crate::shared::app_state::AppState;

use aws_sdk_bedrockruntime::primitives::Blob;
use axum::{extract::State, http::StatusCode, Json, response::{IntoResponse, Response}};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use serde::Serialize;

const NOVA_MODEL_ID: &str = "amazon.nova-2-multimodal-embeddings-v1:0";

/// Error response structure
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    code: u16,
}

fn make_error_response(status: StatusCode, error: &str) -> Response {
    let err = ErrorResponse {
        error: error.to_string(),
        code: status.as_u16(),
    };
    let json = serde_json::to_string(&err).unwrap_or_else(|_| r#"{"error":"Internal error"}"#.to_string());
    (status, [("content-type", "application/json")], json).into_response()
}

pub async fn handle_embeddings(
    State(state): State<AppState>,
    auth: Option<TypedHeader<Authorization<Bearer>>>,
    Json(payload): Json<OpenAiEmbeddingRequest>,
) -> Response {
    // Validate authentication
    let _user_email = match auth {
        Some(TypedHeader(Authorization(bearer))) => {
            match state.auth.authenticate(bearer.token()) {
                Ok(email) => email,
                Err(_) => return make_error_response(StatusCode::UNAUTHORIZED, "Invalid API Key"),
            }
        },
        None => return make_error_response(StatusCode::UNAUTHORIZED, "Missing API Key"),
    };

    // Validate input
    if payload.input.is_empty() {
        tracing::error!("Empty input array received");
        return make_error_response(StatusCode::BAD_REQUEST, "input array cannot be empty");
    }

    let input_text = match payload.input.first() {
        Some(text) => text,
        None => return make_error_response(StatusCode::BAD_REQUEST, "input array cannot be empty"),
    };

    let nova_payload = NovaRequest::new(input_text);

    let request_body = match serde_json::to_vec(&nova_payload) {
        Ok(body) => body,
        Err(e) => {
            tracing::error!("Serialization error: {e}");
            return make_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to serialize request");
        }
    };

    let res = match state
        .client
        .invoke_model()
        .model_id(NOVA_MODEL_ID)
        .content_type("application/json")
        .accept("application/json")
        .body(Blob::new(request_body))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("AWS Bedrock Dispatch Error: {e:?}");
            return make_error_response(StatusCode::BAD_GATEWAY, "Bedrock API error");
        }
    };

    let out: NovaResponse = match serde_json::from_slice(res.body().as_ref()) {
        Ok(o) => o,
        Err(e) => {
            tracing::error!("Failed to parse Nova response: {e}");
            return make_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to parse response");
        }
    };

    let embedding_entry = match out.embeddings.into_iter().next() {
        Some(e) => e,
        None => {
            tracing::error!("Nova returned empty embedding list");
            return make_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Empty embedding response");
        }
    };

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

    match serde_json::to_string(&response) {
        Ok(json) => (StatusCode::OK, [("content-type", "application/json")], json).into_response(),
        Err(_) => make_error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to serialize response"),
    }
}