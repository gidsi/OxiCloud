use std::sync::OnceLock;
use std::time::Instant;

use axum::{
    body::Body,
    extract::MatchedPath,
    http::{HeaderValue, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use prometheus::{Encoder, HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry, TextEncoder};

static METRICS: OnceLock<HttpMetrics> = OnceLock::new();

struct HttpMetrics {
    registry: Registry,
    requests_total: IntCounterVec,
    request_duration_seconds: HistogramVec,
}

fn http_metrics() -> &'static HttpMetrics {
    METRICS.get_or_init(|| {
        let registry = Registry::new();

        let requests_total = IntCounterVec::new(
            Opts::new("http_requests_total", "Total number of HTTP requests."),
            &["method", "path", "status"],
        )
        .expect("http_requests_total metric definition is valid");

        let request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "http_request_duration_seconds",
                "HTTP request duration in seconds.",
            )
            .buckets(vec![
                0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
            &["method", "path", "status"],
        )
        .expect("http_request_duration_seconds metric definition is valid");

        registry
            .register(Box::new(requests_total.clone()))
            .expect("http_requests_total registration succeeds");

        registry
            .register(Box::new(request_duration_seconds.clone()))
            .expect("http_request_duration_seconds registration succeeds");

        HttpMetrics {
            registry,
            requests_total,
            request_duration_seconds,
        }
    })
}

pub async fn metrics_middleware(req: Request<Body>, next: Next) -> Response {
    let method = req.method().as_str().to_owned();

    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|matched_path| matched_path.as_str().to_owned())
        .unwrap_or_else(|| "unknown".to_owned());

    let start = Instant::now();

    let response = next.run(req).await;

    let status = response.status().as_u16().to_string();
    let elapsed = start.elapsed().as_secs_f64();

    let metrics = http_metrics();

    metrics
        .requests_total
        .with_label_values(&[method.as_str(), path.as_str(), status.as_str()])
        .inc();

    metrics
        .request_duration_seconds
        .with_label_values(&[method.as_str(), path.as_str(), status.as_str()])
        .observe(elapsed);

    response
}

pub async fn get_metrics() -> Response {
    let metrics = http_metrics();
    let encoder = TextEncoder::new();
    let metric_families = metrics.registry.gather();

    let mut buffer = Vec::new();

    if let Err(err) = encoder.encode(&metric_families, &mut buffer) {
        tracing::error!(error = %err, "failed to encode Prometheus metrics");
        return (StatusCode::INTERNAL_SERVER_ERROR, "failed to encode metrics").into_response();
    }

    let mut response = Response::new(Body::from(buffer));

    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
    );

    response
}
