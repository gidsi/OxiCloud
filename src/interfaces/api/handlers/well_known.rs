use std::sync::Arc;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Redirect},
    routing::get,
    Router,
};

use crate::app::AppState;

pub fn well_known_router() -> Router<Arc<AppState>> {
    Router::new().route("/caldav", get(caldav_discovery))
}

async fn caldav_discovery() -> impl IntoResponse {
    let mut response = Redirect::permanent("/dav/").into_response();

    // Axum's permanent redirect helper returns a permanent redirect response while
    // preserving a hardcoded Location header. OxiCloud's CalDAV discovery contract
    // specifically requires 301 Moved Permanently for /.well-known/caldav.
    *response.status_mut() = StatusCode::MOVED_PERMANENTLY;

    response
}
