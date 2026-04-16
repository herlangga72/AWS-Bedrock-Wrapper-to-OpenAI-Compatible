//! OpenAI-compatible completions handler - routes to appropriate chat handler
//!
//! Entry point for `/v1/chat/completions` and `/openai/v1/chat/completions`

use crate::interface::openai::chat::thinking_handler::chat_with_thinking_handler;
use crate::domain::chat::ChatRequest;
use crate::shared::app_state::AppState;

use axum::{
    extract::State,
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};

/// Main chat completions handler for OpenAI-compatible requests
/// Routes to thinking handler which handles reasoning models as well
pub async fn openai_chat_handler(
    State(state): State<AppState>,
    auth: Option<TypedHeader<Authorization<Bearer>>>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    // Delegate to thinking handler which handles all routing (thinking, reasoning, standard)
    chat_with_thinking_handler(State(state), auth, headers, Json(req)).await
}