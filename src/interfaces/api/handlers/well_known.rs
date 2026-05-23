use axum::{
    body::Body,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;

use crate::startup::AppState;

pub fn well_known_router() -> Router<Arc<AppState>> {
    Router::new().route("/carddav", get(carddav_discovery))
}

pub async fn carddav_discovery() -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(header::LOCATION, "/dav/")
        .header(header::CONTENT_LENGTH, "0")
        .body(Body::empty())
        .expect("static CardDAV discovery redirect response must be valid")
}
