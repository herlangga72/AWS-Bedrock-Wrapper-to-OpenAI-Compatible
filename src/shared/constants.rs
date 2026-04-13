//! Application-wide constants

/// Server configuration
pub const DEFAULT_HOST: &str = "0.0.0.0";
pub const DEFAULT_PORT: u16 = 3001;

/// Request timeouts (in seconds)
pub const REQUEST_TIMEOUT_CHAT: u64 = 60;
pub const REQUEST_TIMEOUT_THINKING: u64 = 120;
pub const REQUEST_TIMEOUT_CLOUDFLARE: u64 = 120;

/// Validation limits
pub const MIN_TEMPERATURE: f32 = 0.0;
pub const MAX_TEMPERATURE: f32 = 2.0;

/// Model IDs for AWS Bedrock
pub const NOVA_EMBED_MODEL_ID: &str = "amazon.nova-2-multimodal-embeddings-v1:0";

/// ClickHouse configuration defaults
pub const CLICKHOUSE_URL: &str = "http://127.0.0.1:8123";
pub const CLICKHOUSE_USER: &str = "default";
pub const CLICKHOUSE_PASSWORD: &str = ""; // Empty default, override via env var
pub const CLICKHOUSE_DB: &str = "default";
pub const CLICKHOUSE_BATCH_SIZE: usize = 5000;
pub const CLICKHOUSE_FLUSH_INTERVAL_SECS: u64 = 2;

/// Token limits per model family (defaults)
pub const DEFAULT_MAX_TOKENS_LLAMA: u32 = 512;
pub const DEFAULT_MAX_TOKENS_CLAUDE: u32 = 4096;
pub const DEFAULT_MAX_TOKENS_COHERE: u32 = 2048;
pub const DEFAULT_MAX_TOKENS_MISTRAL: u32 = 4096;
pub const DEFAULT_MAX_TOKENS_DEEPSEEK: u32 = 8192;
pub const DEFAULT_MAX_TOKENS_TITAN: u32 = 2048;
pub const DEFAULT_MAX_TOKENS_NOVA: u32 = 4096;
pub const DEFAULT_MAX_TOKENS_AI21: u32 = 2048;
pub const DEFAULT_MAX_TOKENS_CLOUDFLARE: u32 = 256;