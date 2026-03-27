use crate::models::{ChatRequest, Message, ErrorResponse};
use crate::handlers::logger::ClickHouseLogger;
use axum::{
    extract::State,
    http::{StatusCode, HeaderMap},
    Json,
    response::{IntoResponse, Response as AxumResponse, sse::{Event, Sse, KeepAlive}},
};
use axum_extra::{TypedHeader, headers::{authorization::Bearer, Authorization}};
use aws_sdk_bedrockruntime::{
    types::{ContentBlock, ConversationRole, Message as BedrockMessage, ContentBlockDelta},
    Client as RuntimeClient,
};
use futures_util::Stream;
use serde_json::json;
use std::{convert::Infallible, time::Duration};
use tokio::time::timeout;
use uuid::Uuid;

// =======================
// Main Chat Handler
// =======================
pub async fn chat_handler(
    State(state): State<crate::AppState>,
    auth: Option<TypedHeader<Authorization<Bearer>>>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>, 
) -> AxumResponse {
    // Validate API key
    let user_email = match auth {
        Some(TypedHeader(auth_header)) => {
            if auth_header.token() != state.api_key {
                return error("invalid api key", StatusCode::UNAUTHORIZED);
            }
            headers.get("x-openwebui-user-email")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("anonymous")
                .to_string()
        }
        None => return error("missing authorization", StatusCode::UNAUTHORIZED),
    };

    let stream_flag = req.stream.unwrap_or(false);

    if stream_flag {
        let s = stream_converse(state.client.clone(), req, state.logger.clone(), user_email);
        Sse::new(s).keep_alive(KeepAlive::default()).into_response()
    } else {
        non_stream(state.client.clone(), req, state.logger.clone(), user_email).await
    }
}

// =======================
// STREAM MODE
// =======================
fn stream_converse(
    client: RuntimeClient,
    mut req: ChatRequest,
    logger: ClickHouseLogger,
    user_email: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let model_id = req.model.replace("bedrock/", "");
    let request_id = format!("chatcmpl-{}", Uuid::new_v4());
    let model_name = req.model.clone();

    async_stream::stream! {
        let messages = std::mem::take(&mut req.messages);
        let bedrock_messages = convert_messages_owned(messages);

        let stream_res = timeout(
            Duration::from_secs(60),
            client.converse_stream()
                .model_id(&model_id)
                .set_messages(Some(bedrock_messages)) 
                .send()
        ).await;

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
                            "created": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                            "model": model_name,
                            "choices": [{
                                "index": 0,
                                "delta": { "content": t },
                                "finish_reason": null
                            }]
                        });
                        yield Ok(Event::default().data(chunk.to_string()));
                    }
                }
                Out::Metadata(metadata) => {
                    if let Some(usage) = metadata.usage {
                        prompt_tokens = usage.input_tokens as u32;
                        completion_tokens = usage.output_tokens as u32;
                    }
                }
                Out::MessageStop(stop) => {
                    let last_chunk = json!({
                        "id": request_id,
                        "object": "chat.completion.chunk",
                        "model": model_name,
                        "choices": [{
                            "index": 0,
                            "delta": {},
                            "finish_reason": format!("{:?}", stop.stop_reason)
                        }]
                    });
                    yield Ok(Event::default().data(last_chunk.to_string()));
                }
                _ => {}
            }
        }

        logger.log_usage(&user_email, &model_name, prompt_tokens, completion_tokens);
        
        if prompt_tokens > 0 || completion_tokens > 0 {
            let usage_chunk = json!({
                "id": request_id,
                "object": "chat.completion.chunk",
                "created": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                "model": model_name,
                "choices": [], 
                "usage": {
                    "input_tokens": prompt_tokens,
                    "output_tokens": completion_tokens,
                    "total_tokens": prompt_tokens + completion_tokens
                }
            });
            yield Ok(Event::default().data(usage_chunk.to_string()));
        }

        yield Ok(Event::default().data("[DONE]"));
    }
}


// =======================
// NON-STREAM MODE
// =======================
async fn non_stream(
    client: RuntimeClient,
    mut req: ChatRequest,
    logger: ClickHouseLogger,
    user_email: String,
) -> AxumResponse {
    let model_id = req.model.replace("bedrock/", "");
    let messages = std::mem::take(&mut req.messages);
    let bedrock_messages = convert_messages_owned(messages);
    
    let result = client
        .converse()
        .model_id(model_id)
        .set_messages(Some(bedrock_messages))
        .send()
        .await;

    let resp = match result {
        Ok(r) => r,
        Err(e) => return error(&e.to_string(), StatusCode::INTERNAL_SERVER_ERROR),
    };

    let usage = resp.usage.expect("Usage data missing from Bedrock response");
    
    // Extract prompt and completion counts
    let prompt_tokens = usage.input_tokens as u32;
    let completion_tokens = usage.output_tokens as u32;
    let total_tokens = prompt_tokens + completion_tokens;

    // Extract the content string safely
    let content_str = resp.output
        .and_then(|o| {
            match o {
                aws_sdk_bedrockruntime::types::ConverseOutput::Message(m) => Some(m),
                _ => None,
            }
        })
        .and_then(|m| {
            m.content.into_iter().next().and_then(|c| {
                if let ContentBlock::Text(t) = c { Some(t) } else { None }
            })
        })
        .unwrap_or_default();
    
    logger.log_usage(
        &user_email, 
        &req.model, 
        prompt_tokens, 
        completion_tokens
    );

    Json(json!({
        "id": format!("chatcmpl-{}", Uuid::new_v4()),
        "object": "chat.completion",
        "created": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        "model": req.model,
        "choices": [{
            "index": 0,
            "message": { 
                "role": "assistant", 
                "content": content_str 
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "input_tokens": prompt_tokens,
            "output_tokens": completion_tokens,
            "total_tokens": total_tokens
        }
    })).into_response()
}

// =======================
// LIST MODELS HANDLER
// =======================
pub async fn list_models_handler(State(state): State<crate::AppState>) -> AxumResponse {
    match state.mgmt_client.list_foundation_models().send().await {
        Ok(resp) => {
            let data: Vec<crate::models::ModelData> = resp
                .model_summaries
                .unwrap_or_default()
                .into_iter()
                .map(|m| {
                    crate::models::ModelData {
                        id: m.model_id, 
                        object: "model".into(),
                        created: 0,
                        owned_by: m.provider_name.unwrap_or_else(|| "bedrock".into()),
                    }
                })
                .collect();
            
            Json(crate::models::ModelList {
                object: "list".into(),
                data,
            }).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list models: {:?}", e);
            Json(json!({"object": "list", "data": []})).into_response()
        }
    }
}

// =======================
// HELPERS
// =======================
fn error(msg: &str, code: StatusCode) -> AxumResponse {
    (code, Json(ErrorResponse { error: msg.into() })).into_response()
}

pub fn convert_messages_owned(messages: Vec<Message>) -> Vec<BedrockMessage> {
    messages.into_iter().map(|m| {
        let role = match m.role.as_str() {
            "assistant" => ConversationRole::Assistant,
            _ => ConversationRole::User,
        };
        
        BedrockMessage::builder()
            .role(role)
            .content(ContentBlock::Text(m.content)) 
            .build()
            .expect("Failed to build message")
    }).collect()
}