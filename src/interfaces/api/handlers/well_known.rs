use axum::{
    http::StatusCode,
    response::{IntoResponse, Redirect},
    routing::get,
    Router,
};
use std::sync::Arc;

use crate::app::AppState;

pub fn well_known_router() -> Router<Arc<AppState>> {
    Router::new().route("/caldav", get(caldav_discovery))
}

async fn caldav_discovery() -> impl IntoResponse {
    let mut response = Redirect::permanent("/dav/").into_response();

    // Axum 0.8 maps Redirect::permanent to 308 Permanent Redirect.
    // RFC 6764 CalDAV discovery and the project acceptance criteria require
    // 301 Moved Permanently with the same hardcoded Location target.
    *response.status_mut() = StatusCode::MOVED_PERMANENTLY;

    response
}
