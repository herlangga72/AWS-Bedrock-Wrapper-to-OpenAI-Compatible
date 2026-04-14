//! Interface layer - HTTP handlers and API endpoints
//!
//! Contains:
//! - `anthropic` - Anthropic native /v1/messages endpoint
//! - `chat` - Chat completion endpoints (standard, thinking, reasoning)
//! - `embedding` - Embedding endpoints
//! - `models` - Model listing endpoint

pub mod anthropic;
pub mod chat;
pub mod embedding;
pub mod models;
