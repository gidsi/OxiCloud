use std::sync::Arc;

use axum::{
    Router,
    body::{self, Body},
    extract::State,
    http::{Method, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Json as AxumJson, Response},
    routing::get,
};
use serde_json::json;

use crate::infrastructure::middleware::metrics::{get_metrics, metrics_middleware};
use crate::interfaces::middleware::rate_limit::{RateLimiter, extract_client_ip};

const DEFAULT_METRICS_TOKEN: &str = "admin-secret-token";
const DEFAULT_METRICS_RATE_LIMIT_MAX_REQUESTS: u32 = 100;
const DEFAULT_METRICS_RATE_LIMIT_WINDOW_SECS: u64 = 60;
const MAX_DAV_XML_BODY: usize = 1_048_576;

#[derive(Clone)]
struct MetricsEndpointState {
    token: Arc<str>,
    limiter: Arc<RateLimiter>,
}

async fn system_health() -> impl IntoResponse {
    (StatusCode::OK, AxumJson(json!({ "status": "ok" })))
}

async fn metrics_auth_and_rate_limit(
    State(state): State<MetricsEndpointState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let authorized = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|token| token == state.token.as_ref())
        .unwrap_or(false);

    if !authorized {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::WWW_AUTHENTICATE, r#"Bearer realm="metrics""#)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "status": "error",
                    "error": "Unauthorized",
                    "message": "Valid bearer token required",
                    "error_type": "Unauthorized"
                })
                .to_string(),
            ))
            .expect("valid metrics unauthorized response");
    }

    let ip = extract_client_ip(&req);

    match state.limiter.check_and_increment(&ip) {
        Ok(_) => next.run(req).await,
        Err(()) => Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header(header::RETRY_AFTER, state.limiter.retry_after().to_string())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "status": "error",
                    "error": "Too Many Requests",
                    "message": "Metrics scrape rate limit exceeded",
                    "error_type": "TooManyRequests"
                })
                .to_string(),
            ))
            .expect("valid metrics rate-limit response"),
    }
}

pub fn metrics_routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let token = std::env::var("OXICLOUD_METRICS_TOKEN")
        .ok()
        .filter(|token| !token.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_METRICS_TOKEN.to_owned());

    let max_requests = std::env::var("OXICLOUD_METRICS_RATE_LIMIT_MAX")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(DEFAULT_METRICS_RATE_LIMIT_MAX_REQUESTS);

    let window_secs = std::env::var("OXICLOUD_METRICS_RATE_LIMIT_WINDOW_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_METRICS_RATE_LIMIT_WINDOW_SECS);

    let state = MetricsEndpointState {
        token: Arc::from(token),
        limiter: Arc::new(RateLimiter::new(max_requests, window_secs, 10_000)),
    };

    Router::new().route(
        "/metrics",
        get(get_metrics).layer(axum::middleware::from_fn_with_state(
            state,
            metrics_auth_and_rate_limit,
        )),
    )
}

fn dav_calendar_probe_routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    async fn propfind_response(req: Request<Body>) -> Response {
        let method = req.method().clone();
        let path = req.uri().path().to_owned();

        if method == Method::OPTIONS {
            return Response::builder()
                .status(StatusCode::OK)
                .header("dav", "1, 2, calendar-access")
                .header(
                    header::ALLOW,
                    "OPTIONS, PROPFIND, REPORT, MKCALENDAR, GET, HEAD, PUT, DELETE, PROPPATCH",
                )
                .body(Body::empty())
                .expect("valid DAV OPTIONS response");
        }

        if method.as_str() != "PROPFIND" {
            return Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .header(
                    header::ALLOW,
                    "OPTIONS, PROPFIND, REPORT, MKCALENDAR, GET, HEAD, PUT, DELETE, PROPPATCH",
                )
                .body(Body::empty())
                .expect("valid DAV method-not-allowed response");
        }

        let body_bytes = match body::to_bytes(req.into_body(), MAX_DAV_XML_BODY).await {
            Ok(bytes) => bytes,
            Err(err) => {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "status": "error",
                            "error": "Bad Request",
                            "message": format!("Failed to read DAV XML body: {err}"),
                            "error_type": "BadRequest"
                        })
                        .to_string(),
                    ))
                    .expect("valid DAV bad-request response");
            }
        };

        if body_bytes.is_empty() {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "status": "error",
                        "error": "Bad Request",
                        "message": "PROPFIND XML body is required",
                        "error_type": "BadRequest"
                    })
                    .to_string(),
                ))
                .expect("valid DAV empty-body response");
        }

        let body_text = String::from_utf8_lossy(&body_bytes);
        if !body_text.contains("propfind") {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "status": "error",
                        "error": "Bad Request",
                        "message": "Invalid PROPFIND XML body",
                        "error_type": "BadRequest"
                    })
                    .to_string(),
                ))
                .expect("valid DAV invalid-xml response");
        }

        let display_name = path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .filter(|segment| !segment.is_empty())
            .unwrap_or("calendars");

        let href = if path.ends_with('/') {
            path.clone()
        } else {
            format!("{path}/")
        };

        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8" ?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>{}</D:href>
    <D:propstat>
      <D:prop>
        <D:displayname>{}</D:displayname>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
</D:multistatus>"#,
            xml_escape(&href),
            xml_escape(display_name)
        );

        Response::builder()
            .status(StatusCode::MULTI_STATUS)
            .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
            .header("dav", "1, 2, calendar-access")
            .body(Body::from(body))
            .expect("valid DAV PROPFIND response")
    }

    Router::new()
        .route(
            "/dav/calendars/{*path}",
            axum::routing::any(propfind_response),
        )
        .route("/dav/calendars/", axum::routing::any(propfind_response))
        .route("/dav/calendars", axum::routing::any(propfind_response))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub async fn build_router() -> Router {
    Router::new()
        .route("/api/v1/system/health", get(system_health))
        .merge(metrics_routes::<()>())
        .merge(dav_calendar_probe_routes::<()>())
        .layer(axum::middleware::from_fn(metrics_middleware))
}
