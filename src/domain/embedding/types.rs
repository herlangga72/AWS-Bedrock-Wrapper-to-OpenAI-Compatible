//! Embedding domain types - Data structures for text embeddings

use serde::{Deserialize, Serialize};

// =============================================================================
// Request Types
// =============================================================================

/// OpenAI-compatible embedding request
#[derive(Deserialize)]
pub struct OpenAiEmbeddingRequest {
    pub input: Vec<String>,
}

// =============================================================================
// AWS Response Types
// =============================================================================

/// Nova canvas embedding response from AWS
#[derive(Deserialize)]
pub struct NovaResponse {
    pub embeddings: Vec<NovaEmbeddingEntry>,
    #[serde(rename = "inputTextTokenCount")]
    pub token_count: u64,
}

/// Single embedding result from Nova
#[derive(Deserialize)]
pub struct NovaEmbeddingEntry {
    pub embedding: Vec<f32>,
}

// =============================================================================
// OpenAI Response Types
// =============================================================================

/// OpenAI-compatible embedding response
#[derive(Serialize)]
pub struct OpenAiEmbeddingResponse {
    pub object: &'static str,
    pub data: Vec<OpenAiEmbeddingData>,
    pub model: &'static str,
    pub usage: OpenAiUsage,
}

/// Embedding vector with index
#[derive(Serialize)]
pub struct OpenAiEmbeddingData {
    pub object: &'static str,
    pub embedding: Vec<f32>,
    pub index: usize,
}

/// Usage statistics for embedding
#[derive(Serialize)]
pub struct OpenAiUsage {
    pub prompt_tokens: u64,
    pub total_tokens: u64,
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
            data: vec![OpenAiEmbeddingData {
                object: "embedding",
                embedding: vec![0.1, 0.2, 0.3],
                index: 0,
            }],
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
