use crate::models::{ChatRequest};
use crate::handlers::{logger::ClickHouseLogger, message::build_bedrock_payload};

use aws_sdk_bedrockruntime::{
    types::{ContentBlock, ContentBlockDelta, ConverseOutput as OutputEnum, ConverseStreamOutput as Out},
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
use serde::Serialize;
use std::{convert::Infallible, sync::Arc, time::{Duration, SystemTime, UNIX_EPOCH}};
use tokio::time::timeout;
use uuid::Uuid;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

// =========================================================
// ZERO-COPY MODELS (Shared across Stream & Non-Stream)
// =========================================================
#[derive(Serialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
    total_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    ttft_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tokens_per_second: Option<f64>,
}

// Non-Stream Response Structure
#[derive(Serialize)]
struct FullResponse<'a> {
    id: &'a str,
    object: &'static str,
    created: u64,
    model: &'a str,
    choices: [FullChoice<'a>; 1],
    usage: Usage,
}

#[derive(Serialize)]
struct FullChoice<'a> {
    index: u32,
    message: FullMessage<'a>,
    finish_reason: &'static str,
}

#[derive(Serialize)]
struct FullMessage<'a> {
    role: &'static str,
    content: &'a str,
}

// Stream Chunk Structure
#[derive(Serialize)]
struct ChatChunk<'a> {
    id: &'a str,
    object: &'static str,
    created: u64,
    model: &'a str,
    choices: &'a [ChunkChoice<'a>],
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<Usage>,
}

#[derive(Serialize)]
struct ChunkChoice<'a> {
    index: u32,
    delta: ChunkDelta<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finish_reason: Option<String>,
}

#[derive(Serialize)]
struct ChunkDelta<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<&'a str>,
}

// =========================================================
// ENGINE ABSTRACTION
// =========================================================
struct BedrockEngine {
    model_id: String,
    model_name: String,
    payload: crate::handlers::message::BedrockPayload, // Ensure this matches your return type
}

impl BedrockEngine {
    fn new(req: ChatRequest) -> Self {
        Self {
            model_id: req.model.clone().replace("bedrock/", ""),
            model_name: req.model.clone(),
            payload: build_bedrock_payload(req),
        }
    }
}

// =======================
// Main Chat Handler
// =======================
pub async fn chat_handler(
    State(state): State<crate::AppState>,
    auth: Option<TypedHeader<Authorization<Bearer>>>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Response {
// 1. Extract token and authenticate immediately to avoid keeping the header in scope
    let temp_user_email = match auth {
        Some(TypedHeader(Authorization(bearer))) => {
            match state.auth.authenticate(bearer.token()) {
                Ok(email) => email,
                Err(_) => return StatusCode::FORBIDDEN.into_response(),
            }
        }
        None => return (StatusCode::UNAUTHORIZED, "Missing API Key").into_response(),
    };

    // 2. Determine user identity using borrowing
    // Avoid .to_string() until the very last moment or if the downstream requires ownership
    let user_email = if temp_user_email == "chat" {
        headers.get("x-openwebui-user-email")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("anonymous")
            .to_string()
    } else {
        temp_user_email // reuse the String from authenticate()
    };

    // 3. Extract Message ID with fallback
    let message_id = headers.get("x-openwebui-message-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned()) // Preferred over to_string() for explicit intent
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    
    let is_stream = req.stream.unwrap_or(true);
    
    // 4. Zero-cost Engine initialization
    let engine = BedrockEngine::new(req);
    let client = Arc::new(state.client.clone());
    let logger = Arc::new(state.logger.clone());

    if is_stream {
        let s = stream_converse(client, engine, logger, user_email, message_id);
        Sse::new(s).keep_alive(KeepAlive::default()).into_response()
    } else {
        non_stream(client, engine, logger, user_email, message_id).await
    }
}

// =======================
// STREAM MODE
// =======================
fn stream_converse(
    client: Arc<RuntimeClient>,
    engine: BedrockEngine,
    logger: Arc<ClickHouseLogger>,
    user_email: String,
    message_id: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let start_time = tokio::time::Instant::now(); // Start timer
    let mut ttft_ms: Option<u64> = None;


    let request_id = message_id;

    let created = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

    async_stream::stream! {
        let sdk_call = client.converse_stream()
            .model_id(&engine.model_id)
            .set_messages(Some(engine.payload.messages))
            .set_system(engine.payload.system)
            .inference_config(engine.payload.inference_config)
            .send();

        let mut resp = match timeout(REQUEST_TIMEOUT, sdk_call).await {
            Ok(Ok(r)) => r,
            _ => { yield Ok(Event::default().data(r#"{"error":"Stream failed"}"#)); return; }
        };

        let mut metrics = (0u32, 0u32);

        while let Ok(Some(event)) = resp.stream.recv().await {
            match event {
                Out::ContentBlockDelta(delta) => {
                    if ttft_ms.is_none() {
                        ttft_ms = Some(start_time.elapsed().as_millis() as u64);
                    }
                    if let Some(ContentBlockDelta::Text(t)) = delta.delta {
                        let choices = [
                            ChunkChoice { 
                                index: 0, 
                                delta: ChunkDelta { 
                                    content: Some(&t) 
                                }, 
                                finish_reason: None 
                            }];
                        let chunk = ChatChunk { 
                            id: &request_id, 
                            object: "chat.completion.chunk", 
                            created, 
                            model: &engine.model_name, 
                            choices: &choices, 
                            usage: None 
                        };
                        if let Ok(j) = serde_json::to_string(&chunk) { 
                            yield Ok(Event::default().data(j)); 
                        }
                    }
                }
                Out::Metadata(m) => if let Some(u) = m.usage { 
                    metrics = (
                        u.input_tokens as u32, 
                        u.output_tokens as u32
                    ); 
                }
                Out::MessageStop(stop) => {
                    let choices = [
                        ChunkChoice { 
                            index: 0, 
                            delta: ChunkDelta { 
                                content: None 
                            }, 
                            finish_reason: Some(
                                format!("{:?}", stop.stop_reason).to_lowercase()
                            ) 
                        }];
                    let chunk = ChatChunk { 
                        id: &request_id, 
                        object: "chat.completion.chunk", 
                        created, 
                        model: &engine.model_name, 
                        choices: &choices, 
                        usage: None 
                    };
                    if let Ok(j) = serde_json::to_string(&chunk) { 
                        yield Ok(Event::default().data(j)); 
                    }
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

            let usage_chunk = ChatChunk { 
                id: &request_id, 
                object: "chat.completion.chunk", 
                created, 
                model: &engine.model_name, 
                choices: &[], 
                usage: Some(Usage { 
                    input_tokens: p, 
                    output_tokens: c, 
                    total_tokens: p + c,
                    ttft_ms: Some(ttft),
                    latency_ms: Some(total_duration_ms),
                    tokens_per_second: Some((tps * 100.0).round() / 100.0), 
                }) 
            };
            if let Ok(j) = serde_json::to_string(&usage_chunk) { 
                yield Ok(Event::default().data(j)); 
            }
            spawn_log(logger, user_email, engine.model_name, p, c);
        }
        yield Ok(Event::default().data("[DONE]"));
    }
}

// =======================
// NON-STREAM MODE
// =======================
async fn non_stream(
    client: Arc<RuntimeClient>,
    engine: BedrockEngine,
    logger: Arc<ClickHouseLogger>,
    user_email: String,
    message_id: String,
) -> Response {
    let start_time = tokio::time::Instant::now();

    let sdk_call = client.converse()
        .model_id(&engine.model_id)
        .set_messages(Some(engine.payload.messages))
        .set_system(engine.payload.system)
        .inference_config(engine.payload.inference_config)
        .send();

    let resp = match timeout(REQUEST_TIMEOUT, sdk_call).await {
        Ok(Ok(output)) => output,
        _ => return (StatusCode::BAD_GATEWAY, "Bedrock Error").into_response(),
    };

    let p = resp.usage.as_ref().map(|u| u.input_tokens as u32).unwrap_or(0);
    let c = resp.usage.as_ref().map(|u| u.output_tokens as u32).unwrap_or(0);

    let content = resp.output
        .and_then(|o| match o { OutputEnum::Message(m) => Some(m), _ => None })
        .and_then(|m| m.content.into_iter().next())
        .and_then(|cb| match cb { ContentBlock::Text(t) => Some(t), _ => None })
        .unwrap_or_default();

    spawn_log(logger, user_email, engine.model_name.clone(), p, c);

    let request_id = message_id;

    let total_duration_ms = start_time.elapsed().as_millis() as u64;

    let (p, c) = (resp.usage.as_ref().map(|u| u.input_tokens as u32).unwrap_or(0), 
                  resp.usage.as_ref().map(|u| u.output_tokens as u32).unwrap_or(0));

    let tps = if total_duration_ms > 0 { (c as f64) / (total_duration_ms as f64 / 1000.0) } else { 0.0 };
    
    let full_resp = FullResponse {
        id: &request_id,
        object: "chat.completion",
        created: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        model: &engine.model_name,
        choices: [FullChoice {
            index: 0,
            message: FullMessage { role: "assistant", content: &content },
            finish_reason: "stop",
        }],
        usage: Usage { 
            input_tokens: p, 
            output_tokens: c, 
            total_tokens: p + c,
            ttft_ms: Some(total_duration_ms), 
            latency_ms: Some(total_duration_ms),
            tokens_per_second: Some((tps * 100.0).round() / 100.0),
        },
    };

    match serde_json::to_string(&full_resp) {
        Ok(json) => (StatusCode::OK, [("content-type", "application/json")], json).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// Shared Helper
fn spawn_log(logger: Arc<ClickHouseLogger>, email: String, model: String, p: u32, c: u32) {
    tokio::spawn(async move { let _ = logger.log_usage(&email, &model, p, c); });
}