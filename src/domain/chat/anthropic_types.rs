//! Anthropic native API types for /v1/messages endpoint

use serde::{Deserialize, Serialize};

/// Anthropic /v1/messages request body
#[derive(Deserialize, Clone)]
pub struct AnthropicMessagesRequest {
    /// Model identifier, e.g. "claude-3-5-sonnet-20240620"
    pub model: String,
    /// Messages array (user, assistant, system)
    pub messages: Vec<AnthropicMessage>,
    /// System prompt as a separate string (not in messages array)
    #[serde(default)]
    pub system: Option<String>,
    /// Maximum tokens to generate
    pub max_tokens: u32,
    /// Whether to stream the response
    #[serde(default)]
    pub stream: bool,
    /// Sampling temperature
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Nucleus sampling
    #[serde(default)]
    pub top_p: Option<f32>,
    /// Custom stop sequences
    #[serde(default)]
    pub stop_sequences: Option<Vec<String>>,
    /// Anthropic beta header for extended thinking
    #[serde(default)]
    pub thinking: Option<AnthropicThinking>,
}

#[derive(Deserialize, Clone)]
pub struct AnthropicThinking {
    #[serde(rename = "type")]
    pub thinking_type: Option<String>,
    pub budget_tokens: Option<u32>,
}

/// A single message in Anthropic format
#[derive(Deserialize, Serialize, Clone)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: AnthropicContent,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum AnthropicContent {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

#[derive(Deserialize, Serialize, Clone)]
pub struct AnthropicContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Anthropic non-streaming response
#[derive(Serialize)]
pub struct AnthropicResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<AnthropicResponseBlock>,
    pub model: String,
    pub stop_reason: String,
    pub stop_sequence: Option<String>,
    pub usage: AnthropicUsage,
}

#[derive(Serialize)]
pub struct AnthropicResponseBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<AnthropicReasoningBlock>,
}

#[derive(Serialize)]
pub struct AnthropicReasoningBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

#[derive(Serialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl AnthropicUsage {
    pub fn new(input: u32, output: u32) -> Self {
        Self {
            input_tokens: input,
            output_tokens: output,
        }
    }
}
