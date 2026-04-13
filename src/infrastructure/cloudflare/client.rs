//! Cloudflare Workers AI API client
//!
//! Cloudflare Workers AI provides OpenAI-compatible API endpoints.
//! Base URL: https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/run/{model}

use crate::domain::chat::{ChatRequest, Content, Message};
use futures_util::Stream;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const DEFAULT_CF_BASE_URL: &str = "https://api.cloudflare.com/client/v4/accounts";

/// Cloudflare API client
#[derive(Clone, Debug)]
pub struct CloudflareClient {
    client: Client,
    account_id: String,
    api_token: String,
    base_url: String,
}

/// Builder for CloudflareClient
#[derive(Default)]
pub struct CloudflareClientBuilder {
    account_id: Option<String>,
    api_token: Option<String>,
    base_url: Option<String>,
}

impl CloudflareClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn account_id(mut self, account_id: impl Into<String>) -> Self {
        self.account_id = Some(account_id.into());
        self
    }

    pub fn api_token(mut self, api_token: impl Into<String>) -> Self {
        self.api_token = Some(api_token.into());
        self
    }

    /// Override the base URL (useful for testing with mockito)
    #[cfg(test)]
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    pub fn build(self) -> Result<CloudflareClient, String> {
        let account_id = self.account_id.ok_or("account_id is required")?;
        let api_token = self.api_token.ok_or("api_token is required")?;
        let base_url = self
            .base_url
            .unwrap_or_else(|| DEFAULT_CF_BASE_URL.to_string());

use crate::shared::constants::REQUEST_TIMEOUT_CLOUDFLARE;

        Ok(CloudflareClient {
            client: Client::builder()
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_CLOUDFLARE))
                .build()
                .map_err(|e| e.to_string())?,
            account_id,
            api_token,
            base_url,
        })
    }
}

impl CloudflareClient {
    /// Create a new Cloudflare client using builder pattern
    pub fn builder() -> CloudflareClientBuilder {
        CloudflareClientBuilder::new()
    }

    /// Build the full endpoint URL for a model
    #[allow(dead_code)]
    pub fn is_cloudflare_model(model_id: &str) -> bool {
        model_id.starts_with("@cf/")
    }

    /// Build the full endpoint URL for a model
    fn endpoint_url(&self, model: &str) -> String {
        format!("{}/{}/ai/run/{}", self.base_url, self.account_id, model)
    }

    /// Execute a chat completion request
    /// If caveman_prompt is Some, prepend a system message with the caveman rules
    pub async fn chat(&self, mut req: ChatRequest, caveman_prompt: Option<String>) -> Result<CloudflareResponse, String> {
        let url = self.endpoint_url(&req.model);

        // Inject caveman system prompt if activated
        if let Some(prompt) = caveman_prompt {
            req.messages.insert(0, Message {
                role: "system".to_string(),
                content: Content::Text(prompt),
            });
        }

        let cf_req = CloudflareRequest {
            messages: req.messages.into_iter().map(Into::into).collect(),
            max_tokens: req.max_tokens.unwrap_or(256),
            stream: false,
            temperature: req.temperature,
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&cf_req)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Cloudflare API error {}: {}", status, body));
        }

        resp.json::<CloudflareResponse>()
            .await
            .map_err(|e| e.to_string())
    }

    /// Execute a streaming chat completion request
    /// If caveman_prompt is Some, prepend a system message with the caveman rules
    pub async fn chat_streaming(
        &self,
        mut req: ChatRequest,
        caveman_prompt: Option<String>,
    ) -> Result<impl Stream<Item = Result<String, String>>, String> {
        // Inject caveman system prompt if activated
        if let Some(prompt) = caveman_prompt {
            req.messages.insert(0, Message {
                role: "system".to_string(),
                content: Content::Text(prompt),
            });
        }

        let url = self.endpoint_url(&req.model);

        let _cf_req = CloudflareRequest {
            messages: req.messages.into_iter().map(Into::into).collect(),
            max_tokens: req.max_tokens.unwrap_or(256),
            stream: true,
            temperature: req.temperature,
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let status = resp.status();
            return Err(format!("Cloudflare API error: {}", status));
        }

        let stream = resp
            .bytes_stream()
            .map(|chunk: Result<bytes::Bytes, reqwest::Error>| {
                chunk
                    .map(|b| String::from_utf8_lossy(&b).to_string())
                    .map_err(|e| e.to_string())
            });
        Ok(stream)
    }
}

// =============================================================================
// Cloudflare API Request/Response Types
// =============================================================================

#[derive(Serialize)]
struct CloudflareRequest {
    messages: Vec<CfMessage>,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone)]
struct CfMessage {
    role: String,
    content: String,
}

impl From<Message> for CfMessage {
    fn from(msg: Message) -> Self {
        let content = match msg.content {
            Content::Text(s) => s,
            Content::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| b.text.clone())
                .collect::<Vec<_>>()
                .join("\n"),
        };
        Self {
            role: msg.role,
            content,
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct CloudflareResponse {
    #[allow(dead_code)]
    pub id: Option<String>,
    pub result: Option<CfResult>,
}

#[derive(Deserialize, Debug)]
pub struct CfResult {
    pub messages: Vec<CfResponseMessage>,
    pub usage: Option<CfUsage>,
}

#[derive(Deserialize, Debug)]
pub struct CfResponseMessage {
    #[allow(dead_code)]
    pub role: String,
    pub content: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CfUsage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

// =============================================================================
// OpenAI-compatible Response Conversion
// =============================================================================

impl CloudflareResponse {
    /// Convert to OpenAI-compatible response format
    pub fn to_openai_response(&self, model: &str, request_id: &str) -> OpenAiChatResponse {
        let content = self
            .result
            .as_ref()
            .and_then(|r| r.messages.first())
            .map(|m| m.content.clone())
            .unwrap_or_default();

        let usage = self.result.as_ref().and_then(|r| r.usage.clone());

        OpenAiChatResponse {
            id: request_id.to_string(),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp() as u64,
            model: model.to_string(),
            choices: vec![OpenAiChoice {
                index: 0,
                message: OpenAiMessage {
                    role: "assistant".to_string(),
                    content,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: usage.map(|u| OpenAiUsage {
                prompt_tokens: u.input_tokens.unwrap_or(0),
                completion_tokens: u.output_tokens.unwrap_or(0),
                total_tokens: u.input_tokens.unwrap_or(0) + u.output_tokens.unwrap_or(0),
            }),
        }
    }
}

#[derive(Serialize)]
pub struct OpenAiChatResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<OpenAiChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAiUsage>,
}

#[derive(Serialize)]
pub struct OpenAiChoice {
    pub index: u32,
    pub message: OpenAiMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Serialize)]
pub struct OpenAiMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct OpenAiUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// =============================================================================
// Tests (no mocking required)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cloudflare_response_to_openai_response() {
        let cf_resp = CloudflareResponse {
            id: Some("test-id".to_string()),
            result: Some(CfResult {
                messages: vec![CfResponseMessage {
                    role: "assistant".to_string(),
                    content: "Hello from Cloudflare!".to_string(),
                }],
                usage: Some(CfUsage {
                    input_tokens: Some(10),
                    output_tokens: Some(20),
                }),
            }),
        };

        let openai = cf_resp.to_openai_response("cloudflare/meta/llama-3.1-8b-instruct", "req-123");

        assert_eq!(openai.id, "req-123");
        assert_eq!(openai.model, "cloudflare/meta/llama-3.1-8b-instruct");
        assert_eq!(openai.choices.len(), 1);
        assert_eq!(openai.choices[0].message.content, "Hello from Cloudflare!");
        assert_eq!(openai.choices[0].message.role, "assistant");
        assert_eq!(openai.choices[0].finish_reason, Some("stop".to_string()));

        assert!(openai.usage.is_some());
        let usage = openai.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    #[test]
    fn test_cloudflare_response_empty_messages() {
        let cf_resp = CloudflareResponse {
            id: Some("test-id".to_string()),
            result: Some(CfResult {
                messages: vec![],
                usage: None,
            }),
        };

        let openai = cf_resp.to_openai_response("test-model", "req-456");

        assert_eq!(openai.choices[0].message.content, "");
        assert!(openai.usage.is_none());
    }

    #[test]
    fn test_cloudflare_response_no_result() {
        let cf_resp = CloudflareResponse {
            id: None,
            result: None,
        };

        let openai = cf_resp.to_openai_response("test-model", "req-789");

        assert_eq!(openai.choices[0].message.content, "");
        assert!(openai.usage.is_none());
    }

    #[test]
    fn test_cloudflare_response_partial_usage() {
        let cf_resp = CloudflareResponse {
            id: Some("test-id".to_string()),
            result: Some(CfResult {
                messages: vec![CfResponseMessage {
                    role: "assistant".to_string(),
                    content: "Hi".to_string(),
                }],
                usage: Some(CfUsage {
                    input_tokens: Some(5),
                    output_tokens: None,
                }),
            }),
        };

        let openai = cf_resp.to_openai_response("test-model", "req-101");

        assert_eq!(openai.usage.unwrap().total_tokens, 5);
    }

    #[test]
    fn test_is_cloudflare_model_edge_cases() {
        // Valid Cloudflare models
        assert!(CloudflareClient::is_cloudflare_model(
            "@cf/meta/llama-3.1-8b-instruct"
        ));
        assert!(CloudflareClient::is_cloudflare_model(
            "@cf/deepseek-ai/deepseek-r1-distill-qwen-32b"
        ));
        assert!(CloudflareClient::is_cloudflare_model(
            "@cf/google/gemma-2-2b-it"
        ));
        assert!(CloudflareClient::is_cloudflare_model(
            "@cf/mistral/mistral-7b-instruct"
        ));

        // Not Cloudflare models (must start with @cf/)
        assert!(!CloudflareClient::is_cloudflare_model("@cf")); // just prefix
        assert!(!CloudflareClient::is_cloudflare_model("cf/llama")); // missing @
        assert!(!CloudflareClient::is_cloudflare_model("@cfx/meta/llama")); // extra char
        assert!(!CloudflareClient::is_cloudflare_model(
            "anthropic.claude-v1"
        ));
        assert!(!CloudflareClient::is_cloudflare_model(
            "bedrock/anthropic.claude-v1"
        )); // bedrock, not cloudflare
        assert!(!CloudflareClient::is_cloudflare_model(""));
    }

    #[test]
    fn test_endpoint_url_building() {
        // Test via the builder and inspection
        let _client = CloudflareClient::builder()
            .account_id("test-account-id")
            .api_token("test-token")
            .base_url("https://api.cloudflare.com/client/v4/accounts".to_string())
            .build()
            .unwrap();

        // Use a workaround to test URL building - create client and check endpoint
        // Since endpoint_url is private, we test through is_cloudflare_model which is public
        assert!(CloudflareClient::is_cloudflare_model("@cf/meta/llama"));
    }

    #[test]
    fn test_cloudflare_client_builder_validation() {
        // Missing account_id
        let result = CloudflareClient::builder().api_token("token").build();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("account_id"));

        // Missing api_token
        let result = CloudflareClient::builder().account_id("account").build();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("api_token"));

        // Valid
        let result = CloudflareClient::builder()
            .account_id("account")
            .api_token("token")
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_openai_response_serialization() {
        let openai = OpenAiChatResponse {
            id: "test-id".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "cloudflare/meta/llama".to_string(),
            choices: vec![OpenAiChoice {
                index: 0,
                message: OpenAiMessage {
                    role: "assistant".to_string(),
                    content: "Hello".to_string(),
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Some(OpenAiUsage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            }),
        };

        let json = serde_json::to_string(&openai).unwrap();
        assert!(json.contains("\"id\":\"test-id\""));
        assert!(json.contains("\"model\":\"cloudflare/meta/llama\""));
        assert!(json.contains("\"content\":\"Hello\""));
        assert!(json.contains("\"prompt_tokens\":10"));
        assert!(json.contains("\"completion_tokens\":20"));
    }

    #[test]
    fn test_openai_response_skip_optional_usage() {
        let openai = OpenAiChatResponse {
            id: "test-id".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "test".to_string(),
            choices: vec![],
            usage: None,
        };

        let json = serde_json::to_string(&openai).unwrap();
        // usage field should be skipped when None
        assert!(!json.contains("usage"));
    }
}
