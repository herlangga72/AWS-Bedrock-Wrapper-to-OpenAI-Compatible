//! AWS Bedrock Invoke Model API implementation (for Claude extended thinking)

use aws_sdk_bedrock::primitives::Blob;
use aws_sdk_bedrockruntime::Client as RuntimeClient;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::domain::chat::{ChatRequest, Content};

const THINKING_VERSION: &str = "bedrock-2023-05-31";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

// =============================================================================
// Request Types
// =============================================================================

#[derive(Serialize)]
pub struct ThinkingRequestBody<'a> {
    anthropic_version: &'a str,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingBlock>,
    messages: Vec<ThinkingMessage<'a>>,
}

#[derive(Serialize)]
struct ThinkingBlock {
    r#type: &'static str,
    budget_tokens: u32,
}

#[derive(Serialize)]
struct ThinkingMessage<'a> {
    role: &'a str,
    content: ThinkingContent<'a>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ThinkingContent<'a> {
    Text(&'a str),
    OwnedText(String),
    Blocks(Vec<ThinkingContentBlock<'a>>),
}

#[derive(Serialize)]
struct ThinkingContentBlock<'a> {
    r#type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<&'a str>,
}

// =============================================================================
// Response Types
// =============================================================================

#[derive(Deserialize, Debug)]
pub struct ThinkingResponse {
    pub content: Vec<ResponseContentBlock>,
    pub usage: ResponseUsage,
}

#[derive(Deserialize, Debug)]
pub struct ResponseContentBlock {
    pub r#type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub thinking: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ResponseUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

// =============================================================================
// Request Builder
// =============================================================================

/// Build thinking request body for Claude models
/// If caveman_prompt is Some, prepend a system message with the caveman rules
pub fn build_thinking_request<'a>(
    req: &'a ChatRequest,
    max_tokens: u32,
    budget_tokens: u32,
    caveman_prompt: Option<&str>,
) -> ThinkingRequestBody<'a> {
    let thinking_block = Some(ThinkingBlock {
        r#type: "enabled",
        budget_tokens,
    });

    let mut messages: Vec<ThinkingMessage<'_>> = Vec::new();

    // Prepend caveman system message if activated
    if let Some(prompt) = caveman_prompt {
        messages.push(ThinkingMessage {
            role: "user",
            content: ThinkingContent::OwnedText(prompt.to_string()),
        });
    }

    let user_role = "user";
    let assistant_role = "assistant";

    messages.extend(req.messages.iter().map(|m| {
        let role = match m.role.as_str() {
            "user" => user_role,
            "assistant" => assistant_role,
            _ => user_role,
        };
        let content = match &m.content {
            Content::Text(text) => ThinkingContent::Text(text.as_str()),
            Content::Blocks(blocks) => {
                let blocks: Vec<ThinkingContentBlock<'_>> = blocks
                    .iter()
                    .map(|b| ThinkingContentBlock {
                        r#type: if b.thinking.is_some() {
                            "thinking"
                        } else {
                            "text"
                        },
                        text: b.text.as_deref(),
                        thinking: b.thinking.as_deref(),
                    })
                    .collect();
                ThinkingContent::Blocks(blocks)
            }
        };
        ThinkingMessage { role, content }
    }));

    ThinkingRequestBody {
        anthropic_version: THINKING_VERSION,
        max_tokens,
        thinking: thinking_block,
        messages,
    }
}

/// Parse thinking request from ChatRequest
pub fn parse_thinking_params(req: &ChatRequest) -> (u32, u32) {
    let budget_tokens = req
        .thinking
        .as_ref()
        .and_then(|t| t.budget_tokens)
        .unwrap_or(4000);
    let max_tokens = req.max_tokens.unwrap_or(4096).max(budget_tokens + 1024);
    (max_tokens, budget_tokens)
}

/// Send invoke model request
pub async fn invoke_thinking_model(
    client: &RuntimeClient,
    model_id: &str,
    body: &ThinkingRequestBody<'_>,
) -> Result<ThinkingResponse, String> {
    let body_json = serde_json::to_string(body).map_err(|e| e.to_string())?;

    let invoke = client
        .invoke_model()
        .model_id(model_id)
        .content_type("application/json")
        .accept("application/json")
        .body(Blob::new(body_json));

    let resp = timeout(REQUEST_TIMEOUT, invoke.send())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    let resp_body = resp.body().as_ref();
    serde_json::from_slice(resp_body).map_err(|e| e.to_string())
}

use tokio::time::timeout;
