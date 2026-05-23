use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use tower::ServiceExt; // Required for `oneshot`

// Note: `crate::app::create_router` reflects the standard integration test setup 
// provided by the Codebase Expert context to inject the mock database pool.

/// Scenario 1 & 2: Standard CalDAV discovery redirect & Handling unauthenticated discovery
#[sqlx::test]
async fn test_caldav_discovery_redirect_unauthenticated(pool: sqlx::PgPool) {
    // 1. Initialize the app with test state
    let app = crate::app::create_router(pool);

    // 2. Build an unauthenticated GET request to the well-known path
    let request = Request::builder()
        .uri("/.well-known/caldav")
        .method("GET")
        .body(Body::empty())
        .expect("Failed to build request");

    // 3. Execute the request directly against the Axum router in memory
    let response = app.oneshot(request).await.expect("Failed to execute request");

    // 4. Assertions (Acceptance Criteria)
    // Must be HTTP 301 Permanent Redirect (to allow indefinite caching by clients)
    assert_eq!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "Expected 301 Permanent Redirect for CalDAV discovery. The router might be blocked by auth middleware."
    );

    // Location header must point exactly to our hardcoded DAV root
    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("Location header must be present in the response");
    
    assert_eq!(
        location.to_str().expect("Location header should be valid ASCII"),
        "/dav/",
        "The Location header must strictly point to the root CalDAV endpoint '/dav/'"
    );
}

/// Security Constraint: Open Redirect Prevention
#[sqlx::test]
async fn test_caldav_discovery_prevents_open_redirect(pool: sqlx::PgPool) {
    // 1. Initialize the app
    let app = crate::app::create_router(pool);

    // 2. Build an unauthenticated request with malicious query parameters or paths
    let request = Request::builder()
        .uri("/.well-known/caldav?redirect=https://evil.example.com/dav/")
        .method("GET")
        .body(Body::empty())
        .expect("Failed to build request");

    // 3. Execute the request
    let response = app.oneshot(request).await.expect("Failed to execute request");

    // 4. Assertions for Security Constraints
    assert_eq!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "Expected 301 Permanent Redirect even with query parameters present"
    );

    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("Location header must be present");
    
    // The redirect MUST remain hardcoded and ignore client input entirely
    assert_eq!(
        location.to_str().expect("Location header should be valid ASCII"),
        "/dav/",
        "SECURITY FAILURE: Open redirect vulnerability detected. The target path must be strictly hardcoded to '/dav/' and must ignore query parameters."
    );
}

/// Defensive QA: Verify HTTP Methods
#[sqlx::test]
async fn test_caldav_discovery_rejects_invalid_methods(pool: sqlx::PgPool) {
    // Ensuring that standard Axum routing correctly rejects unsupported HTTP methods
    let app = crate::app::create_router(pool);

    let request = Request::builder()
        .uri("/.well-known/caldav")
        .method("POST")
        .body(Body::empty())
        .expect("Failed to build request");

    let response = app.oneshot(request).await.expect("Failed to execute request");

    // Standard axum routers return 405 Method Not Allowed (or 404 Not Found if fallback catches it).
    // It should definitely NOT process the request and return a 301.
    assert!(
        response.status() == StatusCode::METHOD_NOT_ALLOWED || response.status() == StatusCode::NOT_FOUND,
        "Expected POST to /.well-known/caldav to be safely rejected (405 or 404)"
    );
}
