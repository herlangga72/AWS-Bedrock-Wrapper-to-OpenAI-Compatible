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

#[cfg(test)]
mod tests {
    use super::*;

    fn bearer_header(token: &str) -> Option<TypedHeader<Authorization<Bearer>>> {
        Some(TypedHeader(Authorization::bearer(token).unwrap()))
    }

    #[test]
    fn missing_auth_returns_err() {
        let result = extract_user_email(None, &HeaderMap::new(), "secret");
        assert!(result.is_err());
    }

    #[test]
    fn wrong_token_returns_err() {
        let result = extract_user_email(bearer_header("wrong"), &HeaderMap::new(), "secret");
        assert!(result.is_err());
    }

    #[test]
    fn correct_token_without_email_header_returns_anonymous() {
        let result = extract_user_email(bearer_header("secret"), &HeaderMap::new(), "secret");
        assert_eq!(result.unwrap(), "anonymous");
    }

    #[test]
    fn correct_token_with_email_header_returns_email() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-openwebui-user-email",
            "user@example.com".parse().unwrap(),
        );
        let result = extract_user_email(bearer_header("secret"), &headers, "secret");
        assert_eq!(result.unwrap(), "user@example.com");
    }

    #[test]
    fn empty_string_token_must_match_exactly() {
        // An empty expected_key only accepts an empty Bearer token.
        let result = extract_user_email(bearer_header("notempty"), &HeaderMap::new(), "");
        assert!(result.is_err());
    }
}
