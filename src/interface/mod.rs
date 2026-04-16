//! Interface layer - HTTP handlers organized by API compatibility
//!
//! Structure:
//! - `openai/` - OpenAI-compatible endpoints (/v1/chat/completions, etc.)
//! - `anthropic/` - Anthropic-native endpoints (/v1/messages)
//! - `common/` - Shared handlers (models, embeddings)

pub mod anthropic;
pub mod common;
pub mod openai;