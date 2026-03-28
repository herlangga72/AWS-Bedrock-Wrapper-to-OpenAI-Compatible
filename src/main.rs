mod config;
mod handlers;
mod logging;
mod middleware;
mod provider;
mod router;
mod state;
mod types;

use aws_config::Region;
use aws_sdk_bedrock::Client as MgmtClient;
use aws_sdk_bedrockruntime::Client as RuntimeClient;

use config::Config;
use logging::ClickHouseLogger;
use provider::{bedrock::BedrockProvider, registry::ProviderRegistry};
use state::AppState;

#[tokio::main]
async fn main() {
    let cfg = Config::from_env();

    let aws_cfg = aws_config::from_env()
        .region(Region::new(cfg.aws_region.clone()))
        .load()
        .await;

    let registry = ProviderRegistry::new(BedrockProvider::new(
        RuntimeClient::new(&aws_cfg),
        MgmtClient::new(&aws_cfg),
    ));

    let addr = cfg.addr();
    let state = AppState::new(
        cfg.api_key,
        registry,
        ClickHouseLogger::new(&cfg.clickhouse_url),
    );

    let app = router::build_router(state);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("🚀 Bedrock Proxy listening on {addr}");
    axum::serve(listener, app).await.unwrap();
}
