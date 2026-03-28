use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{
        sse::{KeepAlive, Sse},
        IntoResponse, Response as AxumResponse,
    },
    Json,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use serde_json::json;
use uuid::Uuid;

use crate::middleware::extract_user_email;
use crate::provider::ProviderKind;
use crate::state::AppState;
use crate::types::openai::ErrorResponse;

pub async fn chat_handler(
    State(state): State<AppState>,
    auth: Option<TypedHeader<Authorization<Bearer>>>,
    headers: HeaderMap,
    Json(mut req): Json<crate::types::openai::ChatRequest>,
) -> AxumResponse {
    let user_email = match extract_user_email(auth, &headers, &state.api_key) {
        Ok(e) => e,
        Err(_) => return auth_error(),
    };

    // Compact messages before dispatch (no-op by default; swap AppState::compactor to enable).
    let max_tokens = req.max_tokens.unwrap_or(usize::MAX);
    req.messages = state.compactor.compact(req.messages, max_tokens);

    let model_name = req.model.clone();
    let (kind, model_id) = state.registry.provider_for(&req.model);

    match kind {
        ProviderKind::Bedrock => {
            let bedrock = state.registry.bedrock.clone();

            if req.stream.unwrap_or(false) {
                let s = bedrock.stream(
                    model_id,
                    model_name,
                    req.messages,
                    state.logger.clone(),
                    user_email,
                );
                Sse::new(s).keep_alive(KeepAlive::default()).into_response()
            } else {
                match bedrock.chat(&model_id, req.messages).await {
                    Ok((content, pt, ct)) => {
                        state.logger.log_usage(&user_email, &model_name, pt, ct);
                        Json(json!({
                            "id": format!("chatcmpl-{}", Uuid::new_v4()),
                            "object": "chat.completion",
                            "created": now_secs(),
                            "model": model_name,
                            "choices": [{
                                "index": 0,
                                "message": {"role": "assistant", "content": content},
                                "finish_reason": "stop"
                            }],
                            "usage": {
                                "prompt_tokens": pt,
                                "completion_tokens": ct,
                                "total_tokens": pt + ct
                            }
                        }))
                        .into_response()
                    }
                    Err(e) => {
                        tracing::error!("Bedrock error: {e}");
                        (
                            StatusCode::BAD_GATEWAY,
                            Json(ErrorResponse { error: "upstream service error".into() }),
                        )
                            .into_response()
                    }
                }
            }
        }
    }
}

fn auth_error() -> AxumResponse {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse { error: "unauthorized".into() }),
    )
        .into_response()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
