//! AWS Bedrock infrastructure
//!
//! Contains:
//! - `converse.rs` - Converse API (standard chat)
//! - `invoke.rs` - Invoke API (thinking/reasoning)
//! - `anthropic_translator.rs` - Anthropic native → Bedrock translation

pub mod anthropic_translator;
pub mod converse;
pub mod invoke;

// Explicit re-exports to avoid ambiguous glob conflicts
pub use anthropic_translator::anthropic_model_to_bedrock;
pub use anthropic_translator::build_thinking_request_from_anthropic;
pub use anthropic_translator::ConversePayload as AnthropicConversePayload;
pub use anthropic_translator::ThinkingRequestBody;
pub use converse::build_converse_payload;
pub use invoke::build_thinking_request;
pub use invoke::invoke_thinking_model;
pub use invoke::parse_thinking_params;
pub use invoke::ThinkingResponse;