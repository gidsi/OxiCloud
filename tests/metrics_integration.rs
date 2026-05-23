use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::{Service, ServiceExt};
use uuid::Uuid;

// ============================================================================
// QA STUBS: These stand in for the actual application builders so this test
// file compiles out-of-the-box. The test is expected to FAIL.
//
// Developers: Replace these stubs with your actual module imports once you
// implement the `AppState`, `build_main_router`, and `build_metrics_router`.
// ============================================================================
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}

/// Stub for the primary application router (where traffic flows)
pub fn build_main_router(_state: AppState) -> Router {
    Router::new() 
}

/// Stub for the isolated metrics management router
pub fn build_metrics_router() -> Router {
    Router::new()
}
// ============================================================================

#[tokio::test]
async fn test_metrics_endpoint_returns_prometheus_format() {
    let metrics_app = build_metrics_router();

    let req = Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(Body::empty())
        .expect("Failed to build request");

    let response = metrics_app.oneshot(req).await.expect("Failed to execute request");

    // Must return 200 OK
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "The /metrics endpoint must respond with 200 OK"
    );

    let body_bytes = response.into_body().collect().await.expect("Failed to read body").to_bytes();
    let body_text = String::from_utf8(body_bytes.to_vec()).expect("Body is not valid UTF-8");

    // Verify expected Prometheus metrics are initialized and exposed
    assert!(
        body_text.contains("http_requests_total"),
        "Metrics body must contain 'http_requests_total'"
    );
    assert!(
        body_text.contains("http_request_duration_seconds"),
        "Metrics body must contain 'http_request_duration_seconds'"
    );
}

#[sqlx::test]
async fn test_metrics_cardinality_protection_1000_unmatched_requests(pool: PgPool) {
    let state = AppState { db: pool };
    let mut main_app = build_main_router(state.clone());
    let metrics_app = build_metrics_router();

    let mut generated_uuids = Vec::new();

    // 1. Fire 1,000 requests to highly randomized, non-existent URLs
    for _ in 0..1000 {
        let uuid = Uuid::new_v4().to_string();
        generated_uuids.push(uuid.clone());
        
        let path = format!("/remote.php/webdav/random_dir_{}/file_{}.txt", uuid, uuid);
        let req = Request::builder()
            .uri(&path)
            .method("GET")
            .body(Body::empty())
            .unwrap();

        // Using `ready().await.call(req)` to reuse the mutable router service in a loop
        let svc = main_app.ready().await.expect("Router not ready");
        let _response = svc.call(req).await.expect("Request failed");
        
        // Note: These should 404, but our primary concern here is the metrics system
    }

    // 2. Scrape the metrics
    let metrics_req = Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = metrics_app.oneshot(metrics_req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "Metrics endpoint must be reachable after traffic generation");

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_text = String::from_utf8(body_bytes.to_vec()).unwrap();

    // 3. Assert "UNMATCHED" route label is present
    let unmatched_label = r#"route="UNMATCHED""#;
    assert!(
        body_text.contains(unmatched_label),
        "Metrics must group 404s under the route=\"UNMATCHED\" label to prevent cardinality explosion."
    );

    // 4. Verify exactly 1,000 requests were recorded for the UNMATCHED route
    let has_1000_unmatched = body_text.lines().any(|line| {
        line.starts_with("http_requests_total") 
            && line.contains(unmatched_label) 
            && (line.contains(" 1000") || line.contains(" 1000.0"))
    });

    assert!(
        has_1000_unmatched, 
        "Metrics must record exactly 1000 requests for the UNMATCHED route. Output:\n{}", 
        body_text
    );

    // 5. CRITICAL SECURITY/CARDINALITY CHECK: Assert that ZERO random paths leaked into the metrics
    for uuid in generated_uuids {
        assert!(
            !body_text.contains(&uuid),
            "CARDINALITY EXPLOSION RISK: Found raw dynamically generated UUID '{}' in the metrics output. You must use MatchedPath!",
            uuid
        );
    }
}

#[sqlx::test]
async fn test_metrics_cardinality_protection_dynamic_matched_routes(pool: PgPool) {
    let state = AppState { db: pool };
    let mut main_app = build_main_router(state.clone());
    let metrics_app = build_metrics_router();

    // 1. Send requests to recognized WebDAV endpoints with dynamic file names
    let file1 = "taxes.pdf";
    let req1 = Request::builder()
        .uri(format!("/remote.php/webdav/Documents/{}", file1))
        .method("PUT")
        .body(Body::empty())
        .unwrap();
    
    let svc1 = main_app.ready().await.unwrap();
    let _ = svc1.call(req1).await.unwrap();

    let file2 = "vacation.jpg";
    let req2 = Request::builder()
        .uri(format!("/remote.php/webdav/Photos/{}", file2))
        .method("PUT")
        .body(Body::empty())
        .unwrap();

    let svc2 = main_app.ready().await.unwrap();
    let _ = svc2.call(req2).await.unwrap();

    // 2. Scrape the metrics
    let metrics_req = Request::builder()
        .uri("/metrics")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    
    let response = metrics_app.oneshot(metrics_req).await.unwrap();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_text = String::from_utf8(body_bytes.to_vec()).unwrap();

    // 3. Ensure the dynamic routes group under the static template label (e.g., "/remote.php/webdav/*path")
    let expected_route_label = r#"route="/remote.php/webdav/*path""#;
    assert!(
        body_text.contains(expected_route_label),
        "Metrics missing expected static route label. Expected: {}",
        expected_route_label
    );

    // 4. Ensure raw filenames DO NOT appear in metrics (PII / Security / Cardinality constraint)
    assert!(
        !body_text.contains(file1), 
        "Cardinality / PII leak: Found sensitive file name '{}' in metrics payload", file1
    );
    assert!(
        !body_text.contains(file2), 
        "Cardinality / PII leak: Found sensitive file name '{}' in metrics payload", file2
    );
}
