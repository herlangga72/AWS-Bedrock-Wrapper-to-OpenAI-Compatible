//! Chat domain types - Core data structures for chat completions

use serde::{Deserialize, Serialize};

/// Chat completion request from OpenAI-compatible client
#[derive(Deserialize, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: Option<bool>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stop_sequences: Option<String>,
    // OpenAI extended params (mapped to model-specific via capabilities)
    #[serde(default)]
    pub frequency_penalty: Option<f32>,
    #[serde(default)]
    pub presence_penalty: Option<f32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub logit_bias: Option<std::collections::HashMap<u32, f32>>,
    #[serde(default)]
    #[allow(dead_code)]
    pub user: Option<String>,
    // Model-specific params
    #[serde(default)]
    pub top_k: Option<i32>,
    // Thinking params
    #[serde(default)]
    pub thinking: Option<ThinkingRequest>,
}

/// Thinking configuration for Claude extended thinking
#[derive(Deserialize, Serialize, Clone)]
pub struct ThinkingRequest {
    pub enabled: Option<bool>,
    pub budget_tokens: Option<u32>,
}

/// Chat message with role and content
#[derive(Deserialize, Serialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: Content,
}

/// Content can be plain text or structured blocks
#[derive(Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// Content block with type-specific fields
#[derive(Deserialize, Serialize, Clone)]
pub struct ContentBlock {
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<ReasoningContent>,
}

/// Reasoning content for DeepSeek R1
#[derive(Deserialize, Serialize, Clone)]
pub struct ReasoningContent {
    pub reasoning_text: String,
}

// =============================================================================
// Response Types
// =============================================================================

/// Usage statistics for chat completion
#[derive(Serialize, Clone)]
#[allow(dead_code)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttft_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_per_second: Option<f64>,
}

// =============================================================================
// Model Listing Types
// =============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelData {
    pub id: String,
    pub object: &'static str,
    pub created: u64,
    pub owned_by: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelList {
    pub object: &'static str,
    pub data: Vec<ModelData>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_request_deserialization() {
        let json = r#"{
            "model": "anthropic.claude-sonnet-4-5-20250929-v1:0",
            "messages": [{"role": "user", "content": "Hello"}],
            "temperature": 0.7
        }"#;

        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "anthropic.claude-sonnet-4-5-20250929-v1:0");
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.temperature, Some(0.7));
        assert!(req.stream.is_none());
    }

    #[test]
    fn test_chat_request_full_params() {
        let json = r#"{
            "model": "test-model",
            "messages": [{"role": "assistant", "content": "Hi"}],
            "stream": true,
            "temperature": 0.5,
            "top_p": 0.9,
            "max_tokens": 1000,
            "stop_sequences": "END",
            "frequency_penalty": 0.5,
            "presence_penalty": 0.3,
            "top_k": 250,
            "user": "user123"
        }"#;

        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert!(req.stream.unwrap());
        assert_eq!(req.top_p, Some(0.9));
        assert_eq!(req.max_tokens, Some(1000));
        assert_eq!(req.stop_sequences, Some("END".to_string()));
        assert_eq!(req.frequency_penalty, Some(0.5));
        assert_eq!(req.presence_penalty, Some(0.3));
        assert_eq!(req.top_k, Some(250));
        assert_eq!(req.user, Some("user123".to_string()));
    }

    #[test]
    fn test_message_roles() {
        let json = r#"{
            "model": "test",
            "messages": [
                {"role": "user", "content": "Hello"},
                {"role": "assistant", "content": "Hi there"},
                {"role": "system", "content": "You are helpful"}
            ]
        }"#;

        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.messages[0].role, "user");
        assert_eq!(req.messages[1].role, "assistant");
        assert_eq!(req.messages[2].role, "system");
    }

    #[test]
    fn test_content_text_variant() {
        let msg_json = r#"{"role": "user", "content": "Hello world"}"#;
        let msg: Message = serde_json::from_str(msg_json).unwrap();

        match msg.content {
            Content::Text(text) => assert_eq!(text, "Hello world"),
            Content::Blocks(_) => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_content_blocks_variant() {
        let json = r#"{
            "role": "assistant",
            "content": [
                {"type": "thinking", "thinking": "Let me think..."},
                {"type": "text", "text": "Hello!"}
            ]
        }"#;

        let msg: Message = serde_json::from_str(json).unwrap();
        match msg.content {
            Content::Text(_) => panic!("Expected Blocks variant"),
            Content::Blocks(blocks) => {
                assert_eq!(blocks.len(), 2);
                assert_eq!(blocks[0].r#type, "thinking");
                assert_eq!(blocks[1].r#type, "text");
            }
        }
    }

    #[test]
    fn test_content_block_serialization() {
        let block = ContentBlock {
            r#type: "text".to_string(),
            text: Some("Hello".to_string()),
            thinking: None,
            signature: None,
            reasoning_content: None,
        };

        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"Hello\""));
    }

    #[test]
    fn test_content_block_skip_none_fields() {
        let block = ContentBlock {
            r#type: "thinking".to_string(),
            text: None,
            thinking: Some("thinking...".to_string()),
            signature: None,
            reasoning_content: None,
        };

        let json = serde_json::to_string(&block).unwrap();
        // Should NOT contain "text", "signature", "reasoning_content" keys since they're None
        assert!(!json.contains("\"text\""));
        assert!(json.contains("\"thinking\":\"thinking...\""));
    }

    #[test]
    fn test_thinking_request_deserialization() {
        let json = r#"{"enabled": true, "budget_tokens": 8000}"#;
        let thinking: ThinkingRequest = serde_json::from_str(json).unwrap();
        assert!(thinking.enabled.unwrap());
        assert_eq!(thinking.budget_tokens, Some(8000));
    }

    #[test]
    fn test_thinking_request_partial() {
        let json = r#"{"budget_tokens": 4000}"#;
        let thinking: ThinkingRequest = serde_json::from_str(json).unwrap();
        assert!(thinking.enabled.is_none());
        assert_eq!(thinking.budget_tokens, Some(4000));
    }

    #[test]
    fn test_reasoning_content_serde() {
        let rc = ReasoningContent {
            reasoning_text: "Because...".to_string(),
        };

        let json = serde_json::to_string(&rc).unwrap();
        assert!(json.contains("\"reasoning_text\":\"Because...\""));

        let parsed: ReasoningContent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.reasoning_text, "Because...");
    }

    #[test]
    fn test_usage_serialization() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 200,
            total_tokens: 300,
            completion_tokens: Some(200),
            ttft_ms: Some(50),
            latency_ms: Some(150),
            tokens_per_second: Some(13.33),
        };

        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"input_tokens\":100"));
        assert!(json.contains("\"output_tokens\":200"));
        assert!(json.contains("\"completion_tokens\":200"));
    }

    #[test]
    fn test_usage_skip_optional_fields() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 200,
            total_tokens: 300,
            completion_tokens: None,
            ttft_ms: None,
            latency_ms: None,
            tokens_per_second: None,
        };

        let json = serde_json::to_string(&usage).unwrap();
        assert!(!json.contains("completion_tokens"));
        assert!(!json.contains("ttft_ms"));
    }

    #[test]
    fn test_model_data_serde() {
        let json = r#"{
            "id": "anthropic.claude-sonnet-4-5-20250929-v1:0",
            "object": "model",
            "created": 1234567890,
            "owned_by": "anthropic"
        }"#;

        let model: ModelData = serde_json::from_str(json).unwrap();
        assert_eq!(model.id, "anthropic.claude-sonnet-4-5-20250929-v1:0");
        assert_eq!(model.object, "model");
        assert_eq!(model.created, 1234567890);
        assert_eq!(model.owned_by, "anthropic");
    }

    #[test]
    fn test_model_list_serde() {
        let json = r#"{
            "object": "list",
            "data": [
                {"id": "model1", "object": "model", "created": 0, "owned_by": "test"},
                {"id": "model2", "object": "model", "created": 0, "owned_by": "test"}
            ]
        }"#;

        let list: ModelList = serde_json::from_str(json).unwrap();
        assert_eq!(list.object, "list");
        assert_eq!(list.data.len(), 2);
        assert_eq!(list.data[0].id, "model1");
        assert_eq!(list.data[1].id, "model2");
    }
}
