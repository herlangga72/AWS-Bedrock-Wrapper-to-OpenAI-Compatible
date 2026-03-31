use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub stream: Option<bool>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stop_sequences: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}


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