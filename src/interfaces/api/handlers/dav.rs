use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::IntoResponse,
    routing::any,
    Router,
};
use std::sync::Arc;

use crate::application::state::AppState;

pub fn dav_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", any(dav_handler))
        .route("/{*path}", any(dav_handler))
}

pub async fn dav_handler(
    State(_state): State<Arc<AppState>>,
    _req: Request,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        "WebDAV endpoint reachable",
    )
}
