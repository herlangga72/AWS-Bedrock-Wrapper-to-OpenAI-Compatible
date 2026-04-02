use crate::models::{ModelData, ModelList};
use crate::AppState;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
};
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;

pub async fn list_models_handler(State(state): State<AppState>) -> impl IntoResponse {
    let cache = state.file_cache.load();

    match cache.get("bedrock_models") {
        Some(bytes) => {
            ([(header::CONTENT_TYPE, "application/json")], bytes.clone()).into_response()
        }
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

pub async fn refresh_models_cache(
    state: &AppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let resp = state.mgmt_client.list_foundation_models().send().await?;

    let summaries = resp.model_summaries.unwrap_or_default();
    let data: Vec<ModelData> = summaries
        .into_iter()
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
    new_map.insert("bedrock_models".to_string(), bytes.clone());
    state.file_cache.store(Arc::new(new_map));

    let _ = fs::write("/tmp/bedrock_models_cache.json", bytes).await;

    Ok(())
}

pub(crate) async fn run_cache_monitor(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(3600));

    loop {
        interval.tick().await;
        if let Err(e) = refresh_models_cache(&state).await {
            eprintln!("Cache refresh failed: {e}");
        }
    }
}
