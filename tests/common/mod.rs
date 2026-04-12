//! Common utilities for integration tests

/// Creates a mock Cloudflare streaming response
#[allow(dead_code)]
pub fn mock_cloudflare_streaming_chunk(content: &str) -> String {
    format!(r#"data: {{"response":"{}"}}"#, content)
}
