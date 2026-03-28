use axum::{debug_handler, extract::State, response::IntoResponse, Json};
use std::fs;

use crate::state::AppState;
use crate::types::openai::ModelList;

const CACHE_FILE: &str = "/tmp/bedrock_models_cache.json";
const CACHE_TTL_SECONDS: u64 = 3600;

#[debug_handler]
pub async fn list_models_handler(State(state): State<AppState>) -> impl IntoResponse {
    if let Some(cached) = try_read_cache() {
        return Json(cached).into_response();
    }

    let data = state.registry.list_models().await;
    let body = ModelList { object: "list".to_string(), data };

    if let Ok(json_str) = serde_json::to_string(&body) {
        let _ = fs::write(CACHE_FILE, &json_str);
    }

    Json(body).into_response()
}

fn try_read_cache() -> Option<serde_json::Value> {
    let meta = fs::metadata(CACHE_FILE).ok()?;
    let elapsed = meta.modified().ok()?.elapsed().ok()?;
    if elapsed > std::time::Duration::from_secs(CACHE_TTL_SECONDS) {
        return None;
    }
    serde_json::from_str(&fs::read_to_string(CACHE_FILE).ok()?).ok()
}
