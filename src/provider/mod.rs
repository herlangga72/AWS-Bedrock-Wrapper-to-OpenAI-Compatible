pub mod bedrock;
pub mod registry;

/// Identifies which backend will serve a request.
/// Add a variant here when adding a new provider.
#[derive(Debug)]
pub enum ProviderKind {
    Bedrock,
    // OpenAI,   // future HTTP provider example
}

/// Error returned by any provider.
#[derive(Debug)]
pub enum ProviderError {
    Upstream(String),
    Timeout,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Upstream(msg) => write!(f, "upstream error: {msg}"),
            ProviderError::Timeout => write!(f, "request timed out"),
        }
    }
}
