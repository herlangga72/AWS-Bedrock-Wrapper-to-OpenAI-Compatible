//! Model capability registry for AWS Bedrock and Cloudflare models
//! Maps OpenAI-compatible parameters to model-specific parameters
//! Uses lowest common denominator approach

use serde::{Deserialize, Serialize};

/// AI vendor enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vendor {
    AwsBedrock,
    Cloudflare,
}

impl Vendor {
    pub fn from_model_id(model_id: &str) -> Option<Vendor> {
        let model_lower = model_id.to_lowercase();
        if model_lower.starts_with("@cf/") {
            Some(Vendor::Cloudflare)
        } else if model_lower.contains("anthropic.")
            || model_lower.contains("deepseek")
            || model_lower.contains("cohere.")
            || model_lower.contains("ai21.")
            || model_lower.contains("mistral")
            || model_lower.contains("meta.")
            || model_lower.contains("amazon.")
        {
            Some(Vendor::AwsBedrock)
        } else {
            None
        }
    }
}

/// Base parameters supported by models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseParams {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub stop_sequences: Option<String>,
}

/// Model-specific parameters mapped from OpenAI to provider format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSpecificParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repetition_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_params: Option<serde_json::Value>,
}

/// Thinking configuration for Claude extended thinking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    pub enabled: bool,
    pub budget_tokens: Option<u32>,
}

/// Model capabilities and parameter mappings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub provider: &'static str,
    pub vendor: Vendor,
    pub model_id_pattern: &'static str,
    pub uses_converse_api: bool,
    pub supports_thinking: bool,
    pub supports_reasoning: bool,
    pub base_params: BaseParams,
    pub model_specific: ModelSpecificParams,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfig>,
}

impl ModelCapabilities {
    pub fn matches(&self, model_id: &str) -> bool {
        model_id.to_lowercase().contains(self.model_id_pattern)
    }
}

// =============================================================================
// MODEL REGISTRY
// =============================================================================

pub fn get_model_capabilities(model_id: &str) -> Option<ModelCapabilities> {
    let model_lower = model_id.to_lowercase();

    // Check Cloudflare first (prefix @cf/)
    if model_lower.starts_with("@cf/") {
        return Some(cloudflare_capabilities(&model_lower));
    }

    // AWS Bedrock models
    if model_lower.contains("anthropic.claude") {
        Some(claude_capabilities(&model_lower))
    } else if model_lower.contains("deepseek") {
        Some(deepseek_capabilities(&model_lower))
    } else if model_lower.contains("cohere.command") {
        Some(cohere_command_capabilities())
    } else if model_lower.contains("ai21.j2") || model_lower.contains("ai21.jurassic") {
        Some(ai21_jurassic_capabilities())
    } else if model_lower.contains("mistral") {
        Some(mistral_capabilities())
    } else if model_lower.contains("meta.llama") || model_lower.contains("llama") {
        Some(meta_llama_capabilities())
    } else if model_lower.contains("amazon.titan") {
        Some(amazon_titan_capabilities())
    } else if model_lower.contains("amazon.nova") {
        Some(amazon_nova_capabilities())
    } else {
        None
    }
}

fn claude_capabilities(model_id: &str) -> ModelCapabilities {
    let supports_thinking = model_id.contains("claude-opus-4-5")
        || model_id.contains("claude-sonnet-4-5")
        || model_id.contains("claude-haiku-4-5")
        || model_id.contains("claude-3-7-sonnet")
        || model_id.contains("claude-3-5-sonnet");

    ModelCapabilities {
        provider: "anthropic",
        vendor: Vendor::AwsBedrock,
        model_id_pattern: "anthropic.claude",
        uses_converse_api: true,
        supports_thinking,
        supports_reasoning: false,
        base_params: BaseParams {
            max_tokens: Some(4096),
            temperature: Some(1.0),
            top_p: Some(0.9),
            stop_sequences: None,
        },
        model_specific: ModelSpecificParams {
            top_k: Some(250),
            frequency_penalty: None,
            presence_penalty: None,
            repetition_penalty: None,
            count_penalty: None,
            additional_params: None,
        },
        thinking_config: if supports_thinking {
            Some(ThinkingConfig {
                enabled: false,
                budget_tokens: Some(4000),
            })
        } else {
            None
        },
    }
}

fn deepseek_capabilities(model_id: &str) -> ModelCapabilities {
    ModelCapabilities {
        provider: "deepseek",
        vendor: Vendor::AwsBedrock,
        model_id_pattern: "deepseek.r1",
        uses_converse_api: true,
        supports_thinking: false,
        supports_reasoning: model_id.contains("r1"),
        base_params: BaseParams {
            max_tokens: Some(8192),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_sequences: None,
        },
        model_specific: ModelSpecificParams {
            top_k: None,
            frequency_penalty: None,
            presence_penalty: None,
            repetition_penalty: None,
            count_penalty: None,
            additional_params: None,
        },
        thinking_config: None,
    }
}

fn cohere_command_capabilities() -> ModelCapabilities {
    ModelCapabilities {
        provider: "cohere",
        vendor: Vendor::AwsBedrock,
        model_id_pattern: "cohere.command",
        uses_converse_api: false,
        supports_thinking: false,
        supports_reasoning: false,
        base_params: BaseParams {
            max_tokens: Some(2048),
            temperature: Some(0.3),
            top_p: Some(0.75),
            stop_sequences: None,
        },
        model_specific: ModelSpecificParams {
            top_k: Some(250),
            frequency_penalty: Some(0.0),
            presence_penalty: Some(0.0),
            repetition_penalty: None,
            count_penalty: None,
            additional_params: None,
        },
        thinking_config: None,
    }
}

fn ai21_jurassic_capabilities() -> ModelCapabilities {
    ModelCapabilities {
        provider: "ai21",
        vendor: Vendor::AwsBedrock,
        model_id_pattern: "ai21.jurassic",
        uses_converse_api: false,
        supports_thinking: false,
        supports_reasoning: false,
        base_params: BaseParams {
            max_tokens: Some(2048),
            temperature: Some(0.5),
            top_p: Some(0.5),
            stop_sequences: None,
        },
        model_specific: ModelSpecificParams {
            top_k: None,
            frequency_penalty: Some(0.0),
            presence_penalty: Some(0.0),
            repetition_penalty: None,
            count_penalty: Some(0.0),
            additional_params: None,
        },
        thinking_config: None,
    }
}

fn mistral_capabilities() -> ModelCapabilities {
    ModelCapabilities {
        provider: "mistral",
        vendor: Vendor::AwsBedrock,
        model_id_pattern: "mistral",
        uses_converse_api: false,
        supports_thinking: false,
        supports_reasoning: false,
        base_params: BaseParams {
            max_tokens: Some(4096),
            temperature: Some(0.5),
            top_p: Some(0.9),
            stop_sequences: None,
        },
        model_specific: ModelSpecificParams {
            top_k: Some(50),
            frequency_penalty: None,
            presence_penalty: None,
            repetition_penalty: None,
            count_penalty: None,
            additional_params: None,
        },
        thinking_config: None,
    }
}

fn meta_llama_capabilities() -> ModelCapabilities {
    ModelCapabilities {
        provider: "meta",
        vendor: Vendor::AwsBedrock,
        model_id_pattern: "meta.llama",
        uses_converse_api: false,
        supports_thinking: false,
        supports_reasoning: false,
        base_params: BaseParams {
            max_tokens: Some(512),
            temperature: Some(0.5),
            top_p: Some(0.9),
            stop_sequences: None,
        },
        model_specific: ModelSpecificParams {
            top_k: None,
            frequency_penalty: None,
            presence_penalty: None,
            repetition_penalty: None,
            count_penalty: None,
            additional_params: None,
        },
        thinking_config: None,
    }
}

fn amazon_titan_capabilities() -> ModelCapabilities {
    ModelCapabilities {
        provider: "amazon",
        vendor: Vendor::AwsBedrock,
        model_id_pattern: "amazon.titan",
        uses_converse_api: true,
        supports_thinking: false,
        supports_reasoning: false,
        base_params: BaseParams {
            max_tokens: Some(2048),
            temperature: Some(0.5),
            top_p: Some(0.9),
            stop_sequences: None,
        },
        model_specific: ModelSpecificParams {
            top_k: None,
            frequency_penalty: None,
            presence_penalty: None,
            repetition_penalty: None,
            count_penalty: None,
            additional_params: None,
        },
        thinking_config: None,
    }
}

fn amazon_nova_capabilities() -> ModelCapabilities {
    ModelCapabilities {
        provider: "amazon",
        vendor: Vendor::AwsBedrock,
        model_id_pattern: "amazon.nova",
        uses_converse_api: true,
        supports_thinking: false,
        supports_reasoning: false,
        base_params: BaseParams {
            max_tokens: Some(4096),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_sequences: None,
        },
        model_specific: ModelSpecificParams {
            top_k: None,
            frequency_penalty: None,
            presence_penalty: None,
            repetition_penalty: None,
            count_penalty: None,
            additional_params: None,
        },
        thinking_config: None,
    }
}

fn cloudflare_capabilities(model_id: &str) -> ModelCapabilities {
    // Extract the model slug from @cf/ prefix for display
    let model_slug = model_id.strip_prefix("@cf/").unwrap_or(model_id);
    let provider = if model_slug.contains("meta/") {
        "meta"
    } else if model_slug.contains("deepseek-ai/") {
        "deepseek"
    } else if model_slug.contains("mistral/") {
        "mistral"
    } else if model_slug.contains("google/") {
        "google"
    } else {
        "cloudflare"
    };

    ModelCapabilities {
        provider,
        vendor: Vendor::Cloudflare,
        model_id_pattern: "@cf/",
        uses_converse_api: false,
        supports_thinking: false,
        supports_reasoning: false,
        base_params: BaseParams {
            max_tokens: Some(256),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_sequences: None,
        },
        model_specific: ModelSpecificParams {
            top_k: None,
            frequency_penalty: None,
            presence_penalty: None,
            repetition_penalty: None,
            count_penalty: None,
            additional_params: None,
        },
        thinking_config: None,
    }
}

// =============================================================================
// PARAMETER MAPPING
// =============================================================================

pub fn map_openai_params(
    model_id: &str,
    temperature: Option<f32>,
    top_p: Option<f32>,
    max_tokens: Option<u32>,
    stop_sequences: Option<String>,
    frequency_penalty: Option<f32>,
    presence_penalty: Option<f32>,
    top_k: Option<i32>,
) -> (BaseParams, Option<serde_json::Value>) {
    let caps = get_model_capabilities(model_id);

    let base = BaseParams {
        max_tokens: max_tokens.or(caps.as_ref().and_then(|c| c.base_params.max_tokens)),
        temperature: temperature.or(caps.as_ref().and_then(|c| c.base_params.temperature)),
        top_p: top_p.or(caps.as_ref().and_then(|c| c.base_params.top_p)),
        stop_sequences,
    };

    let mut additional = serde_json::Map::new();

    if let Some(ref c) = caps {
        if c.model_specific.top_k.is_some() {
            if let Some(k) = top_k {
                additional.insert("top_k".to_string(), serde_json::json!(k));
            }
        }

        if let Some(fp) = frequency_penalty {
            if c.provider == "cohere" {
                additional.insert("frequency_penalty".to_string(), serde_json::json!(fp));
            } else if c.provider == "ai21" {
                additional.insert(
                    "frequencyPenalty".to_string(),
                    serde_json::json!({"scale": fp}),
                );
            }
        }

        if let Some(pp) = presence_penalty {
            if c.provider == "cohere" {
                additional.insert("presence_penalty".to_string(), serde_json::json!(pp));
            } else if c.provider == "ai21" {
                additional.insert(
                    "presencePenalty".to_string(),
                    serde_json::json!({"scale": pp}),
                );
            }
        }
    }

    let additional_params = if additional.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(additional))
    };

    (base, additional_params)
}

#[cfg(test)]
mod tests {
    use super::*;

    // =============================================================================
    // Model Matching Tests
    // =============================================================================

    #[test]
    fn test_claude_model_matching() {
        let caps = get_model_capabilities("anthropic.claude-3-5-sonnet-20240620-v1:0").unwrap();
        assert!(caps.supports_thinking);

        let caps = get_model_capabilities("anthropic.claude-sonnet-4-5-20250929-v1:0").unwrap();
        assert!(caps.supports_thinking);
        assert!(caps.thinking_config.is_some());
    }

    #[test]
    fn test_claude_various_models() {
        // Opus 4.5 supports thinking
        let caps = get_model_capabilities("anthropic.claude-opus-4-5-20261111-v1:0").unwrap();
        assert!(caps.supports_thinking);
        assert!(caps.thinking_config.is_some());
        assert_eq!(caps.thinking_config.as_ref().unwrap().budget_tokens, Some(4000));

        // Haiku 4.5 supports thinking
        let caps = get_model_capabilities("anthropic.claude-haiku-4-5-20261111-v1:0").unwrap();
        assert!(caps.supports_thinking);

        // 3.7 Sonnet supports thinking
        let caps = get_model_capabilities("anthropic.claude-3-7-sonnet-20250620-v1:0").unwrap();
        assert!(caps.supports_thinking);
    }

    #[test]
    fn test_claude_older_models_no_thinking() {
        // Older Claude 3 models don't support extended thinking
        let caps = get_model_capabilities("anthropic.claude-3-opus-20240229-v1:0").unwrap();
        assert!(!caps.supports_thinking);
        assert!(caps.thinking_config.is_none());

        let caps = get_model_capabilities("anthropic.claude-3-sonnet-20240229-v1:0").unwrap();
        assert!(!caps.supports_thinking);
    }

    #[test]
    fn test_deepseek_model_matching() {
        let caps = get_model_capabilities("deepseek.r1-v1:0").unwrap();
        assert!(caps.supports_reasoning);
        assert!(!caps.supports_thinking);
        assert!(caps.uses_converse_api);
    }

    #[test]
    fn test_deepseek_non_r1_no_reasoning() {
        let caps = get_model_capabilities("deepseek.chat-v2-20241111-v1:0").unwrap();
        assert!(!caps.supports_reasoning);
    }

    #[test]
    fn test_cohere_model_matching() {
        let caps = get_model_capabilities("cohere.command-r-v1:0").unwrap();
        assert_eq!(caps.provider, "cohere");
        assert!(!caps.uses_converse_api);

        let caps = get_model_capabilities("cohere.command-r-plus-v1:0").unwrap();
        assert_eq!(caps.provider, "cohere");
    }

    #[test]
    fn test_cohere_params() {
        let caps = get_model_capabilities("cohere.command-r-v1:0").unwrap();
        assert!(caps.model_specific.top_k.is_some());
        assert!(caps.model_specific.frequency_penalty.is_some());
        assert!(caps.model_specific.presence_penalty.is_some());
    }

    #[test]
    fn test_ai21_jurassic_matching() {
        let caps = get_model_capabilities("ai21.j2-mid-v1").unwrap();
        assert_eq!(caps.provider, "ai21");

        let caps = get_model_capabilities("ai21.jurassic-2-mid-v1").unwrap();
        assert_eq!(caps.provider, "ai21");
    }

    #[test]
    fn test_jurassic_no_topk() {
        let caps = get_model_capabilities("ai21.j2-mid-v1").unwrap();
        assert!(caps.model_specific.top_k.is_none());
        assert!(caps.model_specific.frequency_penalty.is_some());
        assert!(caps.model_specific.presence_penalty.is_some());
        assert!(caps.model_specific.count_penalty.is_some());
    }

    #[test]
    fn test_mistral_matching() {
        let caps = get_model_capabilities("mistral.mistral-7b-instruct-v0:0").unwrap();
        assert_eq!(caps.provider, "mistral");
        assert!(!caps.uses_converse_api);
    }

    #[test]
    fn test_mistral_has_topk() {
        let caps = get_model_capabilities("mistral.mistral-large-2407-v1:0").unwrap();
        assert!(caps.model_specific.top_k.is_some());
        assert!(caps.model_specific.frequency_penalty.is_none());
    }

    #[test]
    fn test_llama_matching() {
        let caps = get_model_capabilities("meta.llama3-1-70b-instruct-v1:0").unwrap();
        assert_eq!(caps.provider, "meta");

        let caps = get_model_capabilities("llama-3-1-70b-instruct-v1:0").unwrap();
        assert_eq!(caps.provider, "meta");
    }

    #[test]
    fn test_llama_no_topk() {
        let caps = get_model_capabilities("meta.llama3-1-70b-instruct-v1:0").unwrap();
        assert!(caps.model_specific.top_k.is_none());
        assert!(caps.model_specific.frequency_penalty.is_none());
    }

    #[test]
    fn test_amazon_titan_matching() {
        let caps = get_model_capabilities("amazon.titan-text-lite-v1").unwrap();
        assert_eq!(caps.provider, "amazon");
        assert!(caps.uses_converse_api);
    }

    #[test]
    fn test_amazon_nova_matching() {
        let caps = get_model_capabilities("amazon.nova-pro-v1:0").unwrap();
        assert_eq!(caps.provider, "amazon");
        assert!(caps.uses_converse_api);

        let caps = get_model_capabilities("amazon.nova-lite-v1:0").unwrap();
        assert_eq!(caps.provider, "amazon");
    }

    #[test]
    fn test_unknown_model_returns_none() {
        let caps = get_model_capabilities("unknown.model-v1:0");
        assert!(caps.is_none());
    }

    #[test]
    fn test_cloudflare_model_matching() {
        let caps = get_model_capabilities("@cf/meta/llama-3.1-8b-instruct").unwrap();
        assert_eq!(caps.vendor, Vendor::Cloudflare);
        assert_eq!(caps.provider, "meta");
        assert!(!caps.uses_converse_api);
        assert!(!caps.supports_thinking);
        assert!(!caps.supports_reasoning);
    }

    #[test]
    fn test_cloudflare_deepseek_model() {
        let caps = get_model_capabilities("@cf/deepseek-ai/deepseek-r1-distill-qwen-32b").unwrap();
        assert_eq!(caps.vendor, Vendor::Cloudflare);
        assert_eq!(caps.provider, "deepseek");
    }

    #[test]
    fn test_cloudflare_model_defaults() {
        let caps = get_model_capabilities("@cf/mistral/mistral-7b-instruct").unwrap();
        assert_eq!(caps.vendor, Vendor::Cloudflare);
        assert_eq!(caps.base_params.max_tokens, Some(256));
        assert_eq!(caps.base_params.temperature, Some(0.7));
    }

    #[test]
    fn test_vendor_enum() {
        assert_eq!(Vendor::from_model_id("@cf/meta/llama-3.1-8b-instruct"), Some(Vendor::Cloudflare));
        assert_eq!(Vendor::from_model_id("anthropic.claude-3-5-sonnet-20240620-v1:0"), Some(Vendor::AwsBedrock));
        assert_eq!(Vendor::from_model_id("unknown.model"), None);
    }

    #[test]
    fn test_model_matches_method() {
        let caps = get_model_capabilities("anthropic.claude-sonnet-4-5-20250929-v1:0").unwrap();
        assert!(caps.matches("anthropic.claude-sonnet-4-5-20250929-v1:0"));
        assert!(caps.matches("ANTHROPIC.CLAUDE-SONNET-4-5-20250929-V1:0")); // case insensitive
    }

    // =============================================================================
    // Parameter Mapping Tests
    // =============================================================================

    #[test]
    fn test_map_openai_params_defaults() {
        let (base, additional) = map_openai_params(
            "anthropic.claude-sonnet-4-5-20250929-v1:0",
            None, None, None, None, None, None, None,
        );

        // Should use model defaults
        assert_eq!(base.max_tokens, Some(4096));
        assert_eq!(base.temperature, Some(1.0));
        assert_eq!(base.top_p, Some(0.9));
        assert!(additional.is_none());
    }

    #[test]
    fn test_map_openai_params_override() {
        let (base, _) = map_openai_params(
            "anthropic.claude-sonnet-4-5-20250929-v1:0",
            Some(0.5),  // temperature
            Some(0.8),  // top_p
            Some(2000),  // max_tokens
            None, None, None, None,
        );

        // Should use provided values, not defaults
        assert_eq!(base.temperature, Some(0.5));
        assert_eq!(base.top_p, Some(0.8));
        assert_eq!(base.max_tokens, Some(2000));
    }

    #[test]
    fn test_map_openai_params_cohere_top_k() {
        let (_, additional) = map_openai_params(
            "cohere.command-r-v1:0",
            None, None, None, None, None, None,
            Some(100), // top_k
        );

        assert!(additional.is_some());
        let add_val = additional.unwrap();
        assert!(add_val.get("top_k").is_some());
    }

    #[test]
    fn test_map_openai_params_cohere_frequency_penalty() {
        let (_, additional) = map_openai_params(
            "cohere.command-r-v1:0",
            None, None, None, None,
            Some(0.5), // frequency_penalty
            None, None,
        );

        assert!(additional.is_some());
        let add_val = additional.unwrap();
        assert!(add_val.get("frequency_penalty").is_some());
    }

    #[test]
    fn test_map_openai_params_ai21_frequency_penalty() {
        let (_, additional) = map_openai_params(
            "ai21.j2-mid-v1",
            None, None, None, None,
            Some(0.5), // frequency_penalty
            None, None,
        );

        assert!(additional.is_some());
        let add_val = additional.unwrap();
        // AI21 uses different format: frequencyPenalty: {scale: 0.5}
        assert!(add_val.get("frequencyPenalty").is_some());
    }

    #[test]
    fn test_map_openai_params_stop_sequences() {
        let (base, _) = map_openai_params(
            "anthropic.claude-sonnet-4-5-20250929-v1:0",
            None, None, None,
            Some("END".to_string()), // stop_sequences
            None, None, None,
        );

        assert_eq!(base.stop_sequences, Some("END".to_string()));
    }

    #[test]
    fn test_map_openai_params_unsupported_model() {
        let (base, additional) = map_openai_params(
            "unknown.model-v1:0",
            Some(0.5),
            None, None, None, None, None, None,
        );

        // Should use provided values but no additional params
        assert_eq!(base.temperature, Some(0.5));
        assert!(additional.is_none());
    }
}
