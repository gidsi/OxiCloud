use axum::{
    body::Body,
    http::{Request, StatusCode, header, Method},
};
use tower::ServiceExt;

// Import the actual main application router builder
use oxicloud::app::build_router; 

#[tokio::test]
async fn test_metrics_endpoint_integration_and_format() {
    let app = build_router().await;

    // Trigger a successful request to ensure routing metrics are recorded
    let api_req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/system/health")
        .body(Body::empty())
        .unwrap();
    let api_res = app.clone().oneshot(api_req).await.unwrap();
    assert_eq!(api_res.status(), StatusCode::OK);

    // Scenario 1 & 2: Request metrics with valid Auth (Bearer Token as per Security Constraints)
    let metrics_req = Request::builder()
        .method(Method::GET)
        .uri("/metrics")
        .header(header::AUTHORIZATION, "Bearer admin-secret-token") 
        .body(Body::empty())
        .unwrap();

    let metrics_res = app.clone().oneshot(metrics_req).await.unwrap();
    
    // Assert 200 OK
    assert_eq!(metrics_res.status(), StatusCode::OK);

    // Assert Content-Type is text/plain
    let content_type = metrics_res.headers().get(header::CONTENT_TYPE).unwrap();
    assert!(content_type.to_str().unwrap().contains("text/plain"));

    let body_bytes = axum::body::to_bytes(metrics_res.into_body(), usize::MAX).await.unwrap();
    let body_text = String::from_utf8_lossy(&body_bytes);

    // Assert standard Prometheus metrics exist
    assert!(body_text.contains("http_requests_total"));
    assert!(body_text.contains("http_request_duration_seconds"));
    
    // Assert correct label metadata was applied from the previous request
    assert!(body_text.contains(r#"path="/api/v1/system/health""#));
    assert!(body_text.contains(r#"method="GET""#));
    assert!(body_text.contains(r#"status="200""#));
}

#[tokio::test]
async fn test_metrics_endpoint_requires_authentication() {
    let app = build_router().await;

    // Security Constraint: Unauthenticated request to /metrics should fail
    let req = Request::builder()
        .method(Method::GET)
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_metrics_endpoint_is_rate_limited() {
    let app = build_router().await;

    let mut hit_rate_limit = false;
    
    // Security Constraint: Simulate multiple requests to exhaust the rate limit bucket
    for _ in 0..200 {
        let req = Request::builder()
            .method(Method::GET)
            .uri("/metrics")
            .header(header::AUTHORIZATION, "Bearer admin-secret-token")
            .body(Body::empty())
            .unwrap();

        let res = app.clone().oneshot(req).await.unwrap();
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            hit_rate_limit = true;
            break;
        }
    }

    assert!(hit_rate_limit, "The /metrics endpoint must be rate-limited to prevent CPU exhaustion via serializing DoS attacks.");
}

#[tokio::test]
async fn test_webdav_propfind_integration_is_unaffected() {
    let app = build_router().await;

    // Scenario 3: Valid PROPFIND request with XML body traversing the entire stack 
    let xml_payload = r#"<?xml version="1.0" encoding="utf-8" ?>
        <D:propfind xmlns:D="DAV:">
            <D:prop>
                <D:displayname/>
            </D:prop>
        </D:propfind>"#;

    let req = Request::builder()
        .method("PROPFIND")
        .uri("/dav/calendars/test_user/")
        .header(header::CONTENT_TYPE, "application/xml")
        .header(header::AUTHORIZATION, "Basic dGVzdF91c2VyOnBhc3N3b3Jk") 
        .body(Body::from(xml_payload))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    
    // WebDAV explicitly responds with 207 Multi-Status.
    // If the metrics middleware extracted the body before letting the request pass to WebDAV, 
    // the XML parsing will fail and return `400 Bad Request`.
    assert_eq!(
        res.status(), 
        StatusCode::MULTI_STATUS, 
        "WebDAV returned an error. Did the metrics middleware consume the request body payload?"
    );
}
