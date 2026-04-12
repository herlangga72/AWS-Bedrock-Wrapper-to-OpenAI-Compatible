//! Extended thinking handler for Claude models on AWS Bedrock
//! Routes to reasoning handler for DeepSeek R1

use crate::domain::chat::{get_model_capabilities, ChatRequest, Content, ContentBlock};
use crate::domain::logging::ClickHouseLogger;
use crate::infrastructure::bedrock::invoke::{
    build_thinking_request, invoke_thinking_model, parse_thinking_params,
};
use crate::interface::chat::chat_handler;
use crate::interface::chat::reasoning_handler::chat_with_reasoning_handler;
use crate::shared::app_state::AppState;
use crate::shared::logging::spawn_log;

use aws_sdk_bedrockruntime::Client as RuntimeClient;
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
use serde::Serialize;
use std::{
    convert::Infallible,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

use super::chat_handler::Usage;

// Response builders
#[derive(Serialize)]
struct ThinkingFullResponse {
    id: String,
    object: &'static str,
    created: u64,
    model: String,
    choices: [ThinkingChoice; 1],
    usage: Usage,
}

#[derive(Serialize)]
struct ThinkingChoice {
    index: u32,
    message: ThinkingMessageResponse,
    finish_reason: &'static str,
}

#[derive(Serialize)]
struct ThinkingMessageResponse {
    role: &'static str,
    content: Content,
}

pub async fn chat_with_thinking_handler(
    State(state): State<AppState>,
    auth: Option<TypedHeader<Authorization<Bearer>>>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> Response {
    let model_id = req.model.clone();

    // Check if model supports reasoning (DeepSeek R1)
    if model_supports_reasoning(&model_id) {
        return chat_with_reasoning_handler(State(state), auth, headers, Json(req)).await;
    }

    // Check if model supports thinking (Claude extended thinking)
    if !model_supports_thinking(&model_id) {
        return chat_handler(State(state), auth, headers, Json(req)).await;
    }

    let thinking_enabled = headers
        .get("x-openwebui-thinking")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    let enabled = req
        .thinking
        .as_ref()
        .map(|t| t.enabled.unwrap_or(false))
        .unwrap_or(false);

    if !thinking_enabled && !enabled {
        return chat_handler(State(state), auth, headers, Json(req)).await;
    }

    thinking_handler(state, auth, headers, Json(req)).await
}

fn model_supports_thinking(model_id: &str) -> bool {
    get_model_capabilities(model_id)
        .map(|c| c.supports_thinking)
        .unwrap_or(false)
}

fn model_supports_reasoning(model_id: &str) -> bool {
    get_model_capabilities(model_id)
        .map(|c| c.supports_reasoning)
        .unwrap_or(false)
}

async fn thinking_handler(
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
    let client = Arc::new(state.client.clone());
    let logger = Arc::new(state.logger.clone());

    if is_stream {
        let s = non_stream_as_stream(client, req, logger, user_email, message_id);
        Sse::new(s).keep_alive(KeepAlive::default()).into_response()
    } else {
        non_stream_thinking(client, req, logger, user_email, message_id).await
    }
}

async fn non_stream_thinking(
    client: Arc<RuntimeClient>,
    req: ChatRequest,
    logger: Arc<ClickHouseLogger>,
    user_email: String,
    message_id: String,
) -> Response {
    let model_id = req.model.clone().replace("bedrock/", "");
    let start_time = tokio::time::Instant::now();

    let (max_tokens, budget_tokens) = parse_thinking_params(&req);
    let body = build_thinking_request(&req, max_tokens, budget_tokens);

    let resp = match invoke_thinking_model(&client, &model_id, &body).await {
        Ok(r) => r,
        Err(_) => return (StatusCode::BAD_GATEWAY, "Bedrock Thinking Error").into_response(),
    };

    let mut thinking_text = String::new();
    let mut response_text = String::new();

    for block in &resp.content {
        match block.r#type.as_str() {
            "thinking" => {
                if let Some(t) = &block.thinking {
                    thinking_text.push_str(t);
                }
            }
            "text" => {
                if let Some(t) = &block.text {
                    response_text.push_str(t);
                }
            }
            _ => {}
        }
    }

    let total_duration_ms = start_time.elapsed().as_millis() as u64;
    let model_name = req.model.clone();

    let usage = Usage {
        input_tokens: resp.usage.input_tokens,
        output_tokens: resp.usage.output_tokens,
        total_tokens: resp.usage.total_tokens,
        ttft_ms: Some(total_duration_ms),
        latency_ms: Some(total_duration_ms),
        tokens_per_second: None,
    };

    spawn_log(
        logger,
        user_email,
        model_name.clone(),
        resp.usage.input_tokens,
        resp.usage.output_tokens,
    );

    let full_resp = ThinkingFullResponse {
        id: message_id,
        object: "chat.completion",
        created: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        model: model_name,
        choices: [ThinkingChoice {
            index: 0,
            message: ThinkingMessageResponse {
                role: "assistant",
                content: Content::Blocks(vec![
                    ContentBlock {
                        r#type: "thinking".to_string(),
                        text: None,
                        thinking: Some(thinking_text),
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
        usage,
    };

    match serde_json::to_string(&full_resp) {
        Ok(json) => (StatusCode::OK, [("content-type", "application/json")], json).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn non_stream_as_stream(
    client: Arc<RuntimeClient>,
    req: ChatRequest,
    logger: Arc<ClickHouseLogger>,
    user_email: String,
    _message_id: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let model_id = req.model.clone().replace("bedrock/", "");
    let model_name = req.model.clone();

    let (max_tokens, budget_tokens) = parse_thinking_params(&req);

    async_stream::stream! {
        // Open Web UI format: thinking block start
        yield Ok(Event::default().data(r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}"#));

        let body = build_thinking_request(&req, max_tokens, budget_tokens);
        let resp = match invoke_thinking_model(&client, &model_id, &body).await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Thinking invoke failed: {e}");
                yield Ok(Event::default().data(r#"{"error":"Thinking stream failed"}"#));
                return;
            }
        };

        let mut thinking_text = String::new();
        let mut response_text = String::new();

        for block in &resp.content {
            match block.r#type.as_str() {
                "thinking" => {
                    if let Some(t) = &block.thinking {
                        thinking_text.push_str(t);
                    }
                }
                "text" => {
                    if let Some(t) = &block.text {
                        response_text.push_str(t);
                    }
                }
                _ => {}
            }
        }

        // Send thinking content if any
        if !thinking_text.is_empty() {
            yield Ok(Event::default().data(serde_json::json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": {
                    "type": "thinking_delta",
                    "thinking": thinking_text
                }
            }).to_string()));
        }

        // End thinking block
        yield Ok(Event::default().data(r#"{"type":"content_block_stop","index":0}"#));

        // Start text block
        yield Ok(Event::default().data(r#"{"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}"#));

        // Send text content if any
        if !response_text.is_empty() {
            yield Ok(Event::default().data(serde_json::json!({
                "type": "content_block_delta",
                "index": 1,
                "delta": {
                    "type": "text_delta",
                    "text": response_text
                }
            }).to_string()));
        }

        // End text block
        yield Ok(Event::default().data(r#"{"type":"content_block_stop","index":1}"#));

        // Final message delta with usage
        yield Ok(Event::default().data(serde_json::json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": "end_turn"
            },
            "usage": {
                "output_tokens": resp.usage.output_tokens,
                "completion_tokens": resp.usage.output_tokens,
                "total_tokens": resp.usage.total_tokens
            }
        }).to_string()));

        spawn_log(logger, user_email, model_name, resp.usage.input_tokens, resp.usage.output_tokens);

        yield Ok(Event::default().data("[DONE]"));
    }
}
