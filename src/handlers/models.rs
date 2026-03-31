use axum::{extract::State, response:: { Json, IntoResponse } };
use serde_json::{json, Value};
use tokio::fs; // Use async FS

const CACHE_FILE: &str = "/tmp/bedrock_models_cache.json";
const CACHE_TTL_SECONDS: u64 = 3600;

pub async fn list_models_handler(
    State(state): State<crate::AppState>,
) -> impl IntoResponse {
    // 1. Async cache check
    if let Some(cached) = try_read_cache().await {
        return Json(cached).into_response();
    }

    // 2. Fetch from AWS
    match state.mgmt_client.list_foundation_models().send().await {
        Ok(resp) => {
            let summaries = resp.model_summaries.unwrap_or_default();
            
            // 3. Map directly into the final vector to avoid double-allocation
            let data: Vec<_> = summaries
                .into_iter()
                .map(|m| crate::models::ModelData {
                    id: m.model_id,
                    object: "model".into(),
                    created: 0,
                    owned_by: m.provider_name.unwrap_or_else(|| "bedrock".into()),
                })
                .collect();

            let response_body = crate::models::ModelList {
                object: "list".to_string(),
                data,
            };

            // 4. Fire-and-forget cache write (Async)
            // Serializing to a Vec<u8> is often faster than String for FS writes
            if let Ok(bytes) = serde_json::to_vec(&response_body) {
                let _ = fs::write(CACHE_FILE, bytes).await;
            }

            Json(response_body).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list Bedrock models: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "object": "error", "message": "Failed to retrieve models" }))
            ).into_response()
        }
    }
}

async fn try_read_cache() -> Option<Value> {
    let metadata = fs::metadata(CACHE_FILE).await.ok()?;
    let modified = metadata.modified().ok()?;
    
    if modified.elapsed().ok()?.as_secs() > CACHE_TTL_SECONDS {
        return None;
    }

    let contents = fs::read(CACHE_FILE).await.ok()?;
    serde_json::from_slice(&contents).ok()
}