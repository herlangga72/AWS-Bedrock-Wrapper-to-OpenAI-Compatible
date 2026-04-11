//! Models listing handler

use crate::shared::app_state::AppState;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
};

pub async fn list_models_handler(State(state): State<AppState>) -> impl IntoResponse {
    let cache = state.file_cache.load();

    match cache.get("bedrock_models") {
        Some(bytes) => {
            ([(header::CONTENT_TYPE, "application/json")], bytes.clone()).into_response()
        }
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
