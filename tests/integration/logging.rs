//! ClickHouse logging integration tests
//!
//! Requires: ClickHouse running at CLICKHOUSE_URL
//!
//! Run with docker-compose:
//! ```bash
//! docker-compose up -d clickhouse
//! cargo test --test logging_test -- --nocapture
//! ```

use aws_bedrock_translation_to_openai::domain::logging::{ClickHouseLogger, LogEntry};
use std::time::Duration;
use tokio::time::sleep;

/// Create a ClickHouse logger for testing
fn create_test_logger() -> ClickHouseLogger {
    ClickHouseLogger::new()
}

#[tokio::test]
#[ignore] // Requires ClickHouse - run with: docker-compose up clickhouse
async fn test_clickhouse_log_usage() {
    let logger = create_test_logger();

    // Log a usage entry
    let result = logger.log_usage("test@example.com", "test-model", 100, 200).await;

    // Give ClickHouse time to process
    sleep(Duration::from_millis(500)).await;

    assert!(result.is_ok(), "Logging should succeed: {:?}", result.err());
}

#[tokio::test]
#[ignore]
async fn test_clickhouse_log_multiple_entries() {
    let logger = create_test_logger();

    for i in 0..5 {
        let email = format!("user{}@example.com", i);
        let result = logger.log_usage(&email, "test-model", 50 * i as u32, 100 * i as u32).await;
        assert!(result.is_ok());
    }

    // Give ClickHouse time to process
    sleep(Duration::from_secs(1)).await;
}

#[tokio::test]
#[ignore]
async fn test_clickhouse_query_usage() {
    use clickhouse::Client;

    let client = Client::default()
        .with_url(std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://127.0.0.1:8123".into()))
        .with_user(std::env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "default".into()))
        .with_password(std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_else(|_| "password".into()))
        .with_database(std::env::var("CLICKHOUSE_DB").unwrap_or_else(|_| "default".into()));

    // Create table if not exists
    let create_sql = r#"
        CREATE TABLE IF NOT EXISTS usage_logs (
            timestamp DateTime DEFAULT now(),
            email String,
            model String,
            input_tokens UInt32,
            output_tokens UInt32,
            total_tokens UInt32
        ) ENGINE = MergeTree()
        ORDER BY (timestamp, email)
    "#;

    // Note: This would fail if table doesn't exist and we can't create it
    // In real integration tests, you'd use a pre-created table

    // Query for logs
    let query_sql = "SELECT COUNT(*) FROM usage_logs";
    let mut cursor = client.query(query_sql).fetch().await.unwrap();

    if let Some(row) = cursor.next().await.unwrap() {
        let count: u64 = row.get("count(*)").unwrap();
        println!("Total log entries: {}", count);
    }
}
