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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::bedrock::test_provider;

    fn make_registry() -> ProviderRegistry {
        ProviderRegistry::new(test_provider())
    }

    #[test]
    fn bare_model_routes_to_bedrock_unchanged() {
        let (kind, id) = make_registry().provider_for("my-model");
        assert!(matches!(kind, crate::provider::ProviderKind::Bedrock));
        assert_eq!(id, "my-model");
    }

    #[test]
    fn bedrock_prefix_is_stripped() {
        let (kind, id) = make_registry().provider_for("bedrock/anthropic.claude-3");
        assert!(matches!(kind, crate::provider::ProviderKind::Bedrock));
        assert_eq!(id, "anthropic.claude-3");
    }

    #[test]
    fn extra_slashes_in_model_id_are_preserved() {
        // splitn(2, '/') only strips the first segment so inner slashes survive.
        let (_, id) = make_registry().provider_for("bedrock/us.anthropic.claude-3/v1");
        assert_eq!(id, "us.anthropic.claude-3/v1");
    }

    #[test]
    fn model_without_prefix_slash_routes_to_bedrock() {
        let (kind, id) = make_registry().provider_for("amazon.titan-text-express-v1");
        assert!(matches!(kind, crate::provider::ProviderKind::Bedrock));
        assert_eq!(id, "amazon.titan-text-express-v1");
    }

    #[test]
    fn empty_model_string_routes_to_bedrock() {
        let (kind, id) = make_registry().provider_for("");
        assert!(matches!(kind, crate::provider::ProviderKind::Bedrock));
        assert_eq!(id, "");
    }
}
