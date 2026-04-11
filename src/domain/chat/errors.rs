//! Chat domain errors

use std::fmt;

/// Errors that can occur in chat operations
#[derive(Debug)]
pub enum ChatError {
    /// Authentication failed
    AuthenticationFailed,
    /// Missing API key
    MissingApiKey,
    /// Bedrock API error
    BedrockError(String),
    /// Timeout waiting for response
    Timeout,
    /// Invalid request
    InvalidRequest(String),
    /// Model not found
    ModelNotFound(String),
    /// Unsupported operation
    UnsupportedOperation(String),
}

impl fmt::Display for ChatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatError::AuthenticationFailed => write!(f, "Authentication failed"),
            ChatError::MissingApiKey => write!(f, "Missing API key"),
            ChatError::BedrockError(msg) => write!(f, "Bedrock error: {}", msg),
            ChatError::Timeout => write!(f, "Request timed out"),
            ChatError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            ChatError::ModelNotFound(model) => write!(f, "Model not found: {}", model),
            ChatError::UnsupportedOperation(op) => write!(f, "Unsupported operation: {}", op),
        }
    }
}

impl std::error::Error for ChatError {}
