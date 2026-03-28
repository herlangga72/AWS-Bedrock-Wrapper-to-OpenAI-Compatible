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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_error_includes_message() {
        let e = ProviderError::Upstream("bad gateway".into());
        assert_eq!(e.to_string(), "upstream error: bad gateway");
    }

    #[test]
    fn timeout_error_message() {
        assert_eq!(ProviderError::Timeout.to_string(), "request timed out");
    }

    #[test]
    fn upstream_error_with_empty_message() {
        assert_eq!(ProviderError::Upstream(String::new()).to_string(), "upstream error: ");
    }
}
