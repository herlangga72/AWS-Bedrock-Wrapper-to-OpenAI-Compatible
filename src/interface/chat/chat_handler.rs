//! Standard chat handler using Converse API

use crate::domain::chat::ChatRequest;
use crate::domain::logging::ClickHouseLogger;
use crate::infrastructure::bedrock::converse::build_converse_payload;
use crate::infrastructure::cloudflare::CloudflareClient;
use crate::shared::app_state::AppState;
use crate::shared::constants::*;
use crate::shared::errors::{error_response, sse_error};
use crate::shared::logging::spawn_log;

use aws_sdk_bedrockruntime::{
    types::{
        ContentBlock, ContentBlockDelta, ConverseOutput as OutputEnum, ConverseStreamOutput as Out,
    },
    Client as RuntimeClient,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use futures_util::{Stream, StreamExt};
use serde::Serialize;
use std::{
    convert::Infallible,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::timeout;
use uuid::Uuid;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(REQUEST_TIMEOUT_CHAT);

/// Validate request parameters early
fn validate_chat_request(req: &ChatRequest) -> Result<(), String> {
    if req.model.is_empty() {
        return Err("model field cannot be empty".to_string());
    }

    if req.messages.is_empty() {
        return Err("messages array cannot be empty".to_string());
    }

    if let Some(temp) = req.temperature {
        if temp < crate::shared::constants::MIN_TEMPERATURE || temp > crate::shared::constants::MAX_TEMPERATURE {
            return Err(format!(
                "temperature must be between {:.1} and {:.1}, got {:.1}",
                crate::shared::constants::MIN_TEMPERATURE, crate::shared::constants::MAX_TEMPERATURE, temp
            ));
        }
    }

    for (i, msg) in req.messages.iter().enumerate() {
        match msg.role.as_str() {
            "user" | "assistant" | "system" => {}
            other => {
                return Err(format!(
                    "messages[{}].role must be 'user', 'assistant', or 'system', got '{}'",
                    i, other
                ));
            }
        }
    }

    Ok(())
}

/// Normalize model name to show provider clearly in responses
fn normalize_model_name(model: &str) -> String {
    if model.starts_with("@cf/") {
        model.replacen("@cf/", "cloudflare/", 1)
    } else {
        model.replacen("bedrock/", "aws/bedrock/", 1)
    }
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

#[derive(Serialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttft_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_per_second: Option<f64>,
}

#[derive(Serialize)]
pub struct ChatChunk<'a> {
    pub id: &'a str,
    pub object: &'static str,
    pub created: u64,
    pub model: &'a str,
    pub choices: &'a [ChunkChoice<'a>],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Serialize)]
pub struct ChunkChoice<'a> {
    pub index: u32,
    pub delta: ChunkDelta<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Serialize)]
pub struct ChunkDelta<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<&'a str>,
}

pub async fn chat_handler(
    State(state): State<AppState>,
    auth: Option<TypedHeader<Authorization<Bearer>>>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Response {
    // Validate request parameters early
    if let Err(e) = validate_chat_request(&req) {
        return error_response(StatusCode::BAD_REQUEST, &e);
    }

    // Authenticate
    let temp_user_email = match auth {
        Some(TypedHeader(Authorization(bearer))) => {
            match state.auth.authenticate(bearer.token()) {
                Ok(email) => email,
                Err(_) => return error_response(StatusCode::UNAUTHORIZED, "Invalid API Key"),
            }
        }
        None => return error_response(StatusCode::UNAUTHORIZED, "Missing API Key"),
    };

    let user_email = if temp_user_email == "chat" {
        headers
            .get("x-openwebui-user-email")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("anonymous")
            .to_string()
    } else {
        temp_user_email
    };

    let message_id = headers
        .get("x-openwebui-message-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let is_stream = req.stream.unwrap_or(true);

    // Check if this is a Cloudflare model
    let is_cloudflare = req.model.starts_with("@cf/");

    // Normalize model_name to show provider clearly
    let model_name = normalize_model_name(&req.model);

    let model_id = req.model.clone().replace("bedrock/", "");
    let client = Arc::new(state.client.clone());
    let logger = Arc::new(state.logger.clone());
    let cloudflare_client = state.cloudflare_client.clone();

    if is_cloudflare {
        if let Some(cf_client) = cloudflare_client {
            if is_stream {
                let s = stream_cloudflare(
                    cf_client, model_id, model_name, req, logger, user_email, message_id,
                );
                return Sse::new(s).keep_alive(KeepAlive::default()).into_response();
            } else {
                return non_stream_cloudflare(
                    cf_client, model_id, model_name, req, logger, user_email, message_id,
                )
                .await;
            }
        } else {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Cloudflare client not configured",
            )
                .into_response();
        }
    }

    if is_stream {
        let s = stream_converse(
            client, model_id, model_name, req, logger, user_email, message_id,
        );
        Sse::new(s).keep_alive(KeepAlive::default()).into_response()
    } else {
        non_stream(
            client, model_id, model_name, req, logger, user_email, message_id,
        )
        .await
    }
}

fn stream_converse(
    client: Arc<RuntimeClient>,
    model_id: String,
    model_name: String,
    req: ChatRequest,
    logger: Arc<ClickHouseLogger>,
    user_email: String,
    message_id: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let start_time = tokio::time::Instant::now();
    let mut ttft_ms: Option<u64> = None;
    let request_id = message_id;
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

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
            _ => { yield sse_error("Stream failed"); return; }
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
                            model: &model_name,
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
                        model: &model_name,
                        choices: &choices,
                        usage: None
                    };
                    if let Ok(j) = serde_json::to_string(&chunk) {
                        yield Ok(Event::default().data(j));
                    }
                }
                _ => {
                    tracing::debug!("Unknown stream event type: {:?}", event);
                }
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
                model: &model_name,
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
            spawn_log(logger, user_email, model_name, p, c);
        }
        yield Ok(Event::default().data("[DONE]"));
    }
}

async fn non_stream(
    client: Arc<RuntimeClient>,
    model_id: String,
    model_name: String,
    req: ChatRequest,
    logger: Arc<ClickHouseLogger>,
    user_email: String,
    message_id: String,
) -> Response {
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
        Ok(Err(e)) => {
            tracing::error!("Bedrock converse error: {:?}", e);
            return (StatusCode::BAD_GATEWAY, format!("Bedrock Error: {}", e)).into_response();
        }
        Err(_) => return (StatusCode::BAD_GATEWAY, "Request timeout").into_response(),
    };

    let p = resp
        .usage
        .as_ref()
        .map(|u| u.input_tokens as u32)
        .unwrap_or(0);
    let c = resp
        .usage
        .as_ref()
        .map(|u| u.output_tokens as u32)
        .unwrap_or(0);

    let content = resp
        .output
        .and_then(|o| match o {
            OutputEnum::Message(m) => Some(m),
            _ => None,
        })
        .and_then(|m| m.content.into_iter().next())
        .and_then(|cb| match cb {
            ContentBlock::Text(t) => Some(t),
            _ => None,
        })
        .unwrap_or_default();

    spawn_log(logger, user_email, model_name.clone(), p, c);

    let request_id = message_id;
    let total_duration_ms = start_time.elapsed().as_millis() as u64;
    let tps = if total_duration_ms > 0 {
        (c as f64) / (total_duration_ms as f64 / 1000.0)
    } else {
        0.0
    };

    let full_resp = FullResponse {
        id: &request_id,
        object: "chat.completion",
        created: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        model: &model_name,
        choices: [FullChoice {
            index: 0,
            message: FullMessage {
                role: "assistant",
                content: &content,
            },
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

// =============================================================================
// Cloudflare Workers AI handlers
// =============================================================================

fn stream_cloudflare(
    client: CloudflareClient,
    _model_id: String,
    model_name: String,
    req: ChatRequest,
    logger: Arc<ClickHouseLogger>,
    user_email: String,
    message_id: String,
) -> impl Stream<Item = Result<Event, Infallible>> + 'static {
    let start_time = tokio::time::Instant::now();
    let mut ttft_ms: Option<u64> = None;
    let request_id = message_id;
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    async_stream::stream! {
        let cf_resp = match client.chat_streaming(req).await {
            Ok(r) => r,
            Err(e) => {
                yield Ok(Event::default().data(format!(r#"{{"error":"{}"}}"#, e)));
                return;
            }
        };

        let mut stream = cf_resp;
        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    if ttft_ms.is_none() {
                        ttft_ms = Some(start_time.elapsed().as_millis() as u64);
                    }
                    // Cloudflare returns SSE-like format, pass through
                    yield Ok(Event::default().data(chunk));
                }
                Err(e) => {
                    yield Ok(Event::default().data(format!(r#"{{"error":"{}"}}"#, e)));
                }
            }
        }

        yield Ok(Event::default().data("[DONE]"));

        let total_duration_ms = start_time.elapsed().as_millis() as u64;
        if let Some(ttft) = ttft_ms {
            let _gen_time_sec = (total_duration_ms.saturating_sub(ttft) as f64) / 1000.0;
            let tps: f64 = 0.0; // Cloudflare streaming doesn't provide token counts

            let usage_chunk = ChatChunk {
                id: &request_id,
                object: "chat.completion.chunk",
                created,
                model: &model_name,
                choices: &[],
                usage: Some(Usage {
                    input_tokens: 0,
                    output_tokens: 0,
                    total_tokens: 0,
                    ttft_ms: Some(ttft),
                    latency_ms: Some(total_duration_ms),
                    tokens_per_second: Some((tps * 100.0).round() / 100.0),
                })
            };
            if let Ok(j) = serde_json::to_string(&usage_chunk) {
                yield Ok(Event::default().data(j));
            }
            spawn_log(logger, user_email, model_name.clone(), 0, 0);
        }
    }
}

async fn non_stream_cloudflare(
    client: CloudflareClient,
    _model_id: String,
    model_name: String,
    req: ChatRequest,
    logger: Arc<ClickHouseLogger>,
    user_email: String,
    message_id: String,
) -> Response {
    let start_time = tokio::time::Instant::now();
    let cf_resp = match client.chat(req).await {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_GATEWAY, e).into_response(),
    };

    let openai_resp = cf_resp.to_openai_response(&model_name, &message_id);
    let p = openai_resp
        .usage
        .as_ref()
        .map(|u| u.prompt_tokens)
        .unwrap_or(0);
    let c = openai_resp
        .usage
        .as_ref()
        .map(|u| u.completion_tokens)
        .unwrap_or(0);

    let total_duration_ms = start_time.elapsed().as_millis() as u64;
    let tps: f64 = if total_duration_ms > 0 {
        (c as f64) / (total_duration_ms as f64 / 1000.0)
    } else {
        0.0f64
    };

    let full_resp = FullResponse {
        id: &message_id,
        object: "chat.completion",
        created: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        model: &model_name,
        choices: [FullChoice {
            index: 0,
            message: FullMessage {
                role: "assistant",
                content: &openai_resp
                    .choices
                    .first()
                    .map(|c| c.message.content.as_str())
                    .unwrap_or(""),
            },
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

    spawn_log(logger, user_email, model_name.clone(), p, c);

    match serde_json::to_string(&full_resp) {
        Ok(json) => (StatusCode::OK, [("content-type", "application/json")], json).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// =============================================================================
// Tests (no mocking required)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_model_name_cloudflare() {
        assert_eq!(
            normalize_model_name("@cf/meta/llama-3.1-8b-instruct"),
            "cloudflare/meta/llama-3.1-8b-instruct"
        );
        assert_eq!(
            normalize_model_name("@cf/deepseek-ai/deepseek-r1"),
            "cloudflare/deepseek-ai/deepseek-r1"
        );
        assert_eq!(
            normalize_model_name("@cf/google/gemma-2-2b-it"),
            "cloudflare/google/gemma-2-2b-it"
        );
    }

    #[test]
    fn test_normalize_model_name_bedrock() {
        assert_eq!(
            normalize_model_name("bedrock/anthropic.claude-3-5-sonnet-v1:0"),
            "aws/bedrock/anthropic.claude-3-5-sonnet-v1:0"
        );
        assert_eq!(
            normalize_model_name("bedrock/deepseek.r1-v1:0"),
            "aws/bedrock/deepseek.r1-v1:0"
        );
    }

    #[test]
    fn test_normalize_model_name_no_prefix() {
        // Models without bedrock/ prefix stay as-is
        assert_eq!(
            normalize_model_name("anthropic.claude-3-5-sonnet-v1:0"),
            "anthropic.claude-3-5-sonnet-v1:0"
        );
        assert_eq!(normalize_model_name("deepseek.r1-v1:0"), "deepseek.r1-v1:0");
    }

    #[test]
    fn test_normalize_model_name_multiple_bedrock() {
        // replacen() replaces only the first occurrence
        assert_eq!(
            normalize_model_name("bedrock/bedrock/anthropic.claude"),
            "aws/bedrock/bedrock/anthropic.claude"
        );
    }

    #[test]
    fn test_normalize_model_name_empty() {
        assert_eq!(normalize_model_name(""), "");
    }

    #[test]
    fn test_usage_serialization() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 200,
            total_tokens: 300,
            ttft_ms: Some(50),
            latency_ms: Some(150),
            tokens_per_second: Some(133.33),
        };

        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"input_tokens\":100"));
        assert!(json.contains("\"output_tokens\":200"));
        assert!(json.contains("\"total_tokens\":300"));
        assert!(json.contains("\"ttft_ms\":50"));
        assert!(json.contains("\"tokens_per_second\":133"));
    }

    #[test]
    fn test_usage_serialization_optional_fields() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 200,
            total_tokens: 300,
            ttft_ms: None,
            latency_ms: None,
            tokens_per_second: None,
        };

        let json = serde_json::to_string(&usage).unwrap();
        assert!(!json.contains("ttft_ms"));
        assert!(!json.contains("latency_ms"));
        assert!(!json.contains("tokens_per_second"));
    }

    #[test]
    fn test_full_response_serialization() {
        let response = FullResponse {
            id: "test-id",
            object: "chat.completion",
            created: 1234567890,
            model: "aws/bedrock/anthropic.claude-v1",
            choices: [FullChoice {
                index: 0,
                message: FullMessage {
                    role: "assistant",
                    content: "Hello, world!",
                },
                finish_reason: "stop",
            }],
            usage: Usage {
                input_tokens: 10,
                output_tokens: 20,
                total_tokens: 30,
                ttft_ms: Some(100),
                latency_ms: Some(200),
                tokens_per_second: Some(20.0),
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"id\":\"test-id\""));
        assert!(json.contains("\"model\":\"aws/bedrock/anthropic.claude-v1\""));
        assert!(json.contains("\"content\":\"Hello, world!\""));
        assert!(json.contains("\"finish_reason\":\"stop\""));
    }

    #[test]
    fn test_chunk_choice_serialization() {
        let chunk = ChatChunk {
            id: "test-id",
            object: "chat.completion.chunk",
            created: 1234567890,
            model: "test-model",
            choices: &[ChunkChoice {
                index: 0,
                delta: ChunkDelta {
                    content: Some("Hello"),
                },
                finish_reason: None,
            }],
            usage: None,
        };

        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"delta\":{\"content\":\"Hello\"}"));
        assert!(!json.contains("usage")); // None, so skipped
    }

    #[test]
    fn test_validate_chat_request_empty_model() {
        let req = ChatRequest {
            model: "".to_string(),
            messages: vec![crate::domain::chat::Message {
                role: "user".to_string(),
                content: crate::domain::chat::Content::Text("hi".to_string()),
            }],
            stream: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stop_sequences: None,
            frequency_penalty: None,
            presence_penalty: None,
            logit_bias: None,
            user: None,
            top_k: None,
            thinking: None,
        };
        let result = validate_chat_request(&req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "model field cannot be empty");
    }

    #[test]
    fn test_validate_chat_request_empty_messages() {
        let req = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![],
            stream: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stop_sequences: None,
            frequency_penalty: None,
            presence_penalty: None,
            logit_bias: None,
            user: None,
            top_k: None,
            thinking: None,
        };
        let result = validate_chat_request(&req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "messages array cannot be empty");
    }

    #[test]
    fn test_validate_chat_request_invalid_role() {
        let req = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![crate::domain::chat::Message {
                role: "admin".to_string(),
                content: crate::domain::chat::Content::Text("hi".to_string()),
            }],
            stream: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stop_sequences: None,
            frequency_penalty: None,
            presence_penalty: None,
            logit_bias: None,
            user: None,
            top_k: None,
            thinking: None,
        };
        let result = validate_chat_request(&req);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be 'user', 'assistant', or 'system'"));
    }

    #[test]
    fn test_validate_chat_request_temperature_out_of_range() {
        let req = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![crate::domain::chat::Message {
                role: "user".to_string(),
                content: crate::domain::chat::Content::Text("hi".to_string()),
            }],
            stream: None,
            temperature: Some(5.0),
            top_p: None,
            max_tokens: None,
            stop_sequences: None,
            frequency_penalty: None,
            presence_penalty: None,
            logit_bias: None,
            user: None,
            top_k: None,
            thinking: None,
        };
        let result = validate_chat_request(&req);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("temperature must be between"));
    }

    #[test]
    fn test_validate_chat_request_valid() {
        let req = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![
                crate::domain::chat::Message {
                    role: "system".to_string(),
                    content: crate::domain::chat::Content::Text("you are helpful".to_string()),
                },
                crate::domain::chat::Message {
                    role: "user".to_string(),
                    content: crate::domain::chat::Content::Text("hi".to_string()),
                },
            ],
            stream: Some(true),
            temperature: Some(0.7),
            top_p: None,
            max_tokens: Some(100),
            stop_sequences: None,
            frequency_penalty: None,
            presence_penalty: None,
            logit_bias: None,
            user: None,
            top_k: None,
            thinking: None,
        };
        let result = validate_chat_request(&req);
        assert!(result.is_ok());
    }
}
