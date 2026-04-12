//! Chat interface - HTTP handlers for chat completions
//!
//! Contains:
//! - `chat_handler` - Standard chat (converse API)
//! - `thinking_handler` - Claude extended thinking
//! - `reasoning_handler` - DeepSeek R1 reasoning

pub mod chat_handler;
pub mod reasoning_handler;
pub mod thinking_handler;

pub use chat_handler::chat_handler;
pub use thinking_handler::chat_with_thinking_handler;
