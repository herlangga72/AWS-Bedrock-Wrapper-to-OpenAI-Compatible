use super::{bedrock::BedrockProvider, ProviderKind};
use crate::types::openai::ModelData;

/// Routes requests to the appropriate provider based on the model name prefix.
///
/// Routing rules (first match wins):
/// - `bedrock/<model>` or no `/` prefix → [`BedrockProvider`]
///
/// # Adding a new provider
/// 1. Create `provider/<name>.rs` with a struct that has `chat`, `stream`, and `list_models`.
/// 2. Add it as a field below (e.g. `pub openai: HttpProvider`).
/// 3. Add a variant to [`ProviderKind`].
/// 4. Add a prefix check in [`ProviderRegistry::provider_for`].
/// 5. Delegate to `list_models` in [`ProviderRegistry::list_models`].
#[derive(Clone)]
pub struct ProviderRegistry {
    pub bedrock: BedrockProvider,
    // pub openai: HttpProvider,  // example future HTTP provider
}

impl ProviderRegistry {
    pub fn new(bedrock: BedrockProvider) -> Self {
        Self { bedrock }
    }

    /// Returns the provider kind and the model ID with the routing prefix stripped.
    ///
    /// Example: `"bedrock/anthropic.claude-3"` → `(Bedrock, "anthropic.claude-3")`
    pub fn provider_for(&self, model: &str) -> (ProviderKind, String) {
        let stripped = model.splitn(2, '/').last().unwrap_or(model).to_string();
        // future: if model.starts_with("openai/") { return (ProviderKind::OpenAI, stripped); }
        (ProviderKind::Bedrock, stripped)
    }

    /// Aggregates model lists from all registered providers.
    pub async fn list_models(&self) -> Vec<ModelData> {
        let models = self.bedrock.list_models().await;
        // future: models.extend(self.openai.list_models().await);
        models
    }
}
