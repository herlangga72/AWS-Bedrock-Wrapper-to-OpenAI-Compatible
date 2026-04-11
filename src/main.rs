mod application;
mod domain;
mod infrastructure;
mod interface;
mod shared;

use aws_config::Region;
use aws_sdk_bedrock::Client as MgmtClient;
use aws_sdk_bedrockruntime::Client as RuntimeClient;
use axum::{
    routing::{get, post},
    Router,
};
use infrastructure::cloudflare::CloudflareClient;
use std::collections::HashMap;
use std::sync::Arc;

use domain::auth::Authentication;
use domain::logging::ClickHouseLogger;
use shared::app_state::AppState;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let region_str = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    let config = aws_config::from_env()
        .region(Region::new(region_str))
        .load()
        .await;

    let api_database =
        std::env::var("DB_API_KEY_LOCATION_SQLITE").unwrap_or_else(|_| "api_keys.db".to_string());

    let runtime_client = RuntimeClient::new(&config);
    let mgmt_client = MgmtClient::new(&config);
    let logger = ClickHouseLogger::new();
    let auth_service =
        Authentication::new(&api_database).expect("Failed to initialize authentication service");

    let _ = auth_service.register_key(
        &std::env::var("DEFAULT_API_KEY").expect("API_KEY must be set"),
        "chat",
    );

    let state = AppState {
        client: runtime_client,
        mgmt_client,
        logger,
        file_cache: Arc::new(arc_swap::ArcSwap::from_pointee(HashMap::new())),
        auth: auth_service,
        cloudflare_client: if let (Ok(account_id), Ok(api_token)) = (
            std::env::var("CLOUDFLARE_ACCOUNT_ID"),
            std::env::var("CLOUDFLARE_API_TOKEN"),
        ) {
            Some(CloudflareClient::builder()
                .account_id(account_id)
                .api_token(api_token)
                .build()
                .expect("Cloudflare client should build"))
        } else {
            None
        },
    };

    // Populate models cache
    let monitor_state = state.clone();
    tokio::spawn(infrastructure::cache::file_cache::run_cache_monitor(monitor_state));

    let app = Router::new()
        .route("/v1/chat/completions", post(interface::chat::chat_with_thinking_handler))
        .route("/v1/models", get(interface::models::models_handler::list_models_handler))
        .route("/v1/embeddings", post(interface::embedding::embedding_handler::handle_embeddings))
        .with_state(state);

    let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("SERVER_PORT").unwrap_or_else(|_| "3001".to_string());
    let addr = format!("{}:{}", host, port);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("🚀 Bedrock Proxy listening on {}", addr);

    axum::serve(listener, app).await.unwrap();
}
