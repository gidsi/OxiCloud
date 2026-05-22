use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use oxicloud::app::create_app_router;
use oxicloud::test_utils::TestState;

#[tokio::test]
async fn caldav_discovery_redirects_permanently_without_auth() {
    let state = TestState::new_dummy();
    let app = create_app_router(state);

    let request = Request::builder()
        .method("GET")
        .uri("/.well-known/caldav")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "Scenario 1: The CalDAV discovery endpoint must return a 301 Permanent Redirect"
    );

    let location_header = response
        .headers()
        .get("Location")
        .expect("Scenario 1: Response must contain a Location header");

    assert_eq!(
        location_header,
        "/dav/",
        "Security Constraint: The target path must be strictly hardcoded to /dav/"
    );
}

#[tokio::test]
async fn caldav_discovery_prevents_open_redirects_via_query_params() {
    let state = TestState::new_dummy();
    let app = create_app_router(state);

    let request = Request::builder()
        .method("GET")
        .uri("/.well-known/caldav?redirect=https://malicious.example.com")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "Endpoint should still process the request and return a 301"
    );

    let location_header = response
        .headers()
        .get("Location")
        .expect("Response must contain a Location header");

    assert_eq!(
        location_header,
        "/dav/",
        "Security Constraint (Open Redirect): Query parameters MUST NOT influence the destination of the Location header."
    );
}

#[tokio::test]
async fn caldav_discovery_enforces_http_get_method() {
    let state = TestState::new_dummy();
    let app = create_app_router(state);

    let request = Request::builder()
        .method("POST")
        .uri("/.well-known/caldav")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_ne!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "The CalDAV discovery endpoint must reject non-GET HTTP methods"
    );

    assert_eq!(
        response.status(),
        StatusCode::METHOD_NOT_ALLOWED,
        "Expected 405 Method Not Allowed for a POST request on a GET-only route"
    );
}
