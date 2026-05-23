use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    routing::get,
    Router,
};
use oxicloud::{
    infrastructure::state::AppState,
    interfaces::api::handlers::health::health_check,
};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use std::{env, sync::Arc};
use tower::ServiceExt;

fn get_test_db_url() -> String {
    env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/postgres".to_string())
}

#[tokio::test]
async fn test_health_check_passes_when_database_is_connected() {
    let db_url = get_test_db_url();
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Test setup failed: Cannot connect to the test database");

    let app_state = Arc::new(AppState { db_pool: pool });
    let app = Router::new()
        .route("/health", get(health_check))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Expected 200 OK when the database is successfully connected"
    );

    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body_json: Value = serde_json::from_slice(&body_bytes)
        .expect("Failed to parse response body as valid JSON");

    assert_eq!(body_json["status"], "pass");
    assert_eq!(body_json["database"], "connected");
}

#[tokio::test]
async fn test_health_check_fails_when_database_pool_is_closed() {
    let db_url = get_test_db_url();
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Test setup failed: Cannot connect to the test database");

    pool.close().await;

    let app_state = Arc::new(AppState { db_pool: pool });
    let app = Router::new()
        .route("/health", get(health_check))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "Expected 503 Service Unavailable when the database connection fails"
    );

    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert!(
        !body_str.to_lowercase().contains("pool closed"),
        "Security Violation: Handler leaked internal SQLx error details"
    );

    let body_json: Value = serde_json::from_str(&body_str)
        .expect("Failed to parse error response body as valid JSON");

    assert_eq!(body_json["status"], "fail");
    assert_eq!(body_json["database"], "disconnected");
}

#[tokio::test]
async fn test_health_check_fails_gracefully_on_network_timeout() {
    let broken_db_url = "postgres://postgres:password@10.255.255.255:5432/postgres";

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy(broken_db_url)
        .expect("Failed to create lazy pool");

    let app_state = Arc::new(AppState { db_pool: pool });
    let app = Router::new()
        .route("/health", get(health_check))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = tokio::time::timeout(std::time::Duration::from_secs(4), app.oneshot(request))
        .await
        .expect("Test Failed: The handler hung! Did you forget to wrap the SQL query in tokio::time::timeout() inside the handler?")
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "Expected 503 Service Unavailable when the database network times out"
    );

    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert!(
        !body_str.contains("10.255.255.255"),
        "Security Violation: Handler leaked internal database IP Address on timeout"
    );
    assert!(
        !body_str.to_lowercase().contains("timeout"),
        "Security Violation: Exact timeout stack or system failure reason was leaked"
    );

    let body_json: Value = serde_json::from_str(&body_str)
        .expect("Failed to parse error response body as valid JSON");

    assert_eq!(body_json["status"], "fail");
    assert_eq!(body_json["database"], "disconnected");
}
