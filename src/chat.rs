use axum::{
    extract::State,
    http::StatusCode,
    Json,
    response::{IntoResponse, Response as AxumResponse},
    response::sse::{Event, Sse, KeepAlive},
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use aws_sdk_bedrockruntime::{
    types::*,
    Client as RuntimeClient,
};
use aws_sdk_bedrock::Client as MgmtClient;
use async_stream::stream;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, time::Duration};
use tokio::time::timeout;
use uuid::Uuid;

// ===================== STATE =====================
#[derive(Clone)]
pub struct AppState {
    pub client: RuntimeClient,
    pub mgmt_client: MgmtClient,
    pub api_key: String,
}

// ===================== REQUEST =====================
#[derive(Deserialize, Clone)]
pub struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: Option<bool>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Message {
    role: String,
    content: String,
}

// ===================== RESPONSE =====================
#[derive(Serialize, Clone)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

// ===================== HANDLER =====================
pub async fn chat_handler(
    State(state): State<AppState>,
    auth: Option<TypedHeader<Authorization<Bearer>>>,
    Json(req): Json<ChatRequest>,
) -> AxumResponse {

    match auth {
        Some(TypedHeader(Authorization(b))) => {
            if b.token() != state.api_key {
                return error("invalid api key", StatusCode::UNAUTHORIZED);
            }
        }
        None => return error("missing authorization", StatusCode::UNAUTHORIZED),
    }

    if req.stream.unwrap_or(false) {
        Sse::new(stream_converse(state.client, req).await)
            .keep_alive(KeepAlive::new())
            .into_response()
    } else {
        non_stream(state.client, req).await
    }
}

// ===================== STREAM =====================
async fn stream_converse(
    client: RuntimeClient,
    req: ChatRequest,
) -> impl futures_util::Stream<Item = Result<Event, Infallible>> + Send {

    let model_id = req.model.replace("bedrock/", "");
    let request_id = Uuid::new_v4().to_string();
    let messages = convert_messages(&req.messages);

    let prompt_tokens: u32 = req.messages
        .iter()
        .map(|m| estimate_tokens(&m.content))
        .sum();

    Box::pin(stream! {
        let mut completion_tokens: u32 = 0;

        let resp = match timeout(
            Duration::from_secs(60),
            client
                .converse_stream()
                .model_id(model_id.clone())
                .set_messages(Some(messages.clone()))
                .send()
        ).await {
            Ok(Ok(r)) => r,
            _ => {
                yield Ok(Event::default().data(r#"{"error":"stream failed"}"#));
                yield Ok(Event::default().data("[DONE]"));
                return;
            }
        };

        let mut stream = resp.stream;

        while let Ok(Some(event)) = stream.recv().await {
            if let ConverseStreamOutput::ContentBlockDelta(delta) = event {
                if let Some(ContentBlockDelta::Text(t)) = delta.delta {

                    completion_tokens += estimate_tokens(&t);

                    let chunk = serde_json::json!({
                        "id": request_id,
                        "object": "chat.completion.chunk",
                        "model": req.model,
                        "choices": [{
                            "delta": { "content": t },
                            "index": 0,
                            "finish_reason": null
                        }]
                    });

                    yield Ok(Event::default().data(chunk.to_string()));
                }
            }
        }

        let usage = Usage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        };

        let final_chunk = serde_json::json!({
            "id": request_id,
            "object": "chat.completion.chunk",
            "model": req.model,
            "choices": [{
                "delta": {},
                "index": 0,
                "finish_reason": "stop"
            }],
            "usage": usage
        });

        yield Ok(Event::default().data(final_chunk.to_string()));
        yield Ok(Event::default().data("[DONE]"));
    })
}

// ===================== NON STREAM =====================
async fn non_stream(
    client: RuntimeClient,
    req: ChatRequest,
) -> AxumResponse {

    let model_id = req.model.replace("bedrock/", "");
    let messages = convert_messages(&req.messages);

    let resp = match client
        .converse()
        .model_id(model_id)
        .set_messages(Some(messages))
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return error("bedrock error", StatusCode::BAD_GATEWAY),
    };

    let mut content = String::new();

    if let Some(ConverseOutput::Message(msg)) = resp.output {
        for block in msg.content {
            if let ContentBlock::Text(t) = block {
                content.push_str(&t);
            }
        }
    }

    let prompt_tokens: u32 = req.messages
        .iter()
        .map(|m| estimate_tokens(&m.content))
        .sum();

    let completion_tokens = estimate_tokens(&content);

    let usage = Usage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
    };

    Json(serde_json::json!({
        "id": Uuid::new_v4().to_string(),
        "object": "chat.completion",
        "model": req.model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": content
            },
            "finish_reason": "stop"
        }],
        "usage": usage
    }))
    .into_response()
}

// ===================== HELPERS =====================
fn estimate_tokens(text: &str) -> u32 {
    (text.len() / 4) as u32
}

fn error(msg: &str, code: StatusCode) -> AxumResponse {
    (code, Json(ErrorResponse { error: msg.into() })).into_response()
}

fn convert_messages(
    messages: &Vec<Message>,
) -> Vec<aws_sdk_bedrockruntime::types::Message> {
    messages
        .iter()
        .map(|m| {
            aws_sdk_bedrockruntime::types::Message::builder()
                .role(match m.role.as_str() {
                    "assistant" => ConversationRole::Assistant,
                    _ => ConversationRole::User,
                })
                .content(ContentBlock::Text(m.content.clone()))
                .build()
                .unwrap()
        })
        .collect()
}
