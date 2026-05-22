use axum::{
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::any,
    Router,
};

pub fn well_known_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::<S>::new().route("/caldav", any(caldav_discovery))
}

pub async fn caldav_discovery() -> Response {
    let mut response = Redirect::permanent("/dav/").into_response();
    *response.status_mut() = StatusCode::MOVED_PERMANENTLY;
    response
}
