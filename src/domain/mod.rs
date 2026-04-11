//! Domain layer - Core business logic and entities
//!
//! Contains bounded contexts:
//! - `chat` - Chat completions, thinking, reasoning
//! - `embedding` - Text embeddings
//! - `auth` - API key authentication
//! - `logging` - Usage logging

pub mod chat;
pub mod embedding;
pub mod auth;
pub mod logging;
