#![cfg(feature = "integration_tests")]

use axum::{
    Extension, Router,
    body::Body,
    http::{Method, Request, StatusCode},
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

/// Build the real application router for Story 4 CalDAV DELETE integration tests.
///
/// Authentication is injected as the same request extension the production CalDAV handler reads,
/// while token issuance itself remains out of scope for these DELETE semantics.
async fn spawn_app() -> (Router, PgPool, Uuid) {
    let base_database_url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect(
            "TEST_DATABASE_URL or DATABASE_URL must be set for Story 4 CalDAV integration tests",
        );

    let test_database_name = format!("oxicloud_story4_{}", Uuid::new_v4().simple());
    create_test_database(&base_database_url, &test_database_name).await;

    let test_database_url = database_url_for_name(&base_database_url, &test_database_name);
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&test_database_url)
        .await
        .expect("failed to connect to isolated Story 4 test database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("failed to run migrations for Story 4 test database");

    let (user_id, calendar_id) = seed_test_data(&pool).await;
    let storage_dir = tempfile::tempdir().expect("failed to create temporary storage directory");
    let storage_path = StoragePath::from(storage_dir.path().to_path_buf());

    let app_state = AppState::new(
        pool.clone(),
        storage_path,
        "story4-test-jwt-secret".to_string(),
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

    let router = create_router(app_state)
        .layer(Extension(resources))
        .layer(Extension(current_user));

    (router, pool, calendar_id)
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
        .expect("failed to create isolated Story 4 test database");

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

async fn seed_test_data(pool: &PgPool) -> (Uuid, Uuid) {
    let user_id = Uuid::new_v4();
    let calendar_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO auth.users (id, username, email, password_hash, role, active)
        VALUES ($1, 'user', 'user@example.test', 'not-used', 'user'::auth.userrole, TRUE)
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await
    .expect("failed to seed user");

    sqlx::query(
        r##"
        INSERT INTO caldav.calendars (id, slug, name, owner_id, description, color, is_public)
        VALUES ($1, 'default', 'Default', $2, 'Default test calendar', '#1f6feb', FALSE)
        "##,
    )
    .bind(calendar_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("failed to seed calendar");

    sqlx::query(
        r#"
        INSERT INTO caldav.calendar_properties (calendar_id, name, value)
        VALUES
            ($1, '{DAV:}displayname', 'Default'),
            ($1, '{DAV:}resourcetype', 'collection,calendar')
        "#,
    )
    .bind(calendar_id)
    .execute(pool)
    .await
    .expect("failed to seed calendar properties");

    let event_id_1 = Uuid::new_v4();
    let event_id_2 = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO caldav.calendar_events (
            id, calendar_id, summary, start_time, end_time, 
            ical_uid, resource_name, etag, ical_data
        ) VALUES 
        ($1, $3, 'Event 1', '2023-10-25 12:00:00Z', '2023-10-25 13:00:00Z', 'uid1', 'event1.ics', 'etag1', 'data1'),
        ($2, $3, 'Event 2', '2023-10-26 12:00:00Z', '2023-10-26 13:00:00Z', 'uid2', 'event2.ics', 'etag2', 'data2')
        "#
    )
    .bind(event_id_1)
    .bind(event_id_2)
    .bind(calendar_id)
    .execute(pool)
    .await
    .expect("failed to seed events");

    (user_id, calendar_id)
}

#[tokio::test]
async fn test_delete_calendar_collection() {
    let (app, pool, calendar_id) = spawn_app().await;

    // 1. Verify calendar and events exist before deletion
    let calendar_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM caldav.calendars WHERE id = $1")
            .bind(calendar_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(calendar_count, 1, "Calendar must exist initially");

    let events_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM caldav.calendar_events WHERE calendar_id = $1")
            .bind(calendar_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(events_count, 2, "Events must exist initially");

    // 2. Send DELETE request to the calendar collection URL
    let request = Request::builder()
        .method(Method::DELETE)
        .uri("/dav/calendars/user/default/")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // 3. Assert HTTP status code 204 No Content
    assert_eq!(
        response.status(),
        StatusCode::NO_CONTENT,
        "Expected 204 No Content when successfully deleting an entire calendar collection"
    );

    // 4. Verify calendar is explicitly deleted from the database
    let calendar_count_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM caldav.calendars WHERE id = $1")
            .bind(calendar_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        calendar_count_after, 0,
        "Calendar record must be completely gone"
    );

    // 5. Verify nested events are deleted via cascading
    let events_count_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM caldav.calendar_events WHERE calendar_id = $1")
            .bind(calendar_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        events_count_after, 0,
        "All calendar events must be cascadingly deleted"
    );
}
