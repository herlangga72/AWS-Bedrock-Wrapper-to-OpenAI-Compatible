//! Shared error response utilities
//!
//! Provides consistent JSON error responses across all handlers.

use axum::{
    http::StatusCode,
    response::IntoResponse,
    response::{Response, sse::Event},
};
use serde::Serialize;
use std::convert::Infallible;

/// Standard error response structure for all API errors
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: u16,
}

/// Create a JSON error response with the given status and message
pub fn error_response(status: StatusCode, error: &str) -> Response {
    let err = ErrorResponse {
        error: error.to_string(),
        code: status.as_u16(),
    };
    match serde_json::to_string(&err) {
        Ok(json) => (status, [("content-type", "application/json")], json).into_response(),
        Err(e) => {
            tracing::error!("Failed to serialize error response: {e}, falling back to plain text");
            (status, [("content-type", "text/plain")], error.to_string()).into_response()
        }
    }
}

/// Helper to create an SSE error event
pub fn sse_error(error: &str) -> Result<Event, Infallible> {
    Ok(Event::default().data(format!(r#"{{"error":"{}"}}"#, error)))
}