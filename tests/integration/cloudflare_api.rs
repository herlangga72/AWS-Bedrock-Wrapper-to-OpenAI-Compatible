//! Cloudflare API integration tests with mockito
//!
//! These tests mock the Cloudflare API responses.
//!
//! Run with:
//! ```bash
//! cargo test --test cloudflare_api_test -- --nocapture
//! ```

use aws_bedrock_translation_to_openai::domain::chat::{ChatRequest, Content, Message};
use aws_bedrock_translation_to_openai::infrastructure::cloudflare::CloudflareClient;
use mockito::{Mock, Server};

fn create_cloudflare_mock(server: &Server, path: &str, response_body: &str) -> Mock {
    server
        .mock("POST", path)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(response_body)
        .create()
}

#[tokio::test]
async fn test_cloudflare_chat_non_streaming() {
    let mut server = Server::new_async().await;

    let response = r#"{
        "id": "test-chat-123",
        "result": {
            "messages": [
                {
                    "role": "assistant",
                    "content": "Hello! How can I help you today?"
                }
            ],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 15
            }
        }
    }"#;

    create_cloudflare_mock(
        &server,
        "/client/v4/accounts/test-account/ai/run/@cf/meta/llama-3.1-8b-instruct",
        response,
    );

    let client = CloudflareClient::builder()
        .account_id("test-account")
        .api_token("test-token")
        .base_url(server.url())
        .build()
        .unwrap();

    let req = ChatRequest {
        model: "@cf/meta/llama-3.1-8b-instruct".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: Content::Text("Hello".to_string()),
        }],
        temperature: Some(0.7),
        max_tokens: Some(256),
        stream: Some(false),
        ..Default::default()
    };

    let result = client.chat(req).await;
    assert!(result.is_ok(), "Request should succeed: {:?}", result.err());

    let response = result.unwrap();
    let openai_resp = response.to_openai_response("@cf/meta/llama-3.1-8b-instruct", "test-id");

    assert!(openai_resp.choices.first().map(|c| c.message.content.contains("Hello")).unwrap_or(false));
}

#[tokio::test]
async fn test_cloudflare_chat_error_response() {
    let mut server = Server::new_async().await;

    let response = r#"{
        "id": "error-chat",
        "result": {
            "messages": [],
            "usage": null
        }
    }"#;

    create_cloudflare_mock(
        &server,
        "/client/v4/accounts/test-account/ai/run/@cf/meta/llama-3.1-8b-instruct",
        response,
    );

    let client = CloudflareClient::builder()
        .account_id("test-account")
        .api_token("test-token")
        .base_url(server.url())
        .build()
        .unwrap();

    let req = ChatRequest {
        model: "@cf/meta/llama-3.1-8b-instruct".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: Content::Text("Hello".to_string()),
        }],
        temperature: Some(0.7),
        max_tokens: Some(256),
        stream: Some(false),
        ..Default::default()
    };

    let result = client.chat(req).await;
    assert!(result.is_ok()); // Even with empty messages, response is ok

    let response = result.unwrap();
    let openai_resp = response.to_openai_response("@cf/meta/llama-3.1-8b-instruct", "test-id");

    // Empty messages should result in empty content
    assert_eq!(openai_resp.choices[0].message.content, "");
}

#[tokio::test]
async fn test_cloudflare_chat_with_system_message() {
    let mut server = Server::new_async().await;

    let response = r#"{
        "id": "test-chat-456",
        "result": {
            "messages": [
                {
                    "role": "assistant",
                    "content": "Understood. I'll follow those instructions."
                }
            ],
            "usage": {
                "input_tokens": 50,
                "output_tokens": 25
            }
        }
    }"#;

    create_cloudflare_mock(
        &server,
        "/client/v4/accounts/test-account/ai/run/@cf/meta/llama-3.1-8b-instruct",
        response,
    );

    let client = CloudflareClient::builder()
        .account_id("test-account")
        .api_token("test-token")
        .base_url(server.url())
        .build()
        .unwrap();

    let req = ChatRequest {
        model: "@cf/meta/llama-3.1-8b-instruct".to_string(),
        messages: vec![
            Message {
                role: "system".to_string(),
                content: Content::Text("You are a helpful assistant.".to_string()),
            },
            Message {
                role: "user".to_string(),
                content: Content::Text("Hello".to_string()),
            },
        ],
        temperature: Some(0.7),
        max_tokens: Some(256),
        stream: Some(false),
        ..Default::default()
    };

    let result = client.chat(req).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cloudflare_deepseek_model() {
    let mut server = Server::new_async().await;

    let response = r#"{
        "id": "test-chat-deepseek",
        "result": {
            "messages": [
                {
                    "role": "assistant",
                    "content": "Let me think through this problem step by step..."
                }
            ],
            "usage": {
                "input_tokens": 100,
                "output_tokens": 200
            }
        }
    }"#;

    create_cloudflare_mock(
        &server,
        "/client/v4/accounts/test-account/ai/run/@cf/deepseek-ai/deepseek-r1-distill-qwen-32b",
        response,
    );

    let client = CloudflareClient::builder()
        .account_id("test-account")
        .api_token("test-token")
        .base_url(server.url())
        .build()
        .unwrap();

    let req = ChatRequest {
        model: "@cf/deepseek-ai/deepseek-r1-distill-qwen-32b".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: Content::Text("Solve 2+2".to_string()),
        }],
        temperature: Some(0.5),
        max_tokens: Some(512),
        stream: Some(false),
        ..Default::default()
    };

    let result = client.chat(req).await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert!(response.result.is_some());
    assert!(response.result.unwrap().messages.len() > 0);
}

#[tokio::test]
async fn test_cloudflare_streaming() {
    let mut server = Server::new_async().await;

    // Cloudflare streaming response is newline-delimited JSON
    let response = r#"data: {"response": "Hello"}
data: {"response": " World"}
data: [DONE]"#;

    server
        .mock("POST", "/client/v4/accounts/test-account/ai/run/@cf/meta/llama-3.1-8b-instruct")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(response)
        .create();

    let client = CloudflareClient::builder()
        .account_id("test-account")
        .api_token("test-token")
        .base_url(server.url())
        .build()
        .unwrap();

    let req = ChatRequest {
        model: "@cf/meta/llama-3.1-8b-instruct".to_string(),
        messages: vec![Message {
            role: "user".to_string(),
            content: Content::Text("Hi".to_string()),
        }],
        temperature: None,
        max_tokens: Some(100),
        stream: Some(true),
        ..Default::default()
    };

    let result = client.chat_streaming(req).await;
    assert!(result.is_ok(), "Streaming request should succeed: {:?}", result.err());

    // Collect stream items
    let mut stream = result.unwrap();
    let mut count = 0;
    while let Some(item) = stream.next().await {
        if let Ok(s) = item {
            count += 1;
            println!("Stream item {}: {}", count, s);
        }
    }

    assert!(count > 0, "Should receive stream items");
}
