//! Main entry point for the AWS Bedrock Translation to OpenAI Compatible API

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
use shared::constants::*;

/// Initialize Cloudflare client (optional - returns None if not configured)
fn init_cloudflare_client() -> Option<CloudflareClient> {
    let account_id = std::env::var("CLOUDFLARE_ACCOUNT_ID").ok()?;
    let api_token = std::env::var("CLOUDFLARE_API_TOKEN").ok()?;

    Some(
        CloudflareClient::builder()
            .account_id(account_id)
            .api_token(api_token)
            .build()
            .expect("Cloudflare client should build"),
    )
}

/// Initialize and run the server
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Failed to start server: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    dotenvy::dotenv().ok();

    // Initialize AWS config
    let region_str = std::env::var("AWS_REGION").unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let config = aws_config::from_env()
        .region(Region::new(region_str))
        .load()
        .await;

    // Initialize database path
    let api_database =
        std::env::var("DB_API_KEY_LOCATION_SQLITE").unwrap_or_else(|_| "api_keys.db".to_string());

    // Initialize clients
    let runtime_client = RuntimeClient::new(&config);
    let mgmt_client = MgmtClient::new(&config);

    // Initialize auth service
    let auth_service = Authentication::new(&api_database)
        .map_err(|e| format!("Failed to initialize auth: {e}"))?;

    // Register default API key
    let default_key = std::env::var("DEFAULT_API_KEY").map_err(|_| "DEFAULT_API_KEY must be set")?;
    auth_service
        .register_key(&default_key, "chat")
        .map_err(|e| format!("Failed to register API key: {e}"))?;

    // Initialize logger
    let logger = ClickHouseLogger::new();

    // Initialize Cloudflare client (optional)
    let cloudflare_client = init_cloudflare_client();

    let state = AppState {
        client: runtime_client,
        mgmt_client,
        logger,
        file_cache: Arc::new(arc_swap::ArcSwap::from_pointee(HashMap::new())),
        auth: auth_service,
        cloudflare_client,
    };

    // Spawn cache monitor
    let monitor_state = state.clone();
    tokio::spawn(infrastructure::cache::file_cache::run_cache_monitor(
        monitor_state,
    ));

    let app = Router::new()
        .route(
            "/v1/chat/completions",
            post(interface::chat::chat_with_thinking_handler),
        )
        .route(
            "/v1/models",
            get(interface::models::models_handler::list_models_handler),
        )
        .route(
            "/v1/embeddings",
            post(interface::embedding::embedding_handler::handle_embeddings),
        )
        .with_state(state);

    let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let port = std::env::var("SERVER_PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string());
    let addr = format!("{}:{}", host, port);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind to {}: {e}", addr))?;

    println!("🚀 Bedrock Proxy listening on {}", addr);

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server error: {e}"))?;

    Ok(())
}