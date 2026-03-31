use reqwest::Client;
use serde::Serialize;
use chrono::Utc;
use tokio::sync::mpsc;
use std::time::Duration;

#[derive(Serialize)]
struct LogEntry {
    timestamp: String,
    model: String,
    user_email: String,
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Clone)]
pub struct ClickHouseLogger {
    tx: mpsc::UnboundedSender<LogEntry>,
}

impl ClickHouseLogger {
    pub fn new(addr: &str) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<LogEntry>();
        let client = Client::builder()
            .tcp_keepalive(Duration::from_secs(60))
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build client");

        let url = format!("{}/?query=INSERT+INTO+chat_logs+FORMAT+JSONEachRow", addr);
        tokio::spawn(async move {
            let mut batch = Vec::with_capacity(100);
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                tokio::select! {
                    // Receive a log
                    Some(entry) = rx.recv() => {
                        batch.push(entry);
                        if batch.len() >= 100 {
                            Self::flush(&client, &url, &mut batch).await;
                        }
                    }
                    // Flush periodically even if batch isn't full
                    _ = interval.tick() => {
                        if !batch.is_empty() {
                            Self::flush(&client, &url, &mut batch).await;
                        }
                    }
                }
            }
        });

        Self { tx }
    }

    async fn flush(client: &Client, url: &str, batch: &mut Vec<LogEntry>) {
        let mut body = String::new();
        for entry in batch.drain(..) {
            if let Ok(json) = serde_json::to_string(&entry) {
                body.push_str(&json);
                body.push('\n');
            }
        }

        if let Err(e) = client.post(url).body(body).send().await {
            eprintln!("ClickHouse Batch Insert Failed: {:?}", e);
        }
    }

    pub fn log_usage(&self, user_email: &str, model: &str, input_tokens: u32, output_tokens: u32) {
        let entry = LogEntry {
            timestamp: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            model: model.to_string(),
            user_email: user_email.to_string(),
            input_tokens,
            output_tokens,
        };
        let _ = self.tx.send(entry);
    }
}