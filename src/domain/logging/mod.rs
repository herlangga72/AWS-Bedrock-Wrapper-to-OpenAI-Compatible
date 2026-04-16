//! Logging domain - Usage logging to ClickHouse

pub mod logger;
pub mod types;

pub use logger::ClickHouseLogger;
pub use types::LogEntry;
