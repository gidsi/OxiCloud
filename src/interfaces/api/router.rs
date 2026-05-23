use axum::{
    http::StatusCode,
    middleware,
    routing::{any, get},
    Router,
};
use std::sync::Arc;

use crate::application::state::AppState;
use crate::interfaces::api::{
    handlers::{health, metrics, webdav},
    middlewares::metrics::{prometheus_handle, record_http_metrics},
};

pub fn app_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health::health_check))
        .route("/remote.php/webdav/{*path}", any(webdav::handle_request))
        .route("/webdav/{*path}", any(webdav::handle_request))
        .fallback(not_found)
        .with_state(state)
        .layer(middleware::from_fn(record_http_metrics))
}

pub fn build_main_router(state: AppState) -> Router {
    app_router(Arc::new(state))
}

pub fn build_metrics_router() -> Router {
    prometheus_handle();

    Router::new().route("/metrics", get(metrics::metrics))
}

async fn not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}
