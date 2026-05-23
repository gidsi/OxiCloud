use axum::{routing::get, Router};
use std::sync::Arc;

use crate::infrastructure::state::AppState;
use crate::interfaces::api::handlers::{files, health, users};

pub fn app_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health::health_check))
        .nest("/api/v1/files", files::router())
        .nest("/api/v1/users", users::router())
        .with_state(state)
}
