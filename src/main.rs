mod handlers;
mod models;

use arc_swap::ArcSwap;
use aws_config::Region;
use aws_sdk_bedrock::Client as MgmtClient;
use aws_sdk_bedrockruntime::Client as RuntimeClient;
use axum::{
    routing::{get, post},
    Router,
};
use bytes::Bytes;
use handlers::{auth::Authentication, logger::ClickHouseLogger};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub client: RuntimeClient,
    pub mgmt_client: MgmtClient,
    pub logger: ClickHouseLogger,
    pub file_cache: Arc<ArcSwap<HashMap<String, Bytes>>>,
    pub auth: handlers::auth::Authentication,
}

#[tokio::main]
async fn main() {
    // Load .env file if it exists
    dotenvy::dotenv().ok();

    // 1. AWS Configuration from ENV
    let region_str = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    let config = aws_config::from_env()
        .region(Region::new(region_str))
        .load()
        .await;

    let api_database =
        std::env::var("DB_API_KEY_LOCATION_SQLITE").unwrap_or_else(|_| "api_keys.db".to_string());

    // 2. Create Clients
    let runtime_client = RuntimeClient::new(&config);
    let mgmt_client = MgmtClient::new(&config);
    let logger = ClickHouseLogger::new();
    let auth_service =
        Authentication::new(&api_database).expect("Failed to initialize authentication service");

    let _ = auth_service.register_key(
        &std::env::var("DEFAULT_API_KEY").expect("API_KEY must be set"),
        "chat",
    );

    // 4. Setup AppState
    let state = AppState {
        client: runtime_client,
        mgmt_client,
        logger,
        file_cache: Arc::new(ArcSwap::from_pointee(HashMap::new())),
        auth: auth_service,
    };

    // let Models Populate
    let monitor_state = state.clone();
    tokio::spawn(crate::handlers::models::run_cache_monitor(monitor_state));

    // 5. Build Router
    let app = Router::new()
        .route("/v1/chat/completions", post(handlers::chat::chat_handler))
        .route("/v1/models", get(handlers::models::list_models_handler))
        .route(
            "/v1/embeddings",
            post(handlers::embedding::handle_embeddings),
        )
        .with_state(state);

    // 6. Start Server
    let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("SERVER_PORT").unwrap_or_else(|_| "3001".to_string());
    let addr = format!("{}:{}", host, port);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("🚀 Bedrock Proxy listening on {}", addr);

    axum::serve(listener, app).await.unwrap();
}
