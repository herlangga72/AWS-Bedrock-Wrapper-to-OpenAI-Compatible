use crate::models::{ChatRequest, Message, Usage, ErrorResponse};
use crate::handlers::logger::ClickHouseLogger;
use axum::{
    extract::State,
    http::StatusCode,
    http::HeaderMap,
    Json,
    response::{IntoResponse, Response as AxumResponse},
    response::sse::{Event, Sse, KeepAlive},
};
use axum_extra::{TypedHeader, headers::{authorization::Bearer, Authorization}};
use async_stream::stream;
use aws_sdk_bedrockruntime::{types::*, Client as RuntimeClient};
use futures_util::Stream;
use serde_json::json;
use std::{convert::Infallible, time::Duration};
use tokio::time::timeout;
use uuid::Uuid;

// =======================
// Main Handler
// =======================
pub async fn chat_handler(
    State(state): State<crate::AppState>,
    auth: Option<TypedHeader<Authorization<Bearer>>>,
    headers: HeaderMap,
    Json(req): Json<ChatRequest>,
) -> AxumResponse {

    // Validate API key
    match auth {
        Some(TypedHeader(auth_header)) => {
            if auth_header.token() != state.api_key {
                return error("invalid api key", StatusCode::UNAUTHORIZED);
            }
        }
        None => return error("missing authorization", StatusCode::UNAUTHORIZED),
    }

    // Extract user email from header
    let user_email = headers
        .get("x-openwebui-user-email")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("anonymous")
        .to_string();

    println!("HEADERS: {:?}", headers);
    
    if req.stream.unwrap_or(false) {
        let stream = stream_converse(
            state.client,
            req,
            state.logger,
            user_email
        ).await;

        Sse::new(stream)
            .keep_alive(KeepAlive::new())
            .into_response()
    } else {
        non_stream(
            state.client,
            req,
            state.logger,
            user_email
        ).await
    }
}

// =======================
// STREAM MODE
// =======================
async fn stream_converse(
    client: RuntimeClient,
    req: ChatRequest,
    logger: ClickHouseLogger,
    user_email: String,
) -> impl Stream<Item = Result<Event, Infallible>> + Send {

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

                    let chunk = json!({
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

        let final_chunk = json!({
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

        // LOG HERE
        logger
            .log_usage(&user_email, &req.model, prompt_tokens, completion_tokens)
            .await;
    })
}

// =======================
// NON-STREAM MODE
// =======================
async fn non_stream(
    client: RuntimeClient,
    req: ChatRequest,
    logger: ClickHouseLogger,
    user_email: String,
) -> AxumResponse {

    let model_id = req.model.replace("bedrock/", "");
    let messages = convert_messages(&req.messages);
    let model_for_response = req.model.clone();

    let resp = match client
        .converse()
        .model_id(model_id)
        .set_messages(Some(messages))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Bedrock error: {:?}", e);
            return error("bedrock error", StatusCode::BAD_GATEWAY);
        }
    };

    let mut content = String::new();

    if let Some(ConverseOutput::Message(msg)) = resp.output {
        for block in msg.content {
            if let ContentBlock::Text(t) = block {
                content.push_str(&t);
            }
        }
    }

    let prompt_tokens: u32 = req.messages.iter().map(|m| estimate_tokens(&m.content)).sum();
    let completion_tokens = estimate_tokens(&content);

    // LOG HERE
    logger
        .log_usage(&user_email, &model_for_response, prompt_tokens, completion_tokens)
        .await;

    let usage = Usage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
    };

    Json(json!({
        "id": Uuid::new_v4().to_string(),
        "object": "chat.completion",
        "model": model_for_response,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": content
            },
            "finish_reason": "stop"
        }],
        "usage": usage
    })).into_response()
}

// =======================
// HELPERS
// =======================
fn estimate_tokens(text: &str) -> u32 {
    (text.len() / 4) as u32
}

fn error(msg: &str, code: StatusCode) -> AxumResponse {
    (code, Json(ErrorResponse { error: msg.into() })).into_response()
}

pub fn convert_messages(
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

pub async fn list_models_handler(State(state): State<crate::AppState>) -> AxumResponse {
    match state
        .mgmt_client
        .list_foundation_models()
        .send()
        .await
    {
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
            })
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list models: {:?}", e);
            Json(serde_json::json!({
                "object": "list",
                "data": []
            }))
            .into_response()
        }
    }
}