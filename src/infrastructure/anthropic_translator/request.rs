//! Translate AnthropicMessagesRequest to Bedrock Converse payload

use aws_sdk_bedrockruntime::types::{
    ContentBlock as BContentBlock, ConversationRole, InferenceConfiguration,
    Message as BedrockMessage, SystemContentBlock,
};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::HashMap;

use crate::domain::chat::anthropic_types::{AnthropicContent, AnthropicMessagesRequest};

/// Cache of known Anthropic model names → Bedrock model IDs
static ANTHROPIC_MODEL_CACHE: Lazy<HashMap<String, String>> = Lazy::new(|| {
    HashMap::from_iter([
        // Claude 3.5 models
        ("claude-3-5-sonnet-20240620".into(), "anthropic.claude-3-5-sonnet-v1:0".into()),
        ("claude-3-5-sonnet-20241022".into(), "anthropic.claude-3-5-sonnet-v2:0".into()),
        ("claude-3-5-haiku-20241022".into(), "anthropic.claude-3-5-haiku-v1:0".into()),
        ("claude-3-7-sonnet-20250620".into(), "anthropic.claude-3-7-sonnet-v1:0".into()),
        // Claude 4 models
        ("claude-sonnet-4-5-20250929".into(), "anthropic.claude-sonnet-4-5-20250929-v1:0".into()),
        ("claude-sonnet-4-6-20250514".into(), "anthropic.claude-sonnet-4-6-v1:0".into()),
        ("claude-opus-4-5-20261111".into(), "anthropic.claude-opus-4-5-20261111-v1:0".into()),
        ("claude-opus-4-6-20250514".into(), "anthropic.claude-opus-4-6-v1:0".into()),
        ("claude-haiku-4-5-20251001".into(), "anthropic.claude-haiku-4-5-20251001-v1:0".into()),
        ("claude-haiku-4-6-20250514".into(), "anthropic.claude-haiku-4-6-v1:0".into()),
        // Legacy models
        ("claude-3-opus-20240229".into(), "anthropic.claude-3-opus-v1:0".into()),
        ("claude-3-sonnet-20240229".into(), "anthropic.claude-3-sonnet-v1:0".into()),
        ("claude-3-haiku-20240307".into(), "anthropic.claude-3-haiku-v1:0".into()),
    ])
});

/// Convert Anthropic model name to Bedrock model ID
/// e.g. "claude-3-5-sonnet-20240620" -> "anthropic.claude-3-5-sonnet-v1:0"
pub fn anthropic_model_to_bedrock(model: &str) -> String {
    // If already a Bedrock model ID, return as-is
    if model.contains('.') || model.contains("bedrock/") {
        return model.to_string();
    }

    // Check cache first (avoids to_lowercase allocation on cache hit)
    if let Some(cached) = ANTHROPIC_MODEL_CACHE.get(model) {
        return cached.clone();
    }

    // Unknown model — try case-insensitive match against cache keys
    let lower = model.to_lowercase();
    if let Some(cached) = ANTHROPIC_MODEL_CACHE.get(&lower) {
        return cached.clone();
    }

    // Parse and convert unknown Anthropic model names
    // For claude- family: claude-3-5-sonnet-20240620 -> anthropic.claude-3-5-sonnet-v1:0
    if lower.starts_with("claude-") {
        if let Some(dash_pos) = lower[7..].find('-') {
            let version = &lower[7..][..dash_pos]; // e.g. "3-5-sonnet"
            return format!("anthropic.{}", version);
        }
    }

    // Unknown model, return as-is
    model.to_string()
}

/// Build a Converse payload from Anthropic request
pub struct ConversePayload {
    pub system: Option<Vec<SystemContentBlock>>,
    pub messages: Vec<BedrockMessage>,
    pub inference_config: InferenceConfiguration,
}

impl ConversePayload {
    pub fn from_anthropic_request(req: &AnthropicMessagesRequest) -> Self {
        let mut system_blocks = Vec::new();
        let mut bedrock_messages = Vec::with_capacity(req.messages.len());

        // Add system prompt if present
        if let Some(ref sys) = req.system {
            system_blocks.push(SystemContentBlock::Text(sys.clone()));
        }

        for msg in &req.messages {
            let role = match msg.role.as_str() {
                "user" => ConversationRole::User,
                "assistant" => ConversationRole::Assistant,
                "system" => {
                    // Anthropic system messages in the messages array
                    let text = extract_text_from_content(&msg.content);
                    system_blocks.push(SystemContentBlock::Text(text));
                    continue;
                }
                other => {
                    tracing::warn!("Skipping unknown role in Anthropic request: {}", other);
                    continue;
                }
            };

            let text = extract_text_from_content(&msg.content);

            if let Ok(bedrock_msg) = BedrockMessage::builder()
                .role(role)
                .content(BContentBlock::Text(text))
                .build()
            {
                bedrock_messages.push(bedrock_msg);
            }
        }

        let mut config_builder = InferenceConfiguration::builder()
            .set_max_tokens(Some(req.max_tokens as i32))
            .set_temperature(req.temperature.map(|t| t as f32));

        if let Some(top_p) = req.top_p {
            config_builder = config_builder.set_top_p(Some(top_p as f32));
        }

        if let Some(ref stopseqs) = req.stop_sequences {
            if !stopseqs.is_empty() {
                config_builder = config_builder.set_stop_sequences(Some(stopseqs.clone()));
            }
        }

        ConversePayload {
            system: if system_blocks.is_empty() {
                None
            } else {
                Some(system_blocks)
            },
            messages: bedrock_messages,
            inference_config: config_builder.build(),
        }
    }
}

fn extract_text_from_content(content: &AnthropicContent) -> String {
    match content {
        AnthropicContent::Text(s) => s.clone(),
        AnthropicContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| b.text.clone())
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

// =============================================================================
// Invoke API (thinking) request types
// =============================================================================

const THINKING_VERSION: &str = "bedrock-2023-05-31";

#[derive(Serialize)]
pub struct ThinkingRequestBody<'a> {
    anthropic_version: &'a str,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingBlock>,
    messages: Vec<ThinkingMessage>,
}

#[derive(Serialize)]
struct ThinkingBlock {
    r#type: &'static str,
    budget_tokens: u32,
}

#[derive(Serialize)]
struct ThinkingMessage {
    role: String,
    content: ThinkingContent,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ThinkingContent {
    OwnedText(String),
    Blocks(Vec<ThinkingContentBlock>),
}

#[derive(Serialize)]
struct ThinkingContentBlock {
    r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<String>,
}

/// Build Invoke API thinking request from Anthropic request
pub fn build_thinking_request_from_anthropic(
    req: &AnthropicMessagesRequest,
) -> ThinkingRequestBody<'static> {
    let budget_tokens = req
        .thinking
        .as_ref()
        .and_then(|t| t.budget_tokens)
        .unwrap_or(4000);

    let max_tokens = req.max_tokens.max(budget_tokens + 1024);

    let thinking_block = Some(ThinkingBlock {
        r#type: "enabled",
        budget_tokens,
    });

    let mut messages = Vec::with_capacity(req.messages.len());

    // Prepend system prompt if present
    if let Some(ref sys) = req.system {
        messages.push(ThinkingMessage {
            role: "user".to_string(),
            content: ThinkingContent::OwnedText(sys.clone()),
        });
    }

    for msg in &req.messages {
        let role = match msg.role.as_str() {
            "user" => "user".to_string(),
            "assistant" => "assistant".to_string(),
            "system" => {
                // System messages in array → prepend as user message
                let text = extract_text_from_content(&msg.content);
                messages.push(ThinkingMessage {
                    role: "user".to_string(),
                    content: ThinkingContent::OwnedText(text),
                });
                continue;
            }
            other => {
                tracing::warn!("Unknown role in thinking request: {}", other);
                continue;
            }
        };

        let content = match &msg.content {
            AnthropicContent::Text(s) => ThinkingContent::OwnedText(s.clone()),
            AnthropicContent::Blocks(blocks) => {
                let converted: Vec<ThinkingContentBlock> = blocks
                    .iter()
                    .map(|b| {
                        let block_type = if b.thinking.is_some() {
                            "thinking".to_string()
                        } else {
                            "text".to_string()
                        };
                        ThinkingContentBlock {
                            r#type: block_type,
                            text: b.text.clone(),
                            thinking: b.thinking.clone(),
                        }
                    })
                    .collect();
                ThinkingContent::Blocks(converted)
            }
        };

        messages.push(ThinkingMessage { role, content });
    }

    ThinkingRequestBody {
        anthropic_version: THINKING_VERSION,
        max_tokens,
        thinking: thinking_block,
        messages,
    }
}
