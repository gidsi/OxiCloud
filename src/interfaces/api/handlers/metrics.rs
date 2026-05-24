use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use std::sync::Arc;

use crate::{
    domain::error::AppError,
    startup::AppState,
    telemetry::memory::render_prometheus_metrics,
};

pub async fn metrics_handler(State(_state): State<Arc<AppState>>) -> Response {
    let body = render_prometheus_metrics();

    match Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(body))
    {
        Ok(response) => response,
        Err(error) => {
            tracing::error!(
                error = %error,
                "failed to build prometheus metrics response"
            );
            AppError::InternalServerError.into_response()
        }
    }
}
