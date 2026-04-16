//! ClickHouse logger - Batched async usage logging

use crate::domain::logging::types::LogEntry;
use crate::shared::constants::*;
use clickhouse::Client;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{interval, MissedTickBehavior};

/// ClickHouse logger for batched usage logging
#[derive(Clone)]
pub struct ClickHouseLogger {
    tx: mpsc::Sender<LogEntry>,
}

impl ClickHouseLogger {
    pub fn new() -> Self {
        let url = std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| CLICKHOUSE_URL.to_string());
        let user = std::env::var("CLICKHOUSE_USER").unwrap_or_else(|_| CLICKHOUSE_USER.to_string());
        let pass =
            std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_else(|_| CLICKHOUSE_PASSWORD.to_string());
        let db = std::env::var("CLICKHOUSE_DB").unwrap_or_else(|_| CLICKHOUSE_DB.to_string());

        let (tx, mut rx) = mpsc::channel::<LogEntry>(CLICKHOUSE_BATCH_SIZE);

        let client = Client::default()
            .with_url(url)
            .with_user(user)
            .with_password(pass)
            .with_database(db)
            .with_compression(clickhouse::Compression::Lz4);

        tokio::spawn(async move {
            let mut batch = Vec::with_capacity(CLICKHOUSE_BATCH_SIZE);
            let mut ticker = interval(Duration::from_secs(CLICKHOUSE_FLUSH_INTERVAL_SECS));
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

            loop {
                tokio::select! {
                    Some(entry) = rx.recv() => {
                        batch.push(entry);
                        if batch.len() >= CLICKHOUSE_BATCH_SIZE {
                            Self::flush(&client, &mut batch).await;
                        }
                    }
                    _ = ticker.tick() => {
                        if !batch.is_empty() {
                            Self::flush(&client, &mut batch).await;
                        }
                    }
                    else => break,
                }
            }
        });

        Self { tx }
    }

    async fn flush(client: &Client, batch: &mut Vec<LogEntry>) {
        let mut inserter = match client.insert::<LogEntry>("chat_logs").await {
            Ok(ins) => ins,
            Err(e) => {
                eprintln!("[ClickHouse] Connection failed: {:?}", e);
                batch.clear();
                return;
            }
        };

        for entry in batch.drain(..) {
            if let Err(e) = inserter.write(&entry).await {
                eprintln!("[ClickHouse] Write failed: {:?}", e);
                break;
            }
        }

        if let Err(e) = inserter.end().await {
            eprintln!("[ClickHouse] Commit failed: {:?}", e);
        }
    }

    /// Log a usage event (fire-and-forget, drops if channel full)
    pub fn log_usage(&self, email: &str, model: &str, input: u32, output: u32) {
        let entry = LogEntry {
            timestamp: chrono::Utc::now(),
            model: model.to_string(),
            user_email: email.to_string(),
            input_tokens: input,
            output_tokens: output,
        };

        let _ = self.tx.try_send(entry);
    }
}

impl Default for ClickHouseLogger {
    fn default() -> Self {
        Self::new()
    }
}
