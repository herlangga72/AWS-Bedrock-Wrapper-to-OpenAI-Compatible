//! Infrastructure layer - External integrations
//!
//! Contains implementations for:
//! - `bedrock` - AWS Bedrock runtime and management clients
//! - `cloudflare` - Cloudflare Workers AI client
//! - `persistence` - SQLite auth, ClickHouse logging
//! - `cache` - File-based caching

pub mod bedrock;
pub mod cache;
pub mod cloudflare;
pub mod persistence;
