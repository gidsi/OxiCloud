use axum::Router;
use oxicloud::{
    interfaces::api::router::app_router,
    startup::AppState,
    telemetry::memory::initialize_process_memory_metrics,
};
use reqwest::Client;
use sqlx::postgres::PgPoolOptions;
use std::{sync::Arc, time::Instant};
use tokio::net::TcpListener;

async fn spawn_app() -> String {
    initialize_process_memory_metrics().await;

    let db_pool = PgPoolOptions::new()
        .connect_lazy("postgres://postgres:password@localhost:5432/oxicloud")
        .expect("failed to create lazy Postgres pool for metrics integration test");

    let app: Router = app_router(Arc::new(AppState { db_pool }));

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind random port");
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://127.0.0.1:{port}")
}

#[tokio::test]
async fn metrics_endpoint_exposes_process_resident_memory_bytes() {
    let app_address = spawn_app().await;
    let client = Client::new();

    let response = client
        .get(format!("{app_address}/metrics"))
        .send()
        .await
        .expect("failed to execute GET request");

    assert_eq!(
        response.status().as_u16(),
        200,
        "expected 200 OK from /metrics"
    );

    let body = response.text().await.expect("failed to read response body");

    assert!(
        body.contains("process_resident_memory_bytes"),
        "the /metrics response is missing the process_resident_memory_bytes gauge. Body:\n{body}"
    );

    let mut found_metric_value = false;

    for line in body.lines() {
        if line.starts_with("process_resident_memory_bytes") && !line.starts_with('#') {
            let parts: Vec<&str> = line.split_whitespace().collect();

            assert!(
                parts.len() >= 2,
                "metric line was malformed, expected key and value: {line}"
            );

            let value: f64 = parts[1]
                .parse()
                .expect("metric value must be a valid number");

            assert!(
                value > 0.0,
                "process resident memory must be > 0 bytes"
            );

            found_metric_value = true;
            break;
        }
    }

    assert!(
        found_metric_value,
        "could not extract a valid numerical value for process_resident_memory_bytes"
    );
}

#[tokio::test]
async fn metrics_endpoint_responds_rapidly_under_limits_without_blocking() {
    let app_address = spawn_app().await;
    let client = Client::new();

    let _ = client.get(format!("{app_address}/metrics")).send().await;

    let start_time = Instant::now();

    let response = client
        .get(format!("{app_address}/metrics"))
        .send()
        .await
        .expect("failed to execute GET request");

    let elapsed = start_time.elapsed();

    assert_eq!(
        response.status().as_u16(),
        200,
        "expected 200 OK for latency benchmark"
    );

    assert!(
        elapsed.as_millis() < 50,
        "metrics endpoint took too long to respond: {}ms; expected < 50ms",
        elapsed.as_millis()
    );
}
