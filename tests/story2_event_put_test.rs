#![cfg(feature = "integration_tests")]

use axum::{
    Extension, Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use oxicloud::application::dtos::user_dto::CurrentUser;
use oxicloud::common::di::AppState;
use oxicloud::domain::services::path_service::StoragePath;
use oxicloud::interfaces::http::router::create_router;
use sqlx::{
    ConnectOptions, Executor, PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{str::FromStr, sync::Arc};
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

struct TestResources {
    _storage_dir: TempDir,
    _database_name: String,
}

/// Build the real application router for Story 2 CalDAV PUT integration tests.
///
/// The Story 2 acceptance tests must exercise production Axum routes, services,
/// sqlx repositories, and PostgreSQL-backed persistence. Authentication is
/// injected as the same request extension the production CalDAV handler reads,
/// while token issuance itself remains out of scope for these PUT semantics.
async fn spawn_app() -> Router {
    let base_database_url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect(
            "TEST_DATABASE_URL or DATABASE_URL must be set for Story 2 CalDAV integration tests",
        );

    let test_database_name = format!("oxicloud_story2_{}", Uuid::new_v4().simple());
    create_test_database(&base_database_url, &test_database_name).await;

    let test_database_url = database_url_for_name(&base_database_url, &test_database_name);
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&test_database_url)
        .await
        .expect("failed to connect to isolated Story 2 test database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("failed to run migrations for Story 2 test database");

    let user_id = seed_user_and_default_calendar(&pool).await;
    let storage_dir = tempfile::tempdir().expect("failed to create temporary storage directory");
    let storage_path = StoragePath::from(storage_dir.path().to_path_buf());

    let app_state = AppState::new(
        pool,
        storage_path,
        "story2-test-jwt-secret".to_string(),
        "localhost".to_string(),
    )
    .await;

    let current_user = Arc::new(CurrentUser {
        id: user_id,
        username: "user".to_string(),
        email: "user@example.test".to_string(),
        role: "user".to_string(),
    });

    let resources = Arc::new(TestResources {
        _storage_dir: storage_dir,
        _database_name: test_database_name,
    });

    create_router(app_state)
        .layer(Extension(resources))
        .layer(Extension(current_user))
}

async fn create_test_database(base_database_url: &str, database_name: &str) {
    let mut admin_options = PgConnectOptions::from_str(base_database_url)
        .expect("TEST_DATABASE_URL or DATABASE_URL must be a valid PostgreSQL connection URL");
    admin_options = admin_options.database("postgres");
    admin_options = admin_options.disable_statement_logging();

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(admin_options)
        .await
        .expect("failed to connect to PostgreSQL maintenance database");

    let create_database_sql = format!("CREATE DATABASE {}", quote_identifier(database_name));
    admin_pool
        .execute(create_database_sql.as_str())
        .await
        .expect("failed to create isolated Story 2 test database");

    admin_pool.close().await;
}

fn database_url_for_name(base_database_url: &str, database_name: &str) -> String {
    let mut options = PgConnectOptions::from_str(base_database_url)
        .expect("TEST_DATABASE_URL or DATABASE_URL must be a valid PostgreSQL connection URL");
    options = options.database(database_name);
    options.to_url_lossy().to_string()
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

async fn seed_user_and_default_calendar(pool: &PgPool) -> Uuid {
    let user_id = Uuid::new_v4();
    let calendar_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO auth.users (id, username, email, password_hash, role, active)
        VALUES ($1, 'user', 'user@example.test', 'not-used-in-story2-tests', 'user'::auth.userrole, TRUE)
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await
    .expect("failed to seed Story 2 test user");

    sqlx::query(
        r##"
        INSERT INTO caldav.calendars (id, slug, name, owner_id, description, color, is_public)
        VALUES ($1, 'default', 'Default', $2, 'Default Story 2 test calendar', '#1f6feb', FALSE)
        "##,
    )
    .bind(calendar_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("failed to seed Story 2 default calendar");

    sqlx::query(
        r#"
        INSERT INTO caldav.calendar_properties (calendar_id, name, value)
        VALUES
            ($1, '{DAV:}displayname', 'Default'),
            ($1, '{DAV:}resourcetype', 'collection,calendar'),
            ($1, '{urn:ietf:params:xml:ns:caldav}supported-calendar-component-set', 'VEVENT')
        ON CONFLICT (calendar_id, name) DO UPDATE SET value = EXCLUDED.value
        "#,
    )
    .bind(calendar_id)
    .execute(pool)
    .await
    .expect("failed to seed Story 2 calendar properties");

    user_id
}

// -----------------------------------------------------------------------------
// Test Constants
// -----------------------------------------------------------------------------
const VALID_ICS: &str = "BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//My Calendar E2E Test//EN\r\n\
BEGIN:VEVENT\r\n\
UID:story2-test-event-001\r\n\
DTSTAMP:20231024T120000Z\r\n\
DTSTART:20231025T120000Z\r\n\
DTEND:20231025T130000Z\r\n\
SUMMARY:Original Test Event\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

const VALID_ICS_UPDATED: &str = "BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//My Calendar E2E Test//EN\r\n\
BEGIN:VEVENT\r\n\
UID:story2-test-event-001\r\n\
DTSTAMP:20231025T120000Z\r\n\
DTSTART:20231025T120000Z\r\n\
DTEND:20231025T130000Z\r\n\
SUMMARY:Updated Test Event\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

// -----------------------------------------------------------------------------
// Test Scenarios
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_put_new_event_success() {
    let app = spawn_app().await;

    let request = Request::builder()
        .method(Method::PUT)
        .uri("/dav/calendars/user/default/new_event.ics")
        .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
        .body(Body::from(VALID_ICS))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "Expected 201 Created when a completely new event is successfully PUT"
    );
    assert!(
        response.headers().contains_key(header::ETAG),
        "Expected ETag header in the response for a successfully created event (RFC 4791 requirement)"
    );
}

#[tokio::test]
async fn test_put_new_event_with_if_none_match_success() {
    let app = spawn_app().await;

    let request = Request::builder()
        .method(Method::PUT)
        .uri("/dav/calendars/user/default/new_event_none_match.ics")
        .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
        .header(header::IF_NONE_MATCH, "*")
        .body(Body::from(VALID_ICS))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "Expected 201 Created when using If-None-Match: * on an event that does not exist yet"
    );
    assert!(
        response.headers().contains_key(header::ETAG),
        "Response MUST include an ETag on successful creation"
    );
}

#[tokio::test]
async fn test_put_existing_event_with_if_none_match_fails() {
    let app = spawn_app().await;
    let uri = "/dav/calendars/user/default/conflict_event.ics";

    // 1. Setup: Create the event
    let request1 = Request::builder()
        .method(Method::PUT)
        .uri(uri)
        .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
        .body(Body::from(VALID_ICS))
        .unwrap();

    let response1 = app.clone().oneshot(request1).await.unwrap();
    assert_eq!(
        response1.status(),
        StatusCode::CREATED,
        "Test Setup Failed: Expected 201 Created"
    );

    // 2. Concurrency Test: Attempt to create it again from another 'device' utilizing If-None-Match: *
    let request2 = Request::builder()
        .method(Method::PUT)
        .uri(uri)
        .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
        .header(header::IF_NONE_MATCH, "*")
        .body(Body::from(VALID_ICS_UPDATED))
        .unwrap();

    let response2 = app.oneshot(request2).await.unwrap();

    assert_eq!(
        response2.status(),
        StatusCode::PRECONDITION_FAILED,
        "Expected 412 Precondition Failed when an event already exists and If-None-Match: * is provided to prevent overwrites"
    );
}

#[tokio::test]
async fn test_put_update_event_with_valid_etag_success() {
    let app = spawn_app().await;
    let uri = "/dav/calendars/user/default/update_event.ics";

    // 1. Setup: Create the event and capture the server-generated ETag
    let request1 = Request::builder()
        .method(Method::PUT)
        .uri(uri)
        .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
        .body(Body::from(VALID_ICS))
        .unwrap();

    let response1 = app.clone().oneshot(request1).await.unwrap();
    assert_eq!(
        response1.status(),
        StatusCode::CREATED,
        "Test Setup Failed: Expected 201 Created"
    );

    let original_etag = response1
        .headers()
        .get(header::ETAG)
        .expect("Server MUST return an ETag on event creation for concurrency handling")
        .to_str()
        .unwrap()
        .to_string();

    // 2. Concurrency Test: Update the event providing the valid ETag to guarantee safe modification
    let request2 = Request::builder()
        .method(Method::PUT)
        .uri(uri)
        .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
        .header(header::IF_MATCH, &original_etag)
        .body(Body::from(VALID_ICS_UPDATED))
        .unwrap();

    let response2 = app.oneshot(request2).await.unwrap();

    let status = response2.status();
    // RFC 4791 supports returning either 200 OK or 204 No Content for a successful PUT update
    assert!(
        status == StatusCode::OK || status == StatusCode::NO_CONTENT,
        "Expected 200 OK or 204 No Content for successful update, got {}",
        status
    );

    let new_etag = response2
        .headers()
        .get(header::ETAG)
        .expect("Server MUST return a new ETag header in the response after updating")
        .to_str()
        .unwrap();

    assert_ne!(
        original_etag, new_etag,
        "The returned ETag after update MUST differ from the original ETag"
    );
}

#[tokio::test]
async fn test_put_update_event_with_invalid_etag_fails() {
    let app = spawn_app().await;
    let uri = "/dav/calendars/user/default/bad_etag_event.ics";

    // 1. Setup: Create the event
    let request1 = Request::builder()
        .method(Method::PUT)
        .uri(uri)
        .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
        .body(Body::from(VALID_ICS))
        .unwrap();

    let response1 = app.clone().oneshot(request1).await.unwrap();
    assert_eq!(
        response1.status(),
        StatusCode::CREATED,
        "Test Setup Failed: Expected 201 Created"
    );

    // 2. Concurrency Test: Try to update it with an outdated/incorrect ETag (simulating concurrent modifications)
    let request2 = Request::builder()
        .method(Method::PUT)
        .uri(uri)
        .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
        .header(header::IF_MATCH, "\"stale-or-bogus-etag-12345\"")
        .body(Body::from(VALID_ICS_UPDATED))
        .unwrap();

    let response2 = app.oneshot(request2).await.unwrap();

    assert_eq!(
        response2.status(),
        StatusCode::PRECONDITION_FAILED,
        "Expected 412 Precondition Failed when providing a mismatched ETag in If-Match header"
    );
}

#[tokio::test]
async fn test_put_event_empty_body_fails() {
    let app = spawn_app().await;

    // QA Edge Case: What if the client inputs nothing?
    let request = Request::builder()
        .method(Method::PUT)
        .uri("/dav/calendars/user/default/empty_event.ics")
        .header(header::CONTENT_TYPE, "text/calendar; charset=utf-8")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "Expected 400 Bad Request for an empty PUT body. Server must validate input!"
    );
}
