//! File-based cache infrastructure

use arc_swap::ArcSwap;
use aws_sdk_bedrock::Client as MgmtClient;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;

use crate::domain::chat::{ModelData, ModelList};

/// Cache key for bedrock models
pub const BEDROCK_MODELS_KEY: &str = "bedrock_models";

/// Refresh the models cache from Bedrock
pub async fn refresh_models_cache(
    mgmt_client: &MgmtClient,
    file_cache: &Arc<ArcSwap<HashMap<String, Bytes>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let resp = mgmt_client.list_foundation_models().send().await?;

    let summaries = resp.model_summaries.unwrap_or_default();
    let data: Vec<ModelData> = summaries
        .into_iter()
        .filter(|m| {
            // Filter out models that require PROVISIONED throughput or INFERENCE_PROFILE
            // Only include models that support ON_DEMAND inference
            let supported_types = m.inference_types_supported.as_ref();
            let is_on_demand = supported_types.map_or(false, |types| {
                types.iter().any(|t| {
                    let s = t.as_str();
                    s == "ON_DEMAND" || s == "ON_DEMAND_USAGE"
                })
            });

            // Filter out embedding models (they use /v1/embeddings endpoint)
            let output_modalities = m.output_modalities.as_ref();
            let is_not_embedding = output_modalities.map_or(true, |mods| {
                !mods.iter().any(|m| m.as_str() == "EMBEDDING")
            });

            // Filter out image generation models (not chat models)
            let is_not_image = output_modalities.map_or(true, |mods| {
                !mods.iter().any(|m| m.as_str() == "IMAGE")
            });

            is_on_demand && is_not_embedding && is_not_image
        })
        .map(|m| ModelData {
            id: m.model_id,
            object: "model",
            created: 0,
            owned_by: m.provider_name.unwrap_or_else(|| "bedrock".into()),
        })
        .collect();

    let response_body = ModelList {
        object: "list",
        data,
    };
    let bytes = Bytes::from(serde_json::to_vec(&response_body)?);

    let mut new_map = HashMap::with_capacity(1);
    new_map.insert(BEDROCK_MODELS_KEY.to_string(), bytes.clone());
    file_cache.store(Arc::new(new_map));

    let _ = fs::write("/tmp/bedrock_models_cache.json", bytes).await;

    Ok(())
}

/// Run cache monitor that refreshes periodically
pub async fn run_cache_monitor(state: crate::shared::app_state::AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(3600));

    loop {
        interval.tick().await;
        if let Err(e) = refresh_models_cache(&state.mgmt_client, &state.file_cache).await {
            eprintln!("Cache refresh failed: {e}");
        }
    }
}
