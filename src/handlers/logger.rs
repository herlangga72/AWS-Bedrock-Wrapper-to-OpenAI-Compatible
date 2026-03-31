use clickhouse::{Client, Row};
use serde::Serialize;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{interval, MissedTickBehavior};

#[derive(Row, Serialize)]
pub struct LogEntry {
    #[serde(with = "clickhouse::serde::chrono::datetime")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub model: String,
    pub user_email: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Clone)]
pub struct ClickHouseLogger {
    tx: mpsc::Sender<LogEntry>,
}

impl ClickHouseLogger {
    pub fn new() -> Self {
        let url = std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://127.0.0.1:8123".to_string());
        let user = std::env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "default".to_string());
        let pass = std::env::var("CLICKHOUSE_PASSWORD").expect("CLICKHOUSE_PASSWORD must be set");
        let db = std::env::var("CLICKHOUSE_DB").unwrap_or_else(|_| "default".to_string());

        let (tx, mut rx) = mpsc::channel::<LogEntry>(4096);
        
        // Configure the client once
        let client = Client::default()
            .with_url(url)
            .with_user(user)
            .with_password(pass)
            .with_database(db)
            .with_compression(clickhouse::Compression::Lz4);

        tokio::spawn(async move {
            let mut batch = Vec::with_capacity(5000);
            let mut ticker = interval(Duration::from_secs(2));
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

            loop {
                tokio::select! {
                    Some(entry) = rx.recv() => {
                        batch.push(entry);
                        if batch.len() >= 5000 {
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
        // Use the turbofish <LogEntry> to satisfy the compiler
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