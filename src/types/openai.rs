use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: Option<bool>,
    pub max_tokens: Option<usize>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ModelData {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

#[derive(Serialize)]
pub struct ModelList {
    pub object: String,
    pub data: Vec<ModelData>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_str, json, to_value};

    // ── ChatRequest deserialization ───────────────────────────────────────────

    #[test]
    fn chat_request_parses_required_fields() {
        let json = r#"{"model":"bedrock/claude","messages":[{"role":"user","content":"hi"}]}"#;
        let req: ChatRequest = from_str(json).unwrap();
        assert_eq!(req.model, "bedrock/claude");
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, "user");
        assert_eq!(req.messages[0].content, "hi");
        assert_eq!(req.stream, None);
        assert_eq!(req.max_tokens, None);
    }

    #[test]
    fn chat_request_parses_optional_fields() {
        let json = r#"{"model":"m","messages":[],"stream":true,"max_tokens":512}"#;
        let req: ChatRequest = from_str(json).unwrap();
        assert_eq!(req.stream, Some(true));
        assert_eq!(req.max_tokens, Some(512));
    }

    #[test]
    fn chat_request_with_empty_messages_is_valid() {
        let json = r#"{"model":"m","messages":[]}"#;
        let req: ChatRequest = from_str(json).unwrap();
        assert!(req.messages.is_empty());
    }

    #[test]
    fn chat_request_missing_model_fails() {
        let result: Result<ChatRequest, _> =
            from_str(r#"{"messages":[{"role":"user","content":"hi"}]}"#);
        assert!(result.is_err());
    }

    #[test]
    fn chat_request_missing_messages_fails() {
        let result: Result<ChatRequest, _> = from_str(r#"{"model":"m"}"#);
        assert!(result.is_err());
    }

    // ── ModelData serialization / deserialization ─────────────────────────────

    #[test]
    fn model_data_serializes_to_expected_shape() {
        let data = ModelData {
            id: "anthropic.claude-3".into(),
            object: "model".into(),
            created: 0,
            owned_by: "Anthropic".into(),
        };
        let v = to_value(&data).unwrap();
        assert_eq!(v["id"], "anthropic.claude-3");
        assert_eq!(v["object"], "model");
        assert_eq!(v["created"], 0);
        assert_eq!(v["owned_by"], "Anthropic");
    }

    #[test]
    fn model_data_round_trips_through_json() {
        let original = ModelData {
            id: "amazon.titan".into(),
            object: "model".into(),
            created: 1_700_000_000,
            owned_by: "Amazon".into(),
        };
        let json_str = serde_json::to_string(&original).unwrap();
        let restored: ModelData = from_str(&json_str).unwrap();
        assert_eq!(restored.id, original.id);
        assert_eq!(restored.created, original.created);
        assert_eq!(restored.owned_by, original.owned_by);
    }

    #[test]
    fn model_list_serializes_with_object_field() {
        let list = ModelList {
            object: "list".into(),
            data: vec![ModelData {
                id: "m".into(),
                object: "model".into(),
                created: 0,
                owned_by: "aws".into(),
            }],
        };
        let v = to_value(&list).unwrap();
        assert_eq!(v["object"], "list");
        assert_eq!(v["data"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn message_serializes_role_and_content() {
        let m = Message { role: "assistant".into(), content: "hello".into() };
        let v = to_value(&m).unwrap();
        assert_eq!(v, json!({"role": "assistant", "content": "hello"}));
    }
}
