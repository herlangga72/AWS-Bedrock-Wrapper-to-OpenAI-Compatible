use axum::{
    routing::{get, post},
    Router,
};

use crate::handlers;
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/v1/chat/completions", post(handlers::chat::chat_handler))
        .route("/v1/models", get(handlers::models::list_models_handler))
        .with_state(state)
}

/// HTTP-level integration tests.
///
/// These tests use `tower::ServiceExt::oneshot` to drive the full Axum router
/// without binding a TCP socket. AWS SDK clients are constructed but **never
/// called** because every test case exits before reaching the provider layer
/// (auth rejection or JSON validation error).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tower::ServiceExt;

    // ── helpers ──────────────────────────────────────────────────────────────

    async fn body_str(body: Body) -> String {
        let bytes = body.collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    fn post_chat(token: Option<&str>, body: &str) -> Request<Body> {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header("content-type", "application/json");
        if let Some(t) = token {
            builder = builder.header("authorization", format!("Bearer {t}"));
        }
        builder.body(Body::from(body.to_string())).unwrap()
    }

    fn valid_body() -> &'static str {
        r#"{"model":"bedrock/claude","messages":[{"role":"user","content":"hello"}]}"#
    }

    // ── auth tests ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn missing_auth_header_returns_401() {
        let app = build_router(AppState::for_testing("secret"));
        let resp = app.oneshot(post_chat(None, valid_body())).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let json: Value =
            serde_json::from_str(&body_str(resp.into_body()).await).unwrap();
        assert!(json["error"].is_string());
    }

    #[tokio::test]
    async fn wrong_api_key_returns_401() {
        let app = build_router(AppState::for_testing("correct-key"));
        let resp = app
            .oneshot(post_chat(Some("wrong-key"), valid_body()))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let json: Value =
            serde_json::from_str(&body_str(resp.into_body()).await).unwrap();
        assert!(json["error"].is_string());
    }

    #[tokio::test]
    async fn correct_key_with_invalid_json_returns_400() {
        // Axum's Json extractor rejects the request before the handler body
        // runs when the body is unparseable.
        let app = build_router(AppState::for_testing("key"));
        let resp = app
            .oneshot(post_chat(Some("key"), "not json"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn correct_key_with_empty_body_returns_400() {
        let app = build_router(AppState::for_testing("key"));
        let req = Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header("content-type", "application/json")
            .header("authorization", "Bearer key")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
