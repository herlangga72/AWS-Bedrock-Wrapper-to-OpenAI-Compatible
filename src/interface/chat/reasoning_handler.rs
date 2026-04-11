//! Reasoning handler for DeepSeek R1 and similar models on AWS Bedrock
//! Uses Converse API but processes reasoningContent from responses

use crate::domain::chat::{get_model_capabilities, ChatRequest, Content, ContentBlock};
use crate::domain::logging::ClickHouseLogger;
use crate::infrastructure::bedrock::converse::build_converse_payload;
use crate::shared::app_state::AppState;

use aws_sdk_bedrockruntime::{
    types::{ContentBlock as BContentBlock, ContentBlockDelta, ConverseOutput as OutputEnum, ConversationRole, InferenceConfiguration, Message as BedrockMessage, SystemContentBlock},
    Client as RuntimeClient,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{sse::{Event, KeepAlive, Sse}, IntoResponse, Response},
    Json,
};
use axum_extra::{headers::{authorization::Bearer, Authorization}, TypedHeader};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc, time::{Duration, SystemTime, UNIX_EPOCH}};
use tokio::time::timeout;
use uuid::Uuid;

use super::chat_handler::Usage;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

// Response structure for reasoning models
#[derive(Deserialize, Debug)]
struct ReasoningResponse {
    content: Vec<ReasoningContentBlock>,
    stop_reason: String,
    usage: ReasoningUsage,
}

#[derive(Deserialize, Debug)]
struct ReasoningContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    reasoning_content: Option<ReasoningText>,
}

#[derive(Deserialize, Debug)]
struct ReasoningText {
    reasoning_text: String,
}

#[derive(Deserialize, Debug)]
struct ReasoningUsage {
    input_tokens: u32,
    output_tokens: u32,
    total_tokens: u32,
}

// Response builders
#[derive(Serialize)]
struct ReasoningFullResponse {
    id: String,
    object: &'static str,
    created: u64,
    model: String,
    choices: [ReasoningChoice; 1],
    usage: ReasoningUsageOutput,
}

#[derive(Serialize)]
struct ReasoningChoice {
    index: u32,
    message: ReasoningMessage,
    finish_reason: &'static str,
}

#[derive(Serialize)]
struct ReasoningMessage {
    role: &'static str,
    content: Content,
}

#[derive(Serialize)]
struct ReasoningUsageOutput {
    input_tokens: u32,
    output_tokens: u32,
    total_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    completion_tokens: Option<u32>,
}

pub async fn chat_with_reasoning_handler(
    State(state): State<AppState>,
    auth: Option<TypedHeader<Authorization<Bearer>>>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Response {
    reasoning_handler(state, auth, headers, Json(req)).await
}

async fn reasoning_handler(
    state: AppState,
    auth: Option<TypedHeader<Authorization<Bearer>>>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Response {
    let temp_user_email = match auth {
        Some(TypedHeader(Authorization(bearer))) => match state.auth.authenticate(bearer.token()) {
            Ok(email) => email,
            Err(_) => return StatusCode::FORBIDDEN.into_response(),
        },
        None => return (StatusCode::UNAUTHORIZED, "Missing API Key").into_response(),
    };

    let user_email = if temp_user_email == "chat" {
        headers.get("x-openwebui-user-email").and_then(|v| v.to_str().ok()).unwrap_or("anonymous").to_string()
    } else {
        temp_user_email
    };

    let message_id = headers.get("x-openwebui-message-id").and_then(|v| v.to_str().ok()).map(|s| s.to_owned()).unwrap_or_else(|| Uuid::new_v4().to_string());
    let is_stream = req.stream.unwrap_or(true);
    let client = Arc::new(state.client.clone());
    let logger = Arc::new(state.logger.clone());

    let include_reasoning = headers
        .get("x-openwebui-reasoning")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if is_stream {
        let s = stream_reasoning(client, req, logger, user_email, message_id, include_reasoning);
        Sse::new(s).keep_alive(KeepAlive::default()).into_response()
    } else {
        non_stream_reasoning(client, req, logger, user_email, message_id, include_reasoning).await
    }
}

async fn non_stream_reasoning(
    client: Arc<RuntimeClient>,
    req: ChatRequest,
    logger: Arc<ClickHouseLogger>,
    user_email: String,
    message_id: String,
    include_reasoning: bool,
) -> Response {
    let model_id = req.model.clone().replace("bedrock/", "");
    let model_name = req.model.clone();
    let start_time = tokio::time::Instant::now();

    let payload = build_converse_payload(&req);

    let sdk_call = client
        .converse()
        .model_id(&model_id)
        .set_messages(Some(payload.messages))
        .set_system(payload.system)
        .inference_config(payload.inference_config)
        .send();

    let resp = match timeout(REQUEST_TIMEOUT, sdk_call).await {
        Ok(Ok(output)) => output,
        _ => return (StatusCode::BAD_GATEWAY, "Bedrock Reasoning Error").into_response(),
    };

    let p = resp.usage.as_ref().map(|u| u.input_tokens as u32).unwrap_or(0);
    let c = resp.usage.as_ref().map(|u| u.output_tokens as u32).unwrap_or(0);

    let mut response_text = String::new();
    let mut reasoning_text = String::new();

    if let Some(output) = resp.output {
        if let OutputEnum::Message(msg) = output {
            for block in msg.content {
                match block {
                    BContentBlock::Text(t) => response_text.push_str(&t),
                    BContentBlock::ReasoningContent(r) => {
                        if let Ok(rt) = r.as_reasoning_text() {
                            reasoning_text.push_str(&rt.text);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let _total_duration_ms = start_time.elapsed().as_millis() as u64;

    spawn_log(logger, user_email, model_name.clone(), p, c);

    let full_resp = if include_reasoning && !reasoning_text.is_empty() {
        ReasoningFullResponse {
            id: message_id,
            object: "chat.completion",
            created: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            model: model_name,
            choices: [ReasoningChoice {
                index: 0,
                message: ReasoningMessage {
                    role: "assistant",
                    content: Content::Blocks(vec![
                        ContentBlock {
                            r#type: "reasoning".to_string(),
                            text: None,
                            thinking: Some(reasoning_text),
                            signature: None,
                            reasoning_content: None,
                        },
                        ContentBlock {
                            r#type: "text".to_string(),
                            text: Some(response_text),
                            thinking: None,
                            signature: None,
                            reasoning_content: None,
                        },
                    ]),
                },
                finish_reason: "stop",
            }],
            usage: ReasoningUsageOutput {
                input_tokens: p,
                output_tokens: c,
                total_tokens: p + c,
                completion_tokens: Some(c),
            },
        }
    } else {
        ReasoningFullResponse {
            id: message_id,
            object: "chat.completion",
            created: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            model: model_name,
            choices: [ReasoningChoice {
                index: 0,
                message: ReasoningMessage {
                    role: "assistant",
                    content: Content::Text(response_text),
                },
                finish_reason: "stop",
            }],
            usage: ReasoningUsageOutput {
                input_tokens: p,
                output_tokens: c,
                total_tokens: p + c,
                completion_tokens: Some(c),
            },
        }
    };

    match serde_json::to_string(&full_resp) {
        Ok(json) => (StatusCode::OK, [("content-type", "application/json")], json).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn stream_reasoning(
    client: Arc<RuntimeClient>,
    req: ChatRequest,
    logger: Arc<ClickHouseLogger>,
    user_email: String,
    _message_id: String,
    _include_reasoning: bool,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let model_id = req.model.clone().replace("bedrock/", "");
    let model_name = req.model.clone();
    let start_time = tokio::time::Instant::now();
    let request_id = _message_id;

    let created = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

    let payload = build_converse_payload(&req);

    async_stream::stream! {
        let sdk_call = client.converse_stream()
            .model_id(&model_id)
            .set_messages(Some(payload.messages))
            .set_system(payload.system)
            .inference_config(payload.inference_config)
            .send();

        let mut resp = match timeout(REQUEST_TIMEOUT, sdk_call).await {
            Ok(Ok(r)) => r,
            _ => { yield Ok(Event::default().data(r#"{"error":"Reasoning stream failed"}"#)); return; }
        };

        let mut ttft_ms: Option<u64> = None;
        let mut metrics = (0u32, 0u32);
        let _reasoning_text = String::new();
        let mut text_accumulated = String::new();

        while let Ok(Some(event)) = resp.stream.recv().await {
            match event {
                aws_sdk_bedrockruntime::types::ConverseStreamOutput::ContentBlockDelta(delta) => {
                    if ttft_ms.is_none() {
                        ttft_ms = Some(start_time.elapsed().as_millis() as u64);
                    }
                    if let Some(ContentBlockDelta::Text(t)) = delta.delta {
                        text_accumulated.push_str(&t);

                        let chunk = serde_json::json!({
                            "id": request_id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": model_name,
                            "choices": [{
                                "index": 0,
                                "delta": { "content": t },
                                "finish_reason": null
                            }]
                        });
                        yield Ok(Event::default().data(serde_json::to_string(&chunk).unwrap_or_default()));
                    }
                }
                aws_sdk_bedrockruntime::types::ConverseStreamOutput::Metadata(m) => if let Some(u) = m.usage {
                    metrics = (u.input_tokens as u32, u.output_tokens as u32);
                }
                aws_sdk_bedrockruntime::types::ConverseStreamOutput::MessageStop(stop) => {
                    let chunk = serde_json::json!({
                        "id": request_id,
                        "object": "chat.completion.chunk",
                        "created": created,
                        "model": model_name,
                        "choices": [{
                            "index": 0,
                            "delta": { "content": null },
                            "finish_reason": format!("{:?}", stop.stop_reason).to_lowercase()
                        }]
                    });
                    yield Ok(Event::default().data(serde_json::to_string(&chunk).unwrap_or_default()));
                }
                _ => {}
            }
        }

        let total_duration_ms = start_time.elapsed().as_millis() as u64;
        let (p, c) = metrics;

        if p > 0 || c > 0 {
            let ttft = ttft_ms.unwrap_or(total_duration_ms);
            let gen_time_sec = (total_duration_ms.saturating_sub(ttft) as f64) / 1000.0;
            let tps = if gen_time_sec > 0.0 { c as f64 / gen_time_sec } else { 0.0 };

            let usage_chunk = serde_json::json!({
                "id": request_id,
                "object": "chat.completion.chunk",
                "created": created,
                "model": model_name,
                "choices": [],
                "usage": {
                    "input_tokens": p,
                    "output_tokens": c,
                    "total_tokens": p + c,
                    "completion_tokens": c,
                    "ttft_ms": ttft,
                    "latency_ms": total_duration_ms,
                    "tokens_per_second": (tps * 100.0).round() / 100.0
                }
            });
            yield Ok(Event::default().data(serde_json::to_string(&usage_chunk).unwrap_or_default()));
            spawn_log(logger, user_email, model_name, p, c);
        }

        yield Ok(Event::default().data("[DONE]"));
    }
}

fn spawn_log(logger: Arc<ClickHouseLogger>, email: String, model: String, p: u32, c: u32) {
    tokio::spawn(async move {
        let _ = logger.log_usage(&email, &model, p, c);
    });
}
