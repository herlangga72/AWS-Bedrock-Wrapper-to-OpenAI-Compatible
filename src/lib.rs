//! AWS Bedrock Translation to OpenAI Compatible API
//!
//! This library provides a proxy layer that translates OpenAI-compatible API requests
//! to AWS Bedrock and Cloudflare Workers AI endpoints.

pub mod domain;
pub mod infrastructure;
pub mod interface;
pub mod shared;

// Re-export AppState for convenience
pub use shared::app_state::AppState;
