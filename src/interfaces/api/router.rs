use axum::routing::get;
use axum::Router;
use std::sync::Arc;

use crate::interfaces::api::handlers::health::health_check;
use crate::interfaces::api::handlers::metrics::metrics_handler;
use crate::startup::AppState;

pub fn app_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}
