use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;

use oxicloud::{app, config::AppConfig, state::AppState};

#[sqlx::test]
async fn test_well_known_caldav_absolute_redirect_and_host_header_mitigation(pool: PgPool) {
    let config = AppConfig {
        base_url: "https://cloud.test-server.com".to_string(),
        ..Default::default()
    };

    let state = AppState {
        config: Arc::new(config),
        pool,
    };

    let app = app(state);

    let request = Request::builder()
        .uri("/.well-known/caldav")
        .header(header::HOST, "attacker.com")
        .body(Body::empty())
        .expect("Failed to build CalDAV request");

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute CalDAV request on the Axum router");

    assert_eq!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "Apple iOS strictly requires a 301 Moved Permanently redirect for CalDAV discovery. \
         A 302, 307, or 308 might fail silently."
    );

    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("Location header MUST be present in a 301 response")
        .to_str()
        .expect("Location header must be valid ASCII");

    assert_eq!(
        location,
        "https://cloud.test-server.com/dav/",
        "Host Header Injection vulnerability detected or invalid relative path used! \
         The CalDAV redirect MUST construct an absolute URL exactly matching the AppConfig base_url \
         and completely ignore the incoming HTTP Host header."
    );
}

#[sqlx::test]
async fn test_well_known_carddav_absolute_redirect_and_host_header_mitigation(pool: PgPool) {
    let config = AppConfig {
        base_url: "https://cloud.test-server.com".to_string(),
        ..Default::default()
    };

    let state = AppState {
        config: Arc::new(config),
        pool,
    };

    let app = app(state);

    let request = Request::builder()
        .uri("/.well-known/carddav")
        .header(header::HOST, "evil-domain.net")
        .body(Body::empty())
        .expect("Failed to build CardDAV request");

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute CardDAV request on the Axum router");

    assert_eq!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "Apple macOS strictly requires a 301 Moved Permanently redirect for CardDAV discovery."
    );

    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("Location header MUST be present in a 301 response")
        .to_str()
        .expect("Location header must be valid ASCII");

    assert_eq!(
        location,
        "https://cloud.test-server.com/dav/",
        "Host Header Injection vulnerability detected or invalid relative path used! \
         The CardDAV redirect MUST construct an absolute URL exactly matching the AppConfig base_url."
    );
}

#[sqlx::test]
async fn test_well_known_caldav_propfind_redirects_to_absolute_dav_root(pool: PgPool) {
    let config = AppConfig {
        base_url: "https://cloud.test-server.com/".to_string(),
        ..Default::default()
    };

    let state = AppState {
        config: Arc::new(config),
        pool,
    };

    let app = app(state);

    let request = Request::builder()
        .method("PROPFIND")
        .uri("/.well-known/caldav")
        .header(header::HOST, "attacker.com")
        .body(Body::empty())
        .expect("Failed to build CalDAV PROPFIND request");

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute CalDAV PROPFIND request on the Axum router");

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);

    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("Location header MUST be present in a 301 response")
        .to_str()
        .expect("Location header must be valid ASCII");

    assert_eq!(location, "https://cloud.test-server.com/dav/");
}

#[sqlx::test]
async fn test_well_known_carddav_head_redirects_to_absolute_dav_root(pool: PgPool) {
    let config = AppConfig {
        base_url: "https://cloud.test-server.com/".to_string(),
        ..Default::default()
    };

    let state = AppState {
        config: Arc::new(config),
        pool,
    };

    let app = app(state);

    let request = Request::builder()
        .method("HEAD")
        .uri("/.well-known/carddav")
        .header(header::HOST, "evil-domain.net")
        .body(Body::empty())
        .expect("Failed to build CardDAV HEAD request");

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute CardDAV HEAD request on the Axum router");

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);

    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("Location header MUST be present in a 301 response")
        .to_str()
        .expect("Location header must be valid ASCII");

    assert_eq!(location, "https://cloud.test-server.com/dav/");
}
