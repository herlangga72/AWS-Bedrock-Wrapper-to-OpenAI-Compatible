use crate::models::ChatRequest;
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, Message as BedrockMessage, 
    SystemContentBlock, InferenceConfiguration,
};

pub struct BedrockPayload {
    pub system: Option<Vec<SystemContentBlock>>,
    pub messages: Vec<BedrockMessage>,
    pub inference_config: InferenceConfiguration,
}

pub fn build_bedrock_payload(req: ChatRequest) -> BedrockPayload {
    let mut messages = req.messages;
    let total_len = messages.len();

    let mut bedrock_messages = Vec::with_capacity(total_len);
    let mut system_blocks = Vec::with_capacity(1); // Usually only one system prompt

    for m in messages.drain(..) {
        if m.role == "system" {
            system_blocks.push(SystemContentBlock::Text(m.content));
        } else {
            let role = match m.role.as_str() {
                "assistant" => ConversationRole::Assistant,
                _ => ConversationRole::User,
            };

            if let Ok(msg) = BedrockMessage::builder()
                .role(role)
                .content(ContentBlock::Text(m.content))
                .build() 
            {
                bedrock_messages.push(msg);
            }
        }
    }

    let inference_config = InferenceConfiguration::builder()
        .set_temperature(req.temperature)
        .set_top_p(req.top_p)
        .set_max_tokens(req.max_tokens.map(|m| m as i32))
        .set_stop_sequences(req.stop_sequences.map(|s| vec![s]))
        .build();

    BedrockPayload {
        system: if system_blocks.is_empty() { None } else { Some(system_blocks) },
        messages: bedrock_messages,
        inference_config,
    }
}