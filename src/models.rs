use axum::{
    extract::State,
    response::{IntoResponse, Response as AxumResponse},
    Json,
};
use serde::Serialize;
use crate::chat::AppState;

#[derive(Serialize)]
struct ModelData {
    id: String,
    object: String,
    created: u64,
    owned_by: String,
}

#[derive(Serialize)]
struct ModelList {
    object: String,
    data: Vec<ModelData>,
}

pub async fn list_models(
    State(state): State<AppState>,
) -> AxumResponse {
    match state
        .mgmt_client
        .list_foundation_models()
        // removed .by_output_modality(ModelModality::Text) to get all
        .send()
        .await
    {
        Ok(resp) => {
            let data: Vec<ModelData> = resp
                .model_summaries
                .unwrap_or_default()
                .into_iter()
                .map(|m| {
                    ModelData {
                        id: m.model_id, // use directly
                        object: "model".into(),
                        created: 0,
                        owned_by: m.provider_name.unwrap_or_else(|| "bedrock".into()),
                    }
                })
                .collect();

            Json(ModelList {
                object: "list".into(),
                data,
            })
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list models: {:?}", e);
            Json(serde_json::json!({
                "object": "list",
                "data": []
            }))
            .into_response()
        }
    }
}
