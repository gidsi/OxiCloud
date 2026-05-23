use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use sqlx::PgPool;
use tower::ServiceExt; // Required for `oneshot` routing

// Assuming standard exports from the main application crate.
// Adjust the `oxicloud` crate import if the internal library name differs.
use oxicloud::{app_router, AppState};

/// SCENARIO 1: Standard CardDAV discovery redirect
/// Verifies that a request to `/.well-known/carddav` yields an HTTP redirect
/// and points to the root CardDAV endpoint.
#[sqlx::test]
async fn test_carddav_discovery_redirects_to_valid_endpoint(pool: PgPool) {
    // Arrange: Initialize state and in-memory router
    let state = AppState { db: pool };
    let app = app_router(state);

    let request = Request::builder()
        .uri("/.well-known/carddav")
        .method("GET")
        .body(Body::empty())
        .expect("Failed to build request");

    // Act: Dispatch the request
    let response = app.oneshot(request).await.expect("Failed to execute request");

    // Assert: Must be a valid redirect status code
    let status = response.status();
    assert!(
        status.is_redirection(),
        "Expected a redirect status code (e.g., 301, 307, 308), but got {}",
        status
    );

    // Assert: Must contain a valid Location header
    let location_header = response
        .headers()
        .get(header::LOCATION)
        .expect("Redirect response must contain a 'Location' header")
        .to_str()
        .expect("Location header must be valid ASCII");

    // Assert: Must target the DAV endpoints explicitly
    assert!(
        location_header == "/dav/" || location_header == "/carddav/",
        "Location header must point to the root CardDAV endpoint. Got: {}",
        location_header
    );

    // Assert: The body should be empty for this redirect
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");
    assert!(
        body_bytes.is_empty(),
        "Expected the redirect response body to be empty"
    );
}

/// SCENARIO 2 & SECURITY CONSTRAINT: Preserving protocol scheme & Proxy Spoofing Prevention
/// Proves the application fulfills the Tech Lead's mitigation strategy by using 
/// an absolute relative path instead of relying on potentially spoofed HTTP headers.
#[sqlx::test]
async fn test_carddav_discovery_ignores_spoofed_forwarded_headers(pool: PgPool) {
    // Arrange
    let state = AppState { db: pool };
    let app = app_router(state);

    // Create a request simulating a malicious actor trying to force a downgrade to HTTP
    // or attempting a host-spoofing attack via proxy headers.
    let request = Request::builder()
        .uri("/.well-known/carddav")
        .method("GET")
        .header("X-Forwarded-Proto", "http")
        .header("X-Forwarded-Host", "malicious-attacker.com")
        .header("Host", "cloud.example.com")
        .body(Body::empty())
        .expect("Failed to build request");

    // Act
    let response = app.oneshot(request).await.expect("Failed to execute request");

    // Assert
    assert!(
        response.status().is_redirection(),
        "Expected redirect status, got {}",
        response.status()
    );

    let location_header = response
        .headers()
        .get(header::LOCATION)
        .expect("Missing Location header")
        .to_str()
        .unwrap();

    // Security Verification: The Location MUST NOT trust the X-Forwarded-Host
    assert!(
        !location_header.contains("malicious-attacker.com"),
        "CRITICAL SECURITY FAILURE: The redirect trusted an unverified X-Forwarded-Host header! Got: {}",
        location_header
    );

    // Security Verification: The Location MUST NOT downgrade to HTTP
    assert!(
        !location_header.starts_with("http://"),
        "CRITICAL SECURITY FAILURE: The redirect downgraded the scheme to http:// based on X-Forwarded-Proto! Got: {}",
        location_header
    );

    // Technical Verification: Best practice absolute relative path (starting with '/')
    assert!(
        location_header.starts_with('/'),
        "To guarantee scheme preservation across all reverse proxies, the Location header must be a relative absolute-path starting with '/'. Got: {}",
        location_header
    );
}
