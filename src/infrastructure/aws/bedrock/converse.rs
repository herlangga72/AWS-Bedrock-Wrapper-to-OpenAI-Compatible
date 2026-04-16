//! AWS Bedrock Converse API implementation

use aws_sdk_bedrockruntime::types::{
    ContentBlock as BContentBlock, ConversationRole, InferenceConfiguration,
    Message as BedrockMessage, SystemContentBlock,
};
use tracing::warn;

use crate::domain::chat::{map_openai_params, ChatRequest, Content, ToolDefinition, ToolChoice};

/// Converse payload structure
pub struct ConversePayload {
    pub system: Option<Vec<SystemContentBlock>>,
    pub messages: Vec<BedrockMessage>,
    pub inference_config: InferenceConfiguration,
}

/// Build payload for Converse API from ChatRequest
pub fn build_converse_payload(req: &ChatRequest) -> ConversePayload {
    let total_len = req.messages.len();
    let mut bedrock_messages = Vec::with_capacity(total_len);
    let mut system_blocks = Vec::new();

    let (base_params, _) = map_openai_params(
        &req.model,
        req.temperature,
        req.top_p,
        req.max_tokens,
        req.stop_sequences.clone(),
        req.frequency_penalty,
        req.presence_penalty,
        req.top_k,
    );

    for m in &req.messages {
        let role = m.role.as_str();
        if role != "user" && role != "assistant" && role != "system" && role != "tool" {
            warn!("Skipping message with unrecognized role: {}", role);
            continue;
        }

        let conversation_role = match role {
            "user" => ConversationRole::User,
            "assistant" => ConversationRole::Assistant,
            "system" => {
                let text = extract_text_from_content(&m.content);
                system_blocks.push(SystemContentBlock::Text(text));
                continue;
            }
            "tool" => {
                // Tool result - format as user message with tool content
                let content = extract_tool_result_content(&m.content, &m.tool_calls);
                bedrock_messages.push(
                    BedrockMessage::builder()
                        .role(ConversationRole::User)
                        .content(BContentBlock::Text(content))
                        .build()
                        .unwrap()
                );
                continue;
            }
            _ => continue,
        };

        // Handle assistant message with tool calls
        if let Some(ref tool_calls) = m.tool_calls {
            for tc in tool_calls {
                let content = serde_json::json!({
                    "toolUse": {
                        "toolCallId": tc.id,
                        "name": tc.function.name,
                        "input": serde_json::from_str::<serde_json::Value>(&tc.function.arguments)
                            .unwrap_or(serde_json::Value::Null),
                    }
                }).to_string();

                bedrock_messages.push(
                    BedrockMessage::builder()
                        .role(conversation_role.clone())
                        .content(BContentBlock::Text(content))
                        .build()
                        .unwrap()
                );
            }
        } else {
            let text = extract_text_from_content(&m.content);
            bedrock_messages.push(
                BedrockMessage::builder()
                    .role(conversation_role)
                    .content(BContentBlock::Text(text))
                    .build()
                    .unwrap()
            );
        }
    }

    let mut config_builder = InferenceConfiguration::builder()
        .set_temperature(base_params.temperature)
        .set_top_p(base_params.top_p)
        .set_max_tokens(base_params.max_tokens.map(|m| m as i32));

    if let Some(stop) = &base_params.stop_sequences {
        config_builder = config_builder.set_stop_sequences(Some(vec![stop.clone()]));
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

/// Build tool configuration from request (for models that support it)
/// Note: Not all Bedrock models support tool configuration via Converse API.
/// For unsupported models, tools will be included in the system prompt instead.
pub fn build_tool_config(req: &ChatRequest) -> Option<String> {
    let tools = req.tools.as_ref()?;
    if tools.is_empty() {
        return None;
    }

    // If tool_choice is "none", skip tools
    if let Some(ToolChoice::None) = &req.tool_choice {
        return None;
    }

    // Format tools as JSON for system prompt
    let tools_json: Vec<serde_json::Value> = tools
        .iter()
        .filter_map(|t| {
            let function = t.function.as_ref()?;
            Some(serde_json::json!({
                "type": "function",
                "function": {
                    "name": function.name,
                    "description": function.description.clone().unwrap_or_default(),
                    "parameters": function.parameters.clone().unwrap_or(serde_json::json!({}))
                }
            }))
        })
        .collect();

    if tools_json.is_empty() {
        return None;
    }

    Some(format!(
        "You have access to the following tools: {}",
        serde_json::to_string(&tools_json).unwrap_or_default()
    ))
}

/// Extract text from content
pub fn extract_text_from_content(content: &Content) -> String {
    match content {
        Content::Text(s) => s.clone(),
        Content::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| b.text.clone())
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// Extract tool result content from message
fn extract_tool_result_content(content: &Content, tool_calls: &Option<Vec<crate::domain::chat::ToolCall>>) -> String {
    if let Some(tc) = tool_calls {
        if let Some(first) = tc.first() {
            return serde_json::json!({
                "toolResult": {
                    "toolCallId": first.id,
                    "name": first.function.name,
                    "content": extract_text_from_content(content),
                }
            }).to_string();
        }
    }
    extract_text_from_content(content)
}