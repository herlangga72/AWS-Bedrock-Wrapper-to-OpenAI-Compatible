//! OpenAI-compatible HTTP handlers
//!
//! Handles OpenAI-format requests and returns OpenAI-format responses.

pub mod chat;
pub mod completions;

pub use completions::openai_chat_handler;