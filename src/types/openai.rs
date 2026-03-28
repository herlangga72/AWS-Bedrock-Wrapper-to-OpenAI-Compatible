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
