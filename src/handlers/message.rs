use crate::models::ChatRequest;
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, InferenceConfiguration, Message as BedrockMessage,
    SystemContentBlock,
};

pub struct BedrockPayload {
    pub system: Option<Vec<SystemContentBlock>>,
    pub messages: Vec<BedrockMessage>,
    pub inference_config: InferenceConfiguration,
}

pub fn build_bedrock_payload(mut req: ChatRequest) -> BedrockPayload {
    let total_len = req.messages.len();
    let mut bedrock_messages = Vec::with_capacity(total_len);
    let mut system_blocks = Vec::new();

    for m in std::mem::take(&mut req.messages).into_iter() {
        match m.role.as_str() {
            "user" => {
                if let Ok(msg) = BedrockMessage::builder()
                    .role(ConversationRole::User)
                    .content(ContentBlock::Text(m.content))
                    .build()
                {
                    bedrock_messages.push(msg);
                }
            }
            "assistant" => {
                if let Ok(msg) = BedrockMessage::builder()
                    .role(ConversationRole::Assistant)
                    .content(ContentBlock::Text(m.content))
                    .build()
                {
                    bedrock_messages.push(msg);
                }
            }
            "system" => {
                if system_blocks.is_empty() {
                    system_blocks.reserve_exact(1);
                }
                system_blocks.push(SystemContentBlock::Text(m.content));
            }
            _ => {
                if let Ok(msg) = BedrockMessage::builder()
                    .role(ConversationRole::User)
                    .content(ContentBlock::Text(m.content))
                    .build()
                {
                    bedrock_messages.push(msg);
                }
            }
        }
    }

    let mut config_builder = InferenceConfiguration::builder()
        .set_temperature(req.temperature)
        .set_top_p(req.top_p)
        .set_max_tokens(req.max_tokens.map(|m| m as i32));

    if let Some(stop) = req.stop_sequences {
        config_builder = config_builder.set_stop_sequences(Some(vec![stop]));
    }

    BedrockPayload {
        system: if system_blocks.is_empty() {
            None
        } else {
            Some(system_blocks)
        },
        messages: bedrock_messages,
        inference_config: config_builder.build(),
    }
}
