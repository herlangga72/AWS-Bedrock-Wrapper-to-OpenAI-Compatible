//! Embedding domain types

use serde::{Deserialize, Serialize};
use std::borrow::Cow;

// =============================================================================
// Request Types
// =============================================================================

/// OpenAI-compatible embedding request
#[derive(Deserialize)]
pub struct OpenAiEmbeddingRequest {
    pub input: Vec<String>,
}

// =============================================================================
// Internal Request Types (for AWS Nova)
// =============================================================================

#[derive(Serialize)]
pub struct NovaRequest<'a> {
    #[serde(rename = "taskType")]
    task_type: &'static str,
    #[serde(rename = "singleEmbeddingParams")]
    params: NovaParams<'a>,
}

#[derive(Serialize)]
struct NovaParams<'a> {
    #[serde(rename = "embeddingPurpose")]
    embedding_purpose: &'static str,
    #[serde(rename = "embeddingDimension")]
    dimension: u32,
    text: NovaText<'a>,
}

#[derive(Serialize)]
struct NovaText<'a> {
    #[serde(rename = "truncationMode")]
    truncation_mode: &'static str,
    value: Cow<'a, str>,
}

// =============================================================================
// AWS Response Types
// =============================================================================

#[derive(Deserialize)]
pub struct NovaResponse {
    pub embeddings: Vec<NovaEmbeddingEntry>,
    #[serde(rename = "inputTextTokenCount")]
    pub token_count: u64,
}

#[derive(Deserialize)]
pub struct NovaEmbeddingEntry {
    pub embedding: Vec<f32>,
}

// =============================================================================
// OpenAI Response Types
// =============================================================================

#[derive(Serialize)]
pub struct OpenAiEmbeddingResponse {
    pub object: &'static str,
    pub data: Vec<OpenAiEmbeddingData>,
    pub model: &'static str,
    pub usage: OpenAiUsage,
}

#[derive(Serialize)]
pub struct OpenAiEmbeddingData {
    pub object: &'static str,
    pub embedding: Vec<f32>,
    pub index: usize,
}

#[derive(Serialize)]
pub struct OpenAiUsage {
    pub prompt_tokens: u64,
    pub total_tokens: u64,
}

// =============================================================================
// Builder for Nova Request
// =============================================================================

impl<'a> NovaRequest<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            task_type: "SINGLE_EMBEDDING",
            params: NovaParams {
                embedding_purpose: "GENERIC_INDEX",
                dimension: 3072,
                text: NovaText {
                    truncation_mode: "END",
                    value: Cow::Borrowed(text),
                },
            },
        }
    }

    pub fn with_dimension(mut self, dimension: u32) -> Self {
        self.params.dimension = dimension;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_embedding_request_deserialization() {
        let json = r#"{"input": ["Hello world", "Goodbye"]}"#;
        let req: OpenAiEmbeddingRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.input.len(), 2);
        assert_eq!(req.input[0], "Hello world");
    }

    #[test]
    fn test_openai_embedding_request_single_input() {
        let json = r#"{"input": ["Single text"]}"#;
        let req: OpenAiEmbeddingRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.input.len(), 1);
    }

    #[test]
    fn test_nova_request_builder() {
        let req = NovaRequest::new("Hello world");

        assert_eq!(req.task_type, "SINGLE_EMBEDDING");
        assert_eq!(req.params.embedding_purpose, "GENERIC_INDEX");
        assert_eq!(req.params.dimension, 3072);
        assert_eq!(req.params.text.truncation_mode, "END");
        assert_eq!(req.params.text.value, "Hello world");
    }

    #[test]
    fn test_nova_request_with_custom_dimension() {
        let req = NovaRequest::new("Hello").with_dimension(1024);
        assert_eq!(req.params.dimension, 1024);
    }

    #[test]
    fn test_nova_request_serialization() {
        let req = NovaRequest::new("Test text");
        let json = serde_json::to_string(&req).unwrap();

        assert!(json.contains("\"taskType\":\"SINGLE_EMBEDDING\""));
        assert!(json.contains("\"embeddingPurpose\":\"GENERIC_INDEX\""));
        assert!(json.contains("\"embeddingDimension\":3072"));
        assert!(json.contains("\"truncationMode\":\"END\""));
        assert!(json.contains("\"value\":\"Test text\""));
    }

    #[test]
    fn test_nova_response_deserialization() {
        let json = r#"{
            "embeddings": [
                {"embedding": [0.1, 0.2, 0.3]},
                {"embedding": [0.4, 0.5, 0.6]}
            ],
            "inputTextTokenCount": 10
        }"#;

        let resp: NovaResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.embeddings.len(), 2);
        assert_eq!(resp.embeddings[0].embedding, vec![0.1, 0.2, 0.3]);
        assert_eq!(resp.embeddings[1].embedding, vec![0.4, 0.5, 0.6]);
        assert_eq!(resp.token_count, 10);
    }

    #[test]
    fn test_openai_embedding_response_serialization() {
        let resp = OpenAiEmbeddingResponse {
            object: "list",
            model: "amazon.nova-2-embeddings-v1:0",
            data: vec![
                OpenAiEmbeddingData {
                    object: "embedding",
                    embedding: vec![0.1, 0.2, 0.3],
                    index: 0,
                },
            ],
            usage: OpenAiUsage {
                prompt_tokens: 5,
                total_tokens: 5,
            },
        };

        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"object\":\"list\""));
        assert!(json.contains("\"model\":\"amazon.nova-2-embeddings-v1:0\""));
        assert!(json.contains("\"embedding\":[0.1,0.2,0.3]"));
        assert!(json.contains("\"prompt_tokens\":5"));
    }

    #[test]
    fn test_openai_usage_serialization() {
        let usage = OpenAiUsage {
            prompt_tokens: 100,
            total_tokens: 150,
        };

        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"prompt_tokens\":100"));
        assert!(json.contains("\"total_tokens\":150"));
    }
}
