//! Logging domain types - Data structures for ClickHouse usage logging

use clickhouse::Row;
use serde::Serialize;

/// Log entry for usage tracking
#[derive(Row, Serialize, Clone)]
pub struct LogEntry {
    #[serde(with = "clickhouse::serde::chrono::datetime")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub model: String,
    pub user_email: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
}
