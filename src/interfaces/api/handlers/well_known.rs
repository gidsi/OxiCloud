use axum::{
    extract::Request,
    http::{
        header::ALLOW,
        HeaderValue, Method, StatusCode,
    },
    response::{IntoResponse, Redirect, Response},
    Router,
};
use std::convert::Infallible;
use tower::service_fn;

const CALDAV_DISCOVERY_TARGET: &str = "/dav/";
const CALDAV_DISCOVERY_ALLOW: &str = "GET, HEAD, PROPFIND";

pub fn well_known_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new().route_service("/caldav", service_fn(caldav_discovery_service))
}

async fn caldav_discovery_service(request: Request) -> Result<Response, Infallible> {
    let response = if is_supported_discovery_method(request.method()) {
        caldav_discovery().await.into_response()
    } else {
        method_not_allowed_response()
    };

    Ok(response)
}

fn is_supported_discovery_method(method: &Method) -> bool {
    method == Method::GET || method == Method::HEAD || method.as_str() == "PROPFIND"
}

fn method_not_allowed_response() -> Response {
    let mut response = StatusCode::METHOD_NOT_ALLOWED.into_response();
    response
        .headers_mut()
        .insert(ALLOW, HeaderValue::from_static(CALDAV_DISCOVERY_ALLOW));
    response
}

pub async fn caldav_discovery() -> impl IntoResponse {
    let mut response = Redirect::permanent(CALDAV_DISCOVERY_TARGET).into_response();

    // axum's permanent redirect helper may map to a permanent redirect variant
    // other than 301 depending on framework version. This story explicitly
    // requires 301 Moved Permanently while still using Redirect::permanent.
    *response.status_mut() = StatusCode::MOVED_PERMANENTLY;

    response
}
