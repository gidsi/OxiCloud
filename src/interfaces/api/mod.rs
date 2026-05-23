use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;

pub mod cookie_auth;
pub mod handlers;

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health_check() -> Response {
    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok",
        }),
    )
        .into_response()
}

pub fn create_health_routes() -> Router {
    Router::new().route("/health", get(health_check))
}

pub fn create_public_api_routes() -> Router {
    Router::new()
}

pub fn create_api_routes() -> Router {
    Router::new()
}
