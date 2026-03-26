use reqwest::blocking::Client;
use chrono::Utc;
use std::sync::Arc;

#[derive(Clone)]
pub struct ClickHouseLogger {
    url: Arc<String>,
    client: Arc<Client>,
}

impl ClickHouseLogger {
    pub fn new(_addr: &str) -> Self {
        Self {
            url: Arc::new("http://default:bhpums2024@127.0.0.1:8123".to_string()),
            client: Arc::new(Client::new()),
        }
    }

    pub async fn log_usage(
        &self,
        user_email: &str,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) {
        let url = Arc::clone(&self.url);
        let client = Arc::clone(&self.client);

        let user_email = user_email.to_string();
        let model = model.to_string();

        let _ = tokio::task::spawn_blocking(move || {
            let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S");

            let query = format!(
                "INSERT INTO chat_logs (timestamp, model, user_email, input_tokens, output_tokens)
                 VALUES ('{}', '{}', '{}', {}, {})",
                timestamp,
                model.replace('\'', "''"),
                user_email.replace('\'', "''"),
                input_tokens,
                output_tokens
            );

            match client.post(url.as_str()).body(query).send() {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        eprintln!("ClickHouse HTTP Error: Status {}", resp.status());
                    }
                }
                Err(e) => {
                    eprintln!("ClickHouse connection error: {:?}", e);
                }
            }
        }).await;
    }
}
