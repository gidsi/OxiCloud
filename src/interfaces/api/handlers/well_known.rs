use crate::state::AppState;
use axum::{
    body::Body,
    extract::State,
    http::{
        header::{CACHE_CONTROL, LOCATION},
        HeaderValue, Response, StatusCode,
    },
    response::IntoResponse,
};
use std::sync::Arc;

pub async fn caldav_redirect(State(state): State<Arc<AppState>>) -> Response<Body> {
    dav_redirect_response(&state)
}

pub async fn carddav_redirect(State(state): State<Arc<AppState>>) -> Response<Body> {
    dav_redirect_response(&state)
}

fn dav_redirect_response(state: &AppState) -> Response<Body> {
    let location = format!("{}/dav/", state.config.base_url.trim_end_matches('/'));

    let location = match HeaderValue::from_str(&location) {
        Ok(location) => location,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    match Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(LOCATION, location)
        .header(CACHE_CONTROL, HeaderValue::from_static("no-cache"))
        .body(Body::empty())
    {
        Ok(response) => response,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
