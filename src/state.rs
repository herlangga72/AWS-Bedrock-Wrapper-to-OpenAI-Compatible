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

/// Test helpers — only compiled when running `cargo test`.
#[cfg(test)]
impl AppState {
    /// Build an `AppState` suitable for unit/integration tests.
    ///
    /// The AWS SDK clients are constructed with a dummy region and no
    /// credentials. They will never be called in tests that exercise the auth
    /// layer, so no real AWS connection is made.
    pub fn for_testing(api_key: &str) -> Self {
        use crate::provider::{bedrock::test_provider, registry::ProviderRegistry};

        let registry = ProviderRegistry::new(test_provider());
        let logger = crate::logging::ClickHouseLogger::new("http://127.0.0.1:0");

        AppState::new(api_key.to_string(), registry, logger)
    }
}
