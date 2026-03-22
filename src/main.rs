mod models;
mod chat;

use axum::{
    routing::{get, post},
    Router,
};
use std::env;
use tower_http::cors::{Any, CorsLayer};
use chat::{chat_handler, AppState};
use models::list_models;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;

    let state = AppState {
        client: aws_sdk_bedrockruntime::Client::new(&config),
        mgmt_client: aws_sdk_bedrock::Client::new(&config),
        api_key: env::var("API_KEY").expect("API_KEY must be set"),
    };

    let app = Router::new()
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_handler))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();

    tracing::info!("Server running on 0.0.0.0:3001");

    axum::serve(listener, app).await.unwrap();
}
