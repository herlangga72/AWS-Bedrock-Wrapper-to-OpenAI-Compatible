//! Auth domain types - Data structures for API key authentication

use std::fmt;

/// Authentication error types
#[derive(Debug, Clone)]
pub enum AuthError {
    DbError(String),
    Forbidden,
    LockError,
    InvalidKeyFormat,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AuthError::DbError(e) => write!(f, "Database Error: {}", e),
            AuthError::Forbidden => write!(f, "403 Forbidden: Invalid API Key"),
            AuthError::LockError => write!(f, "Internal State Contention"),
            AuthError::InvalidKeyFormat => write!(f, "Invalid API Key Format"),
        }
    }
}

impl std::error::Error for AuthError {}

/// API key record (query result, key hash stored not actual key)
#[derive(Debug, Clone)]
pub struct ApiKey {
    pub id: i64,
    pub key_hash: String,
    pub email: String,
    pub name: Option<String>,
    pub created_at: String,
    pub last_used: Option<String>,
    pub is_active: bool,
}
