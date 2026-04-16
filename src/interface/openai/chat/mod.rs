//! OpenAI-compatible chat handlers

pub mod chat_handler;
pub mod thinking_handler;
pub mod reasoning_handler;

pub use chat_handler::chat_handler;
pub use thinking_handler::chat_with_thinking_handler;