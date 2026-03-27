use reqwest::Client;
use serde::Serialize;
use chrono::Utc;

#[derive(Clone)]
pub struct ClickHouseLogger {
    url: String,
    client: Client, 
}

// Struct representing a single log row for ClickHouse's JSONEachRow format
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
            // Append the query parameter to tell ClickHouse to expect JSON.
            url: format!("{}/?query=INSERT+INTO+chat_logs+FORMAT+JSONEachRow", addr),
            client: Client::builder()
                // Prevent broken connections from hanging indefinitely
                .timeout(std::time::Duration::from_secs(5)) 
                .build()
                .expect("Failed to build reqwest::Client for ClickHouseLogger"),
        }
    }

    // Notice: No `async` keyword here. This function returns instantly.
    pub fn log_usage(
        &self,
        user_email: &str,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) {
        // Clone the client and URL to move them into the async background task.
        let client = self.client.clone();
        let url = self.url.clone();
        
        // Take ownership of strings so they live long enough for the background task
        let user_email = user_email.to_string();
        let model = model.to_string();

        // Spawn a background task so the main Axum handler isn't blocked
        tokio::spawn(async move {
            let entry = LogEntry {
                timestamp: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                model: &model,
                user_email: &user_email,
                input_tokens,
                output_tokens,
            };

            // Serialize our row to JSON
            let body = match serde_json::to_string(&entry) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Failed to serialize ClickHouse log entry: {}", e);
                    return;
                }
            };

            // Send the async request
            match client.post(&url).body(body).send().await {
                Ok(resp) if !resp.status().is_success() => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    eprintln!("ClickHouse HTTP Error: Status {} - {}", status, text);
                }
                Err(e) => {
                    eprintln!("ClickHouse connection error: {:?}", e);
                }
                _ => {} // Success!
            }
        });
    }
}