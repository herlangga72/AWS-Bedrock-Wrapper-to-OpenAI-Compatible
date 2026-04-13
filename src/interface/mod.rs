//! Interface layer - HTTP handlers and API endpoints
//!
//! Contains:
//! - `chat` - Chat completion endpoints (standard, thinking, reasoning)
//! - `embedding` - Embedding endpoints
//! - `models` - Model listing endpoint

pub mod chat;
pub mod embedding;
pub mod models;
