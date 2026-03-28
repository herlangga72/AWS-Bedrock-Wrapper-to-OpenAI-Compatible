use chrono::Utc;
use reqwest::Client;
use serde::Serialize;

#[derive(Clone)]
pub struct ClickHouseLogger {
    url: String,
    client: Client,
}

#[derive(Serialize)]
struct LogEntry<'a> {
    timestamp: String,
    model: &'a str,
    user_email: &'a str,
    input_tokens: u32,
    output_tokens: u32,
}

impl ClickHouseLogger {
    pub fn new(addr: &str) -> Self {
        Self {
            url: format!("{}/?query=INSERT+INTO+chat_logs+FORMAT+JSONEachRow", addr),
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("Failed to build reqwest::Client for ClickHouseLogger"),
        }
    }

    /// Fire-and-forget: spawns a background task so the caller is never blocked.
    pub fn log_usage(&self, user_email: &str, model: &str, input_tokens: u32, output_tokens: u32) {
        let client = self.client.clone();
        let url = self.url.clone();
        let user_email = user_email.to_string();
        let model = model.to_string();

        tokio::spawn(async move {
            let entry = LogEntry {
                timestamp: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                model: &model,
                user_email: &user_email,
                input_tokens,
                output_tokens,
            };

            let body = match serde_json::to_string(&entry) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Failed to serialize ClickHouse log entry: {e}");
                    return;
                }
            };

            match client.post(&url).body(body).send().await {
                Ok(resp) if !resp.status().is_success() => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    eprintln!("ClickHouse HTTP Error: {status} - {text}");
                }
                Err(e) => eprintln!("ClickHouse connection error: {e:?}"),
                _ => {}
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn log_usage_does_not_panic_with_unreachable_url() {
        // The background task will fail to connect, but log_usage itself must
        // return instantly without panicking. We wait briefly to let the
        // spawned task run through its error path.
        let logger = ClickHouseLogger::new("http://127.0.0.1:0");
        logger.log_usage("user@example.com", "bedrock/claude", 100, 50);
        // Give the spawned task time to complete (it will hit a connection error).
        sleep(Duration::from_millis(200)).await;
        // If we reach here without panicking the error path is handled correctly.
    }

    #[tokio::test]
    async fn log_usage_accepts_zero_token_counts() {
        let logger = ClickHouseLogger::new("http://127.0.0.1:0");
        // Must not panic even with zero values.
        logger.log_usage("", "", 0, 0);
        sleep(Duration::from_millis(200)).await;
    }
}
