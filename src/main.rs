mod handlers;
mod models;

use aws_config::Region;
use aws_sdk_bedrock::Client as MgmtClient;
use aws_sdk_bedrockruntime::{Client as RuntimeClient};
use axum::{
    routing::{post, get},
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
    // 1. Force AWS region to us-east-1
    let region = Region::new("us-east-1");
    let config = aws_config::from_env().region(region).load().await;

    // 2. Create Clients
    let runtime_client = RuntimeClient::new(&config);
    let mgmt_client = MgmtClient::new(&config);

    // 3. Initialize Logger
    let logger = ClickHouseLogger::new(""); 

    // 4. Setup AppState
    let state = AppState {
        client: runtime_client,
        mgmt_client: mgmt_client,
        api_key: std::env::var("API_KEY").unwrap_or_else(|_| "sk-test-key".to_string()),
        logger,
    };

    // 5. Build Router
    let app = Router::new()
        .route("/v1/chat/completions", post(handlers::chat::chat_handler))
        .route("/v1/models", get(handlers::::list_models_handler))
        .with_state(state);

    // 6. Start Server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    println!("🚀 Server listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
