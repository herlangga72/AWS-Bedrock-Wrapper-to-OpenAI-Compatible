//! Anthropic /v1/messages handler
//!
//! Accepts Anthropic native API format and translates to AWS Bedrock Converse API.
//! Returns responses in Anthropic native format.

use aws_sdk_bedrock::primitives::Blob;
use aws_sdk_bedrockruntime::{
    types::{ContentBlockDelta, ConverseOutput as OutputEnum, ConverseStreamOutput as Out},
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
use futures_util::Stream;
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

use crate::domain::chat::anthropic_types::{
    AnthropicMessagesRequest, AnthropicReasoningBlock, AnthropicResponse,
    AnthropicResponseBlock, AnthropicUsage,
};
use crate::infrastructure::bedrock::invoke::ThinkingResponse;
use crate::domain::logging::ClickHouseLogger;
use crate::infrastructure::anthropic_translator::{
    anthropic_model_to_bedrock, build_thinking_request_from_anthropic, ConversePayload,
};
use crate::shared::app_state::AppState;
use crate::shared::constants::REQUEST_TIMEOUT_CHAT;
use crate::shared::errors::{error_response, sse_error};
use crate::shared::logging::spawn_log;

const REQUEST_TIMEOUT: Duration = std::time::Duration::from_secs(REQUEST_TIMEOUT_CHAT);

/// POST /claude/v1/messages — Anthropic native messages endpoint
pub async fn claude_messages_handler(
    State(state): State<AppState>,
    auth: Option<TypedHeader<Authorization<Bearer>>>,
    headers: HeaderMap,
    Json(req): Json<AnthropicMessagesRequest>,
) -> Response {
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

    let message_id = Uuid::new_v4().to_string();
    let model_id = anthropic_model_to_bedrock(&req.model);

    let client = Arc::new(state.client.clone());
    let logger = Arc::new(state.logger.clone());
    let is_stream = req.stream;

    // Route to thinking path if thinking is enabled
    let is_thinking = req
        .thinking
        .as_ref()
        .is_some_and(|t| t.thinking_type.as_deref() == Some("enabled"));

    if is_thinking {
        if is_stream {
            let s = stream_thinking(
                client,
                req,
                logger,
                user_email,
                message_id,
            );
            return Sse::new(s).keep_alive(KeepAlive::default()).into_response();
        } else {
            return non_stream_thinking(
                state.client,
                req,
                user_email,
                message_id,
                logger,
            )
            .await;
        }
    }

    if is_stream {
        let s = stream_converse(
            client,
            model_id,
            req,
            logger,
            user_email,
            message_id,
        );
        Sse::new(s).keep_alive(KeepAlive::default()).into_response()
    } else {
        non_stream(state.client, req, user_email, message_id, logger).await
    }
}

// =============================================================================
// Non-streaming (Converse API)
// =============================================================================

async fn non_stream(
    client: RuntimeClient,
    req: AnthropicMessagesRequest,
    user_email: String,
    message_id: String,
    logger: Arc<ClickHouseLogger>,
) -> Response {
    let model_id = anthropic_model_to_bedrock(&req.model);
    let payload = ConversePayload::from_anthropic_request(&req);

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

    let input_tokens = resp
        .usage
        .as_ref()
        .map(|u| u.input_tokens as u32)
        .unwrap_or(0);
    let output_tokens = resp
        .usage
        .as_ref()
        .map(|u| u.output_tokens as u32)
        .unwrap_or(0);

    let anthropic_resp = build_anthropic_response(
        resp.output,
        &model_id,
        &message_id,
        input_tokens,
        output_tokens,
    );

    spawn_log(logger, user_email, model_id.clone(), input_tokens, output_tokens);

    match serde_json::to_string(&anthropic_resp) {
        Ok(json) => (
            StatusCode::OK,
            [("content-type", "application/json")],
            json,
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// =============================================================================
// Streaming (Converse API)
// =============================================================================

fn stream_converse(
    client: Arc<RuntimeClient>,
    model_id: String,
    req: AnthropicMessagesRequest,
    logger: Arc<ClickHouseLogger>,
    user_email: String,
    _message_id: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let payload = ConversePayload::from_anthropic_request(&req);
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

        let mut output_tokens: u32 = 0;
        let index: usize = 0;
        let mut sent_content_block_start = false;

        while let Ok(Some(event)) = resp.stream.recv().await {
            match event {
                Out::ContentBlockDelta(delta) => {
                    if let Some(ContentBlockDelta::Text(t)) = delta.delta {
                        if !sent_content_block_start {
                            let start_event = json!({
                                "type": "content_block_start",
                                "index": index,
                                "content_block": { "type": "text", "text": "" }
                            });
                            yield Ok(Event::default().data(start_event.to_string()));
                            sent_content_block_start = true;
                        }
                        let delta_event = json!({
                            "type": "content_block_delta",
                            "index": index,
                            "delta": { "type": "text_delta", "text": t }
                        });
                        yield Ok(Event::default().data(delta_event.to_string()));
                    }
                }
                Out::Metadata(m) => {
                    if let Some(u) = m.usage {
                        output_tokens = u.output_tokens as u32;
                    }
                }
                Out::MessageStop(stop) => {
                    let stop_event = json!({ "type": "content_block_stop", "index": index });
                    yield Ok(Event::default().data(stop_event.to_string()));

                    let stop_reason = format!("{:?}", stop.stop_reason).to_lowercase();
                    let delta_event = json!({
                        "type": "message_delta",
                        "delta": { "stop_reason": stop_reason },
                        "usage": { "output_tokens": output_tokens, "total_tokens": output_tokens }
                    });
                    yield Ok(Event::default().data(delta_event.to_string()));

                    spawn_log(logger.clone(), user_email.clone(), model_id.clone(), 0, output_tokens);
                }
                _ => {
                    tracing::debug!("Unknown stream event: {:?}", event);
                }
            }
        }

        yield Ok(Event::default().data("[DONE]"));
    }
}

// =============================================================================
// Thinking (Invoke API)
// =============================================================================

async fn non_stream_thinking(
    client: RuntimeClient,
    req: AnthropicMessagesRequest,
    user_email: String,
    message_id: String,
    logger: Arc<ClickHouseLogger>,
) -> Response {
    let body = build_thinking_request_from_anthropic(&req);
    let body_json = match serde_json::to_string(&body) {
        Ok(j) => j,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let model_id = anthropic_model_to_bedrock(&req.model);
    let invoke = client
        .invoke_model()
        .model_id(&model_id)
        .content_type("application/json")
        .accept("application/json")
        .body(Blob::new(body_json));

    let resp = match timeout(REQUEST_TIMEOUT, invoke.send()).await {
        Ok(Ok(r)) => r,
        _ => return (StatusCode::BAD_GATEWAY, "Thinking request failed").into_response(),
    };

    let body_bytes = resp.body().as_ref();

    let think_resp: ThinkingResponse = match serde_json::from_slice(body_bytes) {
        Ok(r) => r,
        Err(_) => return (StatusCode::BAD_GATEWAY, "Invalid thinking response").into_response(),
    };

    let input_tokens = think_resp.usage.input_tokens;
    let output_tokens = think_resp.usage.output_tokens;

    let mut content_blocks = Vec::new();
    for block in think_resp.content {
        match block.r#type.as_str() {
            "thinking" => {
                if let Some(t) = block.thinking {
                    content_blocks.push(AnthropicResponseBlock {
                        block_type: "thinking".to_string(),
                        text: None,
                        id: None,
                        name: None,
                        content: None,
                        reasoning: Some(AnthropicReasoningBlock {
                            type_field: None,
                            thinking: Some(t),
                        }),
                    });
                }
            }
            "text" => {
                if let Some(t) = block.text {
                    content_blocks.push(AnthropicResponseBlock {
                        block_type: "text".to_string(),
                        text: Some(t),
                        id: None,
                        name: None,
                        content: None,
                        reasoning: None,
                    });
                }
            }
            _ => {}
        }
    }

    if content_blocks.is_empty() {
        content_blocks.push(AnthropicResponseBlock {
            block_type: "text".to_string(),
            text: Some(String::new()),
            id: None,
            name: None,
            content: None,
            reasoning: None,
        });
    }

    let anthropic_resp = AnthropicResponse {
        id: format!("msg_{}", &message_id[..8.min(message_id.len())]),
        response_type: "message".to_string(),
        role: "assistant".to_string(),
        content: content_blocks,
        model: req.model.clone(),
        stop_reason: "end_turn".to_string(),
        stop_sequence: None,
        usage: AnthropicUsage::new(input_tokens, output_tokens),
    };

    spawn_log(logger, user_email, model_id.clone(), input_tokens, output_tokens);

    match serde_json::to_string(&anthropic_resp) {
        Ok(json) => (
            StatusCode::OK,
            [("content-type", "application/json")],
            json,
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn stream_thinking(
    client: Arc<RuntimeClient>,
    req: AnthropicMessagesRequest,
    logger: Arc<ClickHouseLogger>,
    user_email: String,
    _message_id: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let model_id = anthropic_model_to_bedrock(&req.model);
    let body = build_thinking_request_from_anthropic(&req);
    async_stream::stream! {
        let body_json = match serde_json::to_string(&body) {
            Ok(j) => j,
            Err(_) => { yield sse_error("Failed to serialize thinking request"); return; }
        };

        let invoke = client
            .invoke_model()
            .model_id(&model_id)
            .content_type("application/json")
            .accept("application/x-user-visible-beam-output")
            .body(Blob::new(body_json));

        let resp = match timeout(REQUEST_TIMEOUT, invoke.send()).await {
            Ok(Ok(r)) => r,
            _ => { yield sse_error("Thinking stream failed"); return; }
        };

        let body_bytes = resp.body().as_ref();

        // Parse as lines — each line is a JSON event
        let text = String::from_utf8_lossy(body_bytes);
        let thinking_event = serde_json::json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": { "type": "thinking", "thinking": "" }
        });
        yield Ok(Event::default().data(thinking_event.to_string()));

        let mut thinking_acc = String::new();
        let mut text_acc = String::new();
        let mut output_tokens: u32 = 0;

        // Try to parse as SSE lines
        for line in text.lines() {
            if line.starts_with("data: ") {
                let data = &line[6..];
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                    let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match event_type {
                        "content_block_delta" => {
                            let delta = event.get("delta");
                            if let Some(d) = delta {
                                if let Some(t) = d.get("thinking").and_then(|v| v.as_str()) {
                                    thinking_acc.push_str(t);
                                    let delta_event = serde_json::json!({
                                        "type": "content_block_delta",
                                        "index": 0,
                                        "delta": { "type": "thinking_delta", "thinking": t }
                                    });
                                    yield Ok(Event::default().data(delta_event.to_string()));
                                } else if let Some(t) = d.get("text").and_then(|v| v.as_str()) {
                                    text_acc.push_str(t);
                                }
                            }
                        }
                        "message_stop" => {
                            if let Some(usage) = event.get("usage") {
                                output_tokens = usage.get("output_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0) as u32;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // End thinking block
        yield Ok(Event::default().data(serde_json::json!({"type": "content_block_stop", "index": 0}).to_string()));

        // Start text block
        yield Ok(Event::default().data(serde_json::json!({
            "type": "content_block_start",
            "index": 1,
            "content_block": { "type": "text", "text": "" }
        }).to_string()));

        // Send accumulated text
        if !text_acc.is_empty() {
            yield Ok(Event::default().data(serde_json::json!({
                "type": "content_block_delta",
                "index": 1,
                "delta": { "type": "text_delta", "text": text_acc }
            }).to_string()));
        }

        // End text block
        yield Ok(Event::default().data(serde_json::json!({"type": "content_block_stop", "index": 1}).to_string()));

        // Message delta
        yield Ok(Event::default().data(serde_json::json!({
            "type": "message_delta",
            "delta": { "stop_reason": "end_turn" },
            "usage": { "output_tokens": output_tokens, "total_tokens": output_tokens }
        }).to_string()));

        spawn_log(logger.clone(), user_email.clone(), model_id.clone(), 0, output_tokens);
        yield Ok(Event::default().data("[DONE]"));
    }
}

// =============================================================================
// Response builders
// =============================================================================

fn build_anthropic_response(
    output: Option<OutputEnum>,
    model: &str,
    message_id: &str,
    input_tokens: u32,
    output_tokens: u32,
) -> AnthropicResponse {
    let mut content_blocks = Vec::new();

    if let Some(OutputEnum::Message(msg)) = output {
        for block in msg.content {
            if let aws_sdk_bedrockruntime::types::ContentBlock::Text(t) = block {
                content_blocks.push(AnthropicResponseBlock {
                    block_type: "text".to_string(),
                    text: Some(t.to_string()),
                    id: None,
                    name: None,
                    content: None,
                    reasoning: None,
                });
            }
        }
    }

    if content_blocks.is_empty() {
        content_blocks.push(AnthropicResponseBlock {
            block_type: "text".to_string(),
            text: Some(String::new()),
            id: None,
            name: None,
            content: None,
            reasoning: None,
        });
    }

    AnthropicResponse {
        id: format!("msg_{}", &message_id[..8.min(message_id.len())]),
        response_type: "message".to_string(),
        role: "assistant".to_string(),
        content: content_blocks,
        model: model.to_string(),
        stop_reason: "end_turn".to_string(),
        stop_sequence: None,
        usage: AnthropicUsage::new(input_tokens, output_tokens),
    }
}
