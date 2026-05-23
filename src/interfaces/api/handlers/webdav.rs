use axum::{
    body::Body,
    extract::{Request, State},
    http::{Method, StatusCode},
    response::IntoResponse,
};
use std::sync::Arc;

use crate::application::state::AppState;

pub async fn handle_request(
    State(_state): State<Arc<AppState>>,
    req: Request<Body>,
) -> impl IntoResponse {
    if req.method() == Method::GET || req.method() == Method::HEAD {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::NOT_IMPLEMENTED
    }
}
