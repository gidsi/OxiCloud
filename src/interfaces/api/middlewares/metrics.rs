use std::{sync::OnceLock, time::Instant};

use axum::{
    body::Body,
    extract::{MatchedPath, Request},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use metrics::{counter, describe_counter, describe_histogram, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

pub const UNMATCHED_ROUTE: &str = "UNMATCHED";

static PROMETHEUS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

pub fn prometheus_handle() -> &'static PrometheusHandle {
    PROMETHEUS_HANDLE.get_or_init(|| {
        let handle = PrometheusBuilder::new()
            .install_recorder()
            .expect("failed to install Prometheus metrics recorder");

        describe_counter!(
            "http_requests_total",
            "Total number of HTTP requests processed by OxiCloud."
        );

        describe_histogram!(
            "http_request_duration_seconds",
            "HTTP request duration in seconds, grouped by method, static route template, and status."
        );

        handle
    })
}

pub async fn record_http_metrics(req: Request<Body>, next: Next) -> Response {
    prometheus_handle();

    let start = Instant::now();
    let method = req.method().as_str().to_owned();

    let matched_route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|matched_path| normalize_matched_route(matched_path.as_str()))
        .unwrap_or_else(|| UNMATCHED_ROUTE.to_string());

    let response = next.run(req).await;
    let status = response.status();

    let route = if status == StatusCode::NOT_FOUND {
        UNMATCHED_ROUTE.to_string()
    } else {
        matched_route
    };

    let status_label = status.as_u16().to_string();
    let elapsed_seconds = start.elapsed().as_secs_f64();

    counter!(
        "http_requests_total",
        "method" => method.clone(),
        "route" => route.clone(),
        "status" => status_label.clone(),
    )
    .increment(1);

    histogram!(
        "http_request_duration_seconds",
        "method" => method,
        "route" => route,
        "status" => status_label,
    )
    .record(elapsed_seconds);

    response
}

fn normalize_matched_route(route: &str) -> String {
    route.replace("/{*path}", "/*path")
}
