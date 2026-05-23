use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;

use crate::application::state::AppState;

pub async fn health_check(_state: State<Arc<AppState>>) -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}
