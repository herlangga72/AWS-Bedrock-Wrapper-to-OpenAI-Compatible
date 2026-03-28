use axum::{
    routing::{get, post},
    Router,
};

use crate::handlers;
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/v1/chat/completions", post(handlers::chat::chat_handler))
        .route("/v1/models", get(handlers::models::list_models_handler))
        .with_state(state)
}
