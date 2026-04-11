//! Application state shared between handlers

use arc_swap::ArcSwap;
use aws_sdk_bedrock::Client as MgmtClient;
use aws_sdk_bedrockruntime::Client as RuntimeClient;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::auth::Authentication;
use crate::domain::logging::ClickHouseLogger;
use crate::infrastructure::cloudflare::CloudflareClient;

/// Application state shared across all request handlers
#[derive(Clone)]
pub struct AppState {
    pub client: RuntimeClient,
    pub mgmt_client: MgmtClient,
    pub logger: ClickHouseLogger,
    pub file_cache: Arc<ArcSwap<HashMap<String, Bytes>>>,
    pub auth: Authentication,
    pub cloudflare_client: Option<CloudflareClient>,
}
