//! Common utilities for integration tests

/// Creates a mock Cloudflare Workers AI response
pub fn mock_cloudflare_response(content: &str) -> String {
    format!(
        r#"{{"id":"test-id","result":{{"messages":[{{"role":"assistant","content":"{}"}}],"usage":{{"input_tokens":10,"output_tokens":20}}}}}}"#,
        content
    )
}

/// Creates a mock Cloudflare streaming response
#[allow(dead_code)]
pub fn mock_cloudflare_streaming_chunk(content: &str) -> String {
    format!(r#"data: {{"response":"{}"}}"#, content)
}
