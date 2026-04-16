//! Embedding domain - Text embedding generation

pub mod builder;
pub mod types;

pub use builder::NovaRequest;
pub use types::{
    NovaEmbeddingEntry, NovaResponse, OpenAiEmbeddingData, OpenAiEmbeddingRequest,
    OpenAiEmbeddingResponse, OpenAiUsage,
};
