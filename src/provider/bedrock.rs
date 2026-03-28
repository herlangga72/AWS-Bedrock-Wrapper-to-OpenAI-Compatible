use aws_sdk_bedrock::Client as MgmtClient;
use aws_sdk_bedrockruntime::{
    types::{ContentBlock, ContentBlockDelta, ConversationRole, Message as BedrockMessage},
    Client as RuntimeClient,
};
use axum::response::sse::Event;
use futures_util::Stream;
use serde_json::json;
use std::{convert::Infallible, time::Duration};
use tokio::time::timeout;
use uuid::Uuid;

use crate::logging::ClickHouseLogger;
use crate::types::openai::{Message, ModelData};
use super::ProviderError;

#[derive(Clone)]
pub struct BedrockProvider {
    runtime: RuntimeClient,
    mgmt: MgmtClient,
}

impl BedrockProvider {
    pub fn new(runtime: RuntimeClient, mgmt: MgmtClient) -> Self {
        Self { runtime, mgmt }
    }

    /// Non-streaming chat. Returns `(content, prompt_tokens, completion_tokens)`.
    pub async fn chat(
        &self,
        model_id: &str,
        messages: Vec<Message>,
    ) -> Result<(String, u32, u32), ProviderError> {
        let result = self
            .runtime
            .converse()
            .model_id(model_id)
            .set_messages(Some(to_bedrock_messages(messages)))
            .send()
            .await
            .map_err(|e| ProviderError::Upstream(e.to_string()))?;

        let (pt, ct) = result
            .usage
            .map(|u| (u.input_tokens as u32, u.output_tokens as u32))
            .unwrap_or((0, 0));

        let content = result
            .output
            .and_then(|o| match o {
                aws_sdk_bedrockruntime::types::ConverseOutput::Message(m) => Some(m),
                _ => None,
            })
            .and_then(|m| {
                m.content.into_iter().next().and_then(|c| {
                    if let ContentBlock::Text(t) = c { Some(t) } else { None }
                })
            })
            .unwrap_or_default();

        Ok((content, pt, ct))
    }

    /// Streaming chat. Logs token usage via `logger` once the stream completes.
    pub fn stream(
        &self,
        model_id: String,
        model_name: String,
        messages: Vec<Message>,
        logger: ClickHouseLogger,
        user_email: String,
    ) -> impl Stream<Item = Result<Event, Infallible>> {
        let client = self.runtime.clone();
        let bedrock_messages = to_bedrock_messages(messages);
        let request_id = format!("chatcmpl-{}", Uuid::new_v4());

        async_stream::stream! {
            let stream_res = timeout(
                Duration::from_secs(60),
                client
                    .converse_stream()
                    .model_id(&model_id)
                    .set_messages(Some(bedrock_messages))
                    .send(),
            )
            .await;

            let mut resp = match stream_res {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => {
                    yield Ok(Event::default().data(json!({"error": e.to_string()}).to_string()));
                    return;
                }
                Err(_) => {
                    yield Ok(Event::default().data(r#"{"error":"stream timeout"}"#));
                    return;
                }
            };

            let mut prompt_tokens = 0u32;
            let mut completion_tokens = 0u32;

            while let Ok(Some(event)) = resp.stream.recv().await {
                use aws_sdk_bedrockruntime::types::ConverseStreamOutput as Out;
                match event {
                    Out::ContentBlockDelta(delta) => {
                        if let Some(ContentBlockDelta::Text(t)) = delta.delta {
                            let chunk = json!({
                                "id": request_id,
                                "object": "chat.completion.chunk",
                                "created": now_secs(),
                                "model": model_name,
                                "choices": [{"index":0,"delta":{"content":t},"finish_reason":null}]
                            });
                            yield Ok(Event::default().data(chunk.to_string()));
                        }
                    }
                    Out::Metadata(m) => {
                        if let Some(u) = m.usage {
                            prompt_tokens = u.input_tokens as u32;
                            completion_tokens = u.output_tokens as u32;
                        }
                    }
                    Out::MessageStop(stop) => {
                        let last = json!({
                            "id": request_id,
                            "object": "chat.completion.chunk",
                            "model": model_name,
                            "choices": [{"index":0,"delta":{},"finish_reason":format!("{:?}",stop.stop_reason)}]
                        });
                        yield Ok(Event::default().data(last.to_string()));
                    }
                    _ => {}
                }
            }

            logger.log_usage(&user_email, &model_name, prompt_tokens, completion_tokens);

            if prompt_tokens > 0 || completion_tokens > 0 {
                let usage = json!({
                    "id": request_id,
                    "object": "chat.completion.chunk",
                    "created": now_secs(),
                    "model": model_name,
                    "choices": [],
                    "usage": {
                        "prompt_tokens": prompt_tokens,
                        "completion_tokens": completion_tokens,
                        "total_tokens": prompt_tokens + completion_tokens
                    }
                });
                yield Ok(Event::default().data(usage.to_string()));
            }

            yield Ok(Event::default().data("[DONE]"));
        }
    }

    /// Lists available foundation models from the Bedrock management API.
    pub async fn list_models(&self) -> Vec<ModelData> {
        match self.mgmt.list_foundation_models().send().await {
            Ok(resp) => resp
                .model_summaries
                .unwrap_or_default()
                .into_iter()
                .map(|m| ModelData {
                    id: m.model_id,
                    object: "model".into(),
                    created: 0,
                    owned_by: m.provider_name.unwrap_or_else(|| "bedrock".into()),
                })
                .collect(),
            Err(e) => {
                tracing::error!("Failed to list Bedrock models: {e}");
                vec![]
            }
        }
    }
}

fn to_bedrock_messages(messages: Vec<Message>) -> Vec<BedrockMessage> {
    messages
        .into_iter()
        .map(|m| {
            let role = match m.role.as_str() {
                "assistant" => ConversationRole::Assistant,
                _ => ConversationRole::User,
            };
            BedrockMessage::builder()
                .role(role)
                .content(ContentBlock::Text(m.content))
                .build()
                .unwrap_or_else(|e| panic!("Failed to build Bedrock message: {e}"))
        })
        .collect()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
