use axum::{
    body::Body,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;

use crate::application::state::AppState;

const DAV_ROOT_PATH: &str = "/dav/";

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/caldav", get(caldav_redirect))
        .route("/carddav", get(carddav_discovery))
}

async fn caldav_redirect() -> impl IntoResponse {
    dav_root_redirect_response()
}

async fn carddav_discovery() -> impl IntoResponse {
    dav_root_redirect_response()
}

fn dav_root_redirect_response() -> Response {
    Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(header::LOCATION, DAV_ROOT_PATH)
        .body(Body::empty())
        .expect("static well-known DAV redirect response must be valid")
}
