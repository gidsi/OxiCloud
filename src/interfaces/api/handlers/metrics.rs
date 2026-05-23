use axum::{
    http::header,
    response::{IntoResponse, Response},
};

use crate::interfaces::api::middlewares::metrics::prometheus_handle;

const HTTP_REQUESTS_TOTAL_HELP: &str = "# HELP http_requests_total Total number of HTTP requests processed by OxiCloud.\n# TYPE http_requests_total counter\n";
const HTTP_REQUEST_DURATION_HELP: &str = "# HELP http_request_duration_seconds HTTP request duration in seconds, grouped by method, static route template, and status.\n# TYPE http_request_duration_seconds histogram\n";

pub async fn metrics() -> Response {
    let mut body = prometheus_handle().render();

    if !body.contains("http_requests_total") {
        body.push_str(HTTP_REQUESTS_TOTAL_HELP);
    }

    if !body.contains("http_request_duration_seconds") {
        body.push_str(HTTP_REQUEST_DURATION_HELP);
    }

    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
        .into_response()
}
