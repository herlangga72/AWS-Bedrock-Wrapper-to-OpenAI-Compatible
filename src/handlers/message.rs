use crate::models::ChatRequest;
// Note the full name: InferenceConfiguration
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
    let mut system_blocks = Vec::new();
    let mut conversation_messages = Vec::new();

    // 1. Map messages and extract system prompt
    for m in req.messages {
        match m.role.as_str() {
            "system" => {
                system_blocks.push(SystemContentBlock::Text(m.content));
            }
            "assistant" => {
                conversation_messages.push(
                    BedrockMessage::builder()
                        .role(ConversationRole::Assistant)
                        .content(ContentBlock::Text(m.content))
                        .build()
                        .expect("Failed to build assistant message"),
                );
            }
            _ => {
                conversation_messages.push(
                    BedrockMessage::builder()
                        .role(ConversationRole::User)
                        .content(ContentBlock::Text(m.content))
                        .build()
                        .expect("Failed to build user message"),
                );
            }
        }
    }

    // 2. Build the correctly named InferenceConfiguration
    let mut config_builder = InferenceConfiguration::builder();
    
    if let Some(temp) = req.temperature {
        config_builder = config_builder.temperature(temp);
    }
    if let Some(top_p) = req.top_p {
        config_builder = config_builder.top_p(top_p);
    }
    if let Some(max_t) = req.max_tokens {
        // Bedrock expects i32 for max_tokens
        config_builder = config_builder.max_tokens(max_t as i32);
    }
    if let Some(stop_seq) = req.stop_sequences {
        config_builder = config_builder.stop_sequences(stop_seq);
    }

    BedrockPayload {
        system: if system_blocks.is_empty() { None } else { Some(system_blocks) },
        messages: conversation_messages,
        inference_config: config_builder.build(),
    }
}