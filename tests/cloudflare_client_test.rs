//! Cloudflare Workers AI integration tests
//!
//! These tests use mockito to mock the Cloudflare API.
//! Run with: `cargo test --test cloudflare_client_test`

mod common;

use aws_bedrock_translation_to_openai::infrastructure::cloudflare::CloudflareClient;

#[test]
fn test_cloudflare_client_is_cloudflare_model() {
    assert!(CloudflareClient::is_cloudflare_model("@cf/meta/llama-3.1-8b-instruct"));
    assert!(CloudflareClient::is_cloudflare_model("@cf/deepseek-ai/deepseek-r1"));
    assert!(!CloudflareClient::is_cloudflare_model("anthropic.claude-3-5-sonnet"));
    assert!(!CloudflareClient::is_cloudflare_model("bedrock/anthropic.claude-v1"));
}

#[test]
fn test_cloudflare_client_builder() {
    let client = CloudflareClient::builder()
        .account_id("test-account")
        .api_token("fake-token")
        .build();

    assert!(client.is_ok());
}
