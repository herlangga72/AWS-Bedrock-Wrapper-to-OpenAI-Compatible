//! Nova embedding request builder

use serde::Serialize;
use std::borrow::Cow;

/// Builder for Nova embedding requests to Amazon Nova canvas
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

    pub fn with_purpose(mut self, purpose: &'static str) -> Self {
        self.params.embedding_purpose = purpose;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
