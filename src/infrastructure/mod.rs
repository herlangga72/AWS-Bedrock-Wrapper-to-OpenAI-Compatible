//! Infrastructure layer - External integrations organized by provider
//!
//! Structure:
//! - `aws/` - AWS services (Bedrock runtime, management)
//!   - `bedrock/` - Bedrock Converse API, Invoke API, and Anthropic-to-Bedrock translation
//! - `cloudflare/` - Cloudflare Workers AI client
//! - `cache/` - File-based caching
//! - `persistence/` - SQLite, ClickHouse (reserved)

pub mod aws;
pub mod cache;
pub mod cloudflare;
pub mod persistence;