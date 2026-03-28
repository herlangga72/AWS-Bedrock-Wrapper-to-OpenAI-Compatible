use axum::http::HeaderMap;
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};

/// Validates the Bearer token and returns the user e-mail (or `"anonymous"`).
/// Returns `Err(())` if the token is missing or does not match `expected_key`.
pub fn extract_user_email(
    auth: Option<TypedHeader<Authorization<Bearer>>>,
    headers: &HeaderMap,
    expected_key: &str,
) -> Result<String, ()> {
    match auth {
        Some(TypedHeader(bearer)) if bearer.token() == expected_key => Ok(headers
            .get("x-openwebui-user-email")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("anonymous")
            .to_string()),
        _ => Err(()),
    }
}
