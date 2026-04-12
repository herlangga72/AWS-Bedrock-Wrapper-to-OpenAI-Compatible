//! Shared logging utilities

use std::sync::Arc;

use crate::domain::logging::ClickHouseLogger;

/// Spawn an async logging task to record usage metrics
pub fn spawn_log(logger: Arc<ClickHouseLogger>, email: String, model: String, p: u32, c: u32) {
    tokio::spawn(async move {
        let _ = logger.log_usage(&email, &model, p, c);
    });
}
