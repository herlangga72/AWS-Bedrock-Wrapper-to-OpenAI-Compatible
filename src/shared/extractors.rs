//! Shared HTTP extractors

use axum_extra::headers::{authorization::Bearer, Authorization};
use axum_extra::TypedHeader;

/// Extract Bearer token from Authorization header
pub fn extract_bearer_token(auth: Option<TypedHeader<Authorization<Bearer>>>) -> Option<String> {
    auth.map(|TypedHeader(Authorization(bearer))| bearer.token().to_string())
}
