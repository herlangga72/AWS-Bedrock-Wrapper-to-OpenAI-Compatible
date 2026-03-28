pub struct Config {
    pub aws_region: String,
    pub api_key: String,
    pub clickhouse_url: String,
    pub server_host: String,
    pub server_port: String,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        Self {
            aws_region: std::env::var("AWS_REGION")
                .unwrap_or_else(|_| "us-east-1".to_string()),
            api_key: std::env::var("API_KEY").expect("API_KEY must be set"),
            clickhouse_url: std::env::var("CLICKHOUSE_URL")
                .expect("CLICKHOUSE_URL must be set"),
            server_host: std::env::var("SERVER_HOST")
                .unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: std::env::var("SERVER_PORT")
                .unwrap_or_else(|_| "3001".to_string()),
        }
    }

    pub fn addr(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }
}
