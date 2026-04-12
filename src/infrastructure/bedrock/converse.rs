//! AWS Bedrock Converse API implementation

use aws_sdk_bedrockruntime::types::{
    ContentBlock as BContentBlock, ConversationRole, InferenceConfiguration,
    Message as BedrockMessage, SystemContentBlock,
};

use crate::domain::chat::{map_openai_params, ChatRequest, Content};

/// Extract text from content
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
        match m.role.as_str() {
            "user" => {
                let text = extract_text_from_content(&m.content);
                if let Ok(msg) = BedrockMessage::builder()
                    .role(ConversationRole::User)
                    .content(BContentBlock::Text(text))
                    .build()
                {
                    bedrock_messages.push(msg);
                }
            }
            "assistant" => {
                let text = extract_text_from_content(&m.content);
                if let Ok(msg) = BedrockMessage::builder()
                    .role(ConversationRole::Assistant)
                    .content(BContentBlock::Text(text))
                    .build()
                {
                    bedrock_messages.push(msg);
                }
            }
            "system" => {
                let text = extract_text_from_content(&m.content);
                system_blocks.push(SystemContentBlock::Text(text));
            }
            _ => {}
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
