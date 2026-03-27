mod handlers;
mod models;

use aws_config::Region;
use aws_sdk_bedrock::Client as MgmtClient;
use aws_sdk_bedrockruntime::Client as RuntimeClient;
use axum::{
    routing::{get, post},
    Router,
};
use handlers::logger::ClickHouseLogger;

#[derive(Clone)]
pub struct AppState {
    pub client: RuntimeClient,
    pub mgmt_client: MgmtClient,
    pub api_key: String,
    pub logger: ClickHouseLogger,
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

    // 2. Create Clients
    let runtime_client = RuntimeClient::new(&config);
    let mgmt_client = MgmtClient::new(&config);

    // 3. Initialize Logger (ClickHouse URL from ENV)
    let clickhouse_url = std::env::var("CLICKHOUSE_URL").expect("CLICKHOUSE_URL must be set");
    
    // FIX: Pass as a reference (&str) instead of owned String
    let logger = ClickHouseLogger::new(&clickhouse_url); 

    // 4. Setup AppState
    let state = AppState {
        client: runtime_client,
        mgmt_client,
        api_key: std::env::var("API_KEY").expect("API_KEY must be set"),
        logger,
    };

    // 5. Build Router
    let app = Router::new()
        .route("/v1/chat/completions", post(handlers::chat::chat_handler))
        .route("/v1/models", get(handlers::models::list_models_handler))
        .with_state(state);

    // 6. Start Server
    let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("SERVER_PORT").unwrap_or_else(|_| "3001".to_string());
    let addr = format!("{}:{}", host, port);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("🚀 Bedrock Proxy listening on {}", addr);
    
    axum::serve(listener, app).await.unwrap();
}