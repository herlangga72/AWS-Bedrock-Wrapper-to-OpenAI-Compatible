use axum::{
    extract::State,
    response::IntoResponse,
    Json,
    debug_handler,
};
use serde_json::json;
use std::fs;

const CACHE_FILE: &str = "/tmp/bedrock_models_cache.json";
const CACHE_TTL_SECONDS: u64 = 3600;

#[debug_handler]
pub async fn list_models_handler(
    State(state): State<crate::AppState>,
) -> impl IntoResponse {
    if let Some(cached) = try_read_cache() {
        return Json(cached).into_response();
    }
    match state.mgmt_client.list_foundation_models().send().await {
        Ok(resp) => {
            let summaries = resp.model_summaries.unwrap_or_default();
            let mut data = Vec::with_capacity(summaries.len());

            for m in summaries {
                data.push(crate::models::ModelData {
                    id: m.model_id, 
                    object: "model".into(),
                    created: 0,
                    owned_by: m.provider_name.unwrap_or_else(|| "bedrock".into()),
                });
            }

            let response_body = crate::models::ModelList {
                object: "list".to_string(),
                data,
            };

            if let Ok(json_str) = serde_json::to_string(&response_body) {
                let _ = fs::write(CACHE_FILE, &*json_str); // &* → &str
            }

            Json(response_body).into_response()
        }

        Err(e) => {
            tracing::error!("Failed to list Bedrock models: {e}");
            Json(json!({
                "object": "error",
                "message": "Failed to retrieve models",
                "data": []
            })).into_response()
        }
    }
}

// Same cache reader (unchanged)
fn try_read_cache() -> Option<serde_json::Value> {
    let metadata = fs::metadata(CACHE_FILE).ok()?;
    let elapsed = metadata.modified().ok()?.elapsed().ok()?;
    if elapsed > std::time::Duration::from_secs(CACHE_TTL_SECONDS) {
        return None;
    }
    let contents = fs::read_to_string(CACHE_FILE).ok()?;
    serde_json::from_str(&contents).ok()
}