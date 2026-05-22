use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use oxi_cloud::application::state::AppState;
use oxi_cloud::interfaces::api::router::app_router as build_app_router;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower::ServiceExt;

fn app_router() -> Router {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/oxicloud_test".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy(&database_url)
        .expect("Failed to create lazy test database pool");

    let state = Arc::new(AppState::new(pool));

    build_app_router(state)
}

#[tokio::test]
async fn caldav_discovery_redirects_permanently_without_auth() {
    let app = app_router();

    let request = Request::builder()
        .uri("/.well-known/caldav")
        .method("GET")
        .body(Body::empty())
        .expect("Failed to build request");

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    assert_eq!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "Expected 301 Moved Permanently, got {}",
        response.status()
    );

    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("Missing Location header")
        .to_str()
        .expect("Location header must be valid ASCII");

    assert_eq!(location, "/dav/");
}

#[tokio::test]
async fn caldav_discovery_supports_propfind_method() {
    let app = app_router();

    let request = Request::builder()
        .uri("/.well-known/caldav")
        .method("PROPFIND")
        .body(Body::empty())
        .expect("Failed to build request");

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    assert_eq!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "Expected 301 Moved Permanently for PROPFIND requests"
    );

    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("Missing Location header")
        .to_str()
        .expect("Location header must be valid ASCII");

    assert_eq!(location, "/dav/");
}

#[tokio::test]
async fn caldav_discovery_supports_head_method() {
    let app = app_router();

    let request = Request::builder()
        .uri("/.well-known/caldav")
        .method("HEAD")
        .body(Body::empty())
        .expect("Failed to build request");

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    assert_eq!(
        response.status(),
        StatusCode::MOVED_PERMANENTLY,
        "Expected 301 Moved Permanently for HEAD requests"
    );

    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("Missing Location header")
        .to_str()
        .expect("Location header must be valid ASCII");

    assert_eq!(location, "/dav/");
}

#[tokio::test]
async fn caldav_discovery_ignores_query_parameters_for_security() {
    let app = app_router();

    let request = Request::builder()
        .uri("/.well-known/caldav?redirect=https://malicious.com")
        .method("GET")
        .body(Body::empty())
        .expect("Failed to build request");

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);

    let location = response
        .headers()
        .get(header::LOCATION)
        .expect("Missing Location header")
        .to_str()
        .expect("Location header must be valid ASCII");

    assert_eq!(
        location,
        "/dav/",
        "SECURITY FAILURE: Route is susceptible to Open Redirect or query parameter injection."
    );
}

#[tokio::test]
async fn caldav_discovery_does_not_challenge_for_authentication() {
    let app = app_router();

    let request = Request::builder()
        .uri("/.well-known/caldav")
        .method("GET")
        .body(Body::empty())
        .expect("Failed to build request");

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert!(
        response.headers().get(header::WWW_AUTHENTICATE).is_none(),
        "The /.well-known/caldav discovery endpoint must not issue an authentication challenge"
    );
}

#[tokio::test]
async fn actual_dav_endpoint_enforces_authentication() {
    let app = app_router();

    let request = Request::builder()
        .uri("/dav/")
        .method("GET")
        .body(Body::empty())
        .expect("Failed to build request");

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "Expected 401 Unauthorized on the actual /dav/ endpoint to prove auth is enforced post-redirect"
    );
}

#[tokio::test]
async fn actual_dav_endpoint_returns_basic_auth_challenge() {
    let app = app_router();

    let request = Request::builder()
        .uri("/dav/")
        .method("PROPFIND")
        .body(Body::empty())
        .expect("Failed to build request");

    let response = app
        .oneshot(request)
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let challenge = response
        .headers()
        .get(header::WWW_AUTHENTICATE)
        .expect("Missing WWW-Authenticate header on DAV authentication challenge")
        .to_str()
        .expect("WWW-Authenticate header must be valid ASCII");

    assert_eq!(challenge, r#"Basic realm="OxiCloud", charset="UTF-8""#);
}
