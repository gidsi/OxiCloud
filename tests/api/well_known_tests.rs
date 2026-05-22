use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use std::sync::Arc;
use tower::ServiceExt;
use oxicloud::{app::create_router, config::AppConfig, state::AppState};

/// Scenario 1: iOS Calendar native account creation
/// Verifies that Apple iOS clients receive a strict, absolute 301 redirect
/// from `/.well-known/caldav` and mitigates Host header injection.
#[sqlx::test]
async fn test_apple_caldav_absolute_redirect(pool: sqlx::PgPool) {
    // 1. Arrange: Initialize State with a strict HTTPS base_url
    let config = AppConfig {
        base_url: "https://cloud.apple-strict.com".to_string(),
        ..AppConfig::default() 
    };
    
    let state = Arc::new(AppState {
        db: pool,
        config,
    });
    
    let app = create_router(state);

    // 2. Act: Send a GET request to the .well-known caldav endpoint
    let request = Request::builder()
        .method("GET")
        .uri("/.well-known/caldav")
        .header("Host", "attacker-controlled-host.com") // Ensure Host header is ignored
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // 3. Assert: Verify 301 Permanent Redirect and exact absolute URL
    assert_eq!(
        response.status(), 
        StatusCode::MOVED_PERMANENTLY,
        "Expected HTTP 301 Moved Permanently for Apple strict compliance"
    );
    
    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("Location header missing")
        .to_str()
        .unwrap();

    // Apple strict validation: MUST be an absolute URL derived from AppConfig, NOT the Host header
    assert_eq!(
        location, 
        "https://cloud.apple-strict.com/dav/",
        "Redirect URL must be absolute and use the configured base_url to prevent silent Apple client failures"
    );
}

/// Scenario 2: macOS Contacts native account creation
/// Verifies that Apple macOS clients receive a strict, absolute 301 redirect
/// from `/.well-known/carddav` and mitigates Host header injection.
#[sqlx::test]
async fn test_apple_carddav_absolute_redirect(pool: sqlx::PgPool) {
    // 1. Arrange: Initialize State with a strict HTTPS base_url
    let config = AppConfig {
        base_url: "https://cloud.apple-strict.com".to_string(),
        ..AppConfig::default() 
    };
    
    let state = Arc::new(AppState {
        db: pool,
        config,
    });
    
    let app = create_router(state);

    // 2. Act: Send a GET request to the .well-known carddav endpoint
    let request = Request::builder()
        .method("GET")
        .uri("/.well-known/carddav")
        .header("Host", "attacker-controlled-host.com") // Ensure Host header is ignored
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // 3. Assert: Verify 301 Permanent Redirect and exact absolute URL
    assert_eq!(
        response.status(), 
        StatusCode::MOVED_PERMANENTLY,
        "Expected HTTP 301 Moved Permanently for Apple strict compliance"
    );
    
    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("Location header missing")
        .to_str()
        .unwrap();

    // Apple strict validation: MUST be an absolute URL derived from AppConfig, NOT the Host header
    assert_eq!(
        location, 
        "https://cloud.apple-strict.com/dav/",
        "Redirect URL must be absolute and use the configured base_url to prevent silent Apple client failures"
    );
}
