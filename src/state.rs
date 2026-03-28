use std::sync::Arc;

use crate::logging::ClickHouseLogger;
use crate::provider::registry::ProviderRegistry;
use crate::types::compactor::{MessageCompactor, NoopCompactor};

#[derive(Clone)]
pub struct AppState {
    pub api_key: String,
    pub registry: ProviderRegistry,
    pub logger: ClickHouseLogger,
    /// Swap this out at startup to enable real token compaction.
    pub compactor: Arc<dyn MessageCompactor>,
}

impl AppState {
    pub fn new(api_key: String, registry: ProviderRegistry, logger: ClickHouseLogger) -> Self {
        Self {
            api_key,
            registry,
            logger,
            compactor: Arc::new(NoopCompactor),
        }
    }
}
