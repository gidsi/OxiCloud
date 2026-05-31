#![cfg(feature = "integration_tests")]

use axum::{
    Extension, Router,
    body::Body,
    http::{Request, StatusCode},
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
use tower::ServiceExt;
use uuid::Uuid;

const VALID_ICS: &str = "BEGIN:VCALENDAR\r\n\
VERSION:2.0\r\n\
PRODID:-//My Calendar E2E Test//EN\r\n\
BEGIN:VEVENT\r\n\
UID:story2-readonly-test-event-001\r\n\
DTSTAMP:20231024T120000Z\r\n\
DTSTART:20231025T120000Z\r\n\
DTEND:20231025T130000Z\r\n\
SUMMARY:Read Only Test Event\r\n\
END:VEVENT\r\n\
END:VCALENDAR\r\n";

struct TestApp {
    router: Router,
    // Keep tempdir alive for the duration of the TestApp
    _storage_dir: tempfile::TempDir,
}

async fn build_app_for_user(user_id: Uuid, username: &str, pool: PgPool) -> TestApp {
    let storage_dir = tempfile::tempdir().expect("failed to create temporary storage directory");
    let storage_path = StoragePath::from(storage_dir.path().to_path_buf());

    let app_state = AppState::new(
        pool,
        storage_path,
        "story2-readonly-jwt-secret".to_string(),
        "localhost".to_string(),
    )
    .await;

    let current_user = Arc::new(CurrentUser {
        id: user_id,
        username: username.to_string(),
        email: format!("{}@example.test", username),
        role: "user".to_string(),
    });

    let router = create_router(app_state).layer(Extension(current_user));

    TestApp {
        router,
        _storage_dir: storage_dir,
    }
}

async fn setup_db_pool() -> PgPool {
    let base_database_url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect(
            "TEST_DATABASE_URL or DATABASE_URL must be set for Story 2 CalDAV integration tests",
        );

    let test_database_name = format!("oxicloud_story2_readonly_{}", Uuid::new_v4().simple());
    
    // Connect to global template to create isolated DB
    let mut admin_options = PgConnectOptions::from_str(&base_database_url)
        .expect("TEST_DATABASE_URL or DATABASE_URL must be a valid PostgreSQL connection URL");
    admin_options = admin_options.database("postgres");
    admin_options = admin_options.disable_statement_logging();

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(admin_options)
        .await
        .expect("failed to connect to PostgreSQL maintenance database");

    let create_database_sql = format!("CREATE DATABASE \"{}\"", test_database_name.replace('"', "\"\""));
    admin_pool
        .execute(create_database_sql.as_str())
        .await
        .expect("failed to create isolated Story 2 test database");

    admin_pool.close().await;

    // Connect to newly created isolated DB
    let mut options = PgConnectOptions::from_str(&base_database_url).unwrap();
    options = options.database(&test_database_name);
    let test_database_url = options.to_url_lossy().to_string();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&test_database_url)
        .await
        .expect("failed to connect to isolated Story 2 test database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("failed to run migrations for Story 2 test database");

    pool
}

async fn seed_test_data(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    let user_a_id = Uuid::new_v4();
    let user_b_id = Uuid::new_v4();
    let user_c_id = Uuid::new_v4();
    let calendar_id = Uuid::new_v4();

    // 1. Insert Test Users
    for (id, username) in [(user_a_id, "usera"), (user_b_id, "userb"), (user_c_id, "userc")] {
        sqlx::query(
            "INSERT INTO auth.users (id, username, email, password_hash, role, active) VALUES ($1, $2, $3, 'hash', 'user'::auth.userrole, TRUE)"
        )
        .bind(id)
        .bind(username)
        .bind(format!("{}@example.test", username))
        .execute(pool)
        .await
        .unwrap();
    }

    // 2. Insert Calendar for User A
    sqlx::query(
        "INSERT INTO caldav.calendars (id, slug, name, owner_id, description, color, is_public) VALUES ($1, 'default', 'Default', $2, 'Test', '#000000', FALSE)"
    )
    .bind(calendar_id)
    .bind(user_a_id)
    .execute(pool)
    .await
    .unwrap();

    // 3. Populate Calendar Properties (as per architecture layout)
    sqlx::query(
        r#"
        INSERT INTO caldav.calendar_properties (calendar_id, name, value)
        VALUES
            ($1, '{DAV:}displayname', 'Default'),
            ($1, '{DAV:}resourcetype', 'collection,calendar'),
            ($1, '{urn:ietf:params:xml:ns:caldav}supported-calendar-component-set', 'VEVENT')
        ON CONFLICT (calendar_id, name) DO UPDATE SET value = EXCLUDED.value
        "#
    )
    .bind(calendar_id)
    .execute(pool)
    .await
    .unwrap();

    // 4. Assign User B read-only access
    sqlx::query(
        "INSERT INTO caldav.calendar_shares (calendar_id, user_id, access_level) VALUES ($1, $2, 'read')"
    )
    .bind(calendar_id)
    .bind(user_b_id)
    .execute(pool)
    .await
    .unwrap();

    (user_a_id, user_b_id, user_c_id, calendar_id)
}

#[tokio::test]
async fn test_read_only_privilege_enforcement_in_propfind() {
    let pool = setup_db_pool().await;
    let (_user_a, user_b, _user_c, _cal_id) = seed_test_data(&pool).await;
    
    // Boot up app injecting User B
    let app_b = build_app_for_user(user_b, "userb", pool).await;

    let req = Request::builder()
        .method("PROPFIND")
        .uri("/dav/calendars/usera/default/")
        .header("Depth", "0")
        .body(Body::from(
            r#"<?xml version="1.0" encoding="utf-8" ?>
<D:propfind xmlns:D="DAV:">
    <D:prop>
        <D:current-user-privilege-set/>
    </D:prop>
</D:propfind>"#,
        ))
        .unwrap();

    let response = app_b.router.oneshot(req).await.unwrap();
    
    assert_eq!(
        response.status(),
        StatusCode::MULTI_STATUS,
        "PROPFIND should succeed with 207 Multi-Status for a user with read access"
    );

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let xml = String::from_utf8(body_bytes.to_vec()).unwrap().to_lowercase();
    
    // AC: XML MUST include <D:read/>
    assert!(
        xml.contains("<d:read/>") || xml.contains("<read xmlns=\"dav:\"/>") || xml.contains("<read/>"),
        "Expected PROPFIND response to grant the read privilege"
    );
    
    // AC: XML MUST NOT include <D:write/> in current-user-privilege-set
    assert!(
        !xml.contains("<d:write/>") 
        && !xml.contains("<write xmlns=\"dav:\"/>") 
        && !xml.contains("<write/>") 
        && !xml.contains("write-content") 
        && !xml.contains("write-properties"),
        "Expected PROPFIND response to strictly NOT contain any write privileges for a read-only user"
    );
}

#[tokio::test]
async fn test_server_side_rejection_of_unauthorized_edits_403() {
    let pool = setup_db_pool().await;
    let (_user_a, user_b, _user_c, _cal_id) = seed_test_data(&pool).await;
    
    // Boot up app injecting User B
    let app_b = build_app_for_user(user_b, "userb", pool).await;

    let scenarios = vec![
        ("PUT", "/dav/calendars/usera/default/test_event.ics", Body::from(VALID_ICS)),
        ("DELETE", "/dav/calendars/usera/default/test_event.ics", Body::empty()),
        ("PROPPATCH", "/dav/calendars/usera/default/", Body::empty()),
        ("MKCALENDAR", "/dav/calendars/usera/default/", Body::empty()),
    ];

    for (method, uri, body) in scenarios {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri);
            
        if method == "PUT" {
            builder = builder.header("Content-Type", "text/calendar; charset=utf-8");
        } else if method == "PROPPATCH" {
            builder = builder.header("Content-Type", "text/xml; charset=utf-8");
        }

        let req = builder.body(body).unwrap();
        let response = app_b.router.clone().oneshot(req).await.unwrap();
        
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "Expected 403 Forbidden for state-mutating {} attempt as read-only user",
            method
        );
    }
}

#[tokio::test]
async fn test_idor_prevention_for_unauthorized_users_404() {
    let pool = setup_db_pool().await;
    let (_user_a, _user_b, user_c, _cal_id) = seed_test_data(&pool).await;
    
    // Boot up app injecting User C (Zero Access)
    let app_c = build_app_for_user(user_c, "userc", pool).await;

    let scenarios = vec![
        ("PROPFIND", "/dav/calendars/usera/default/"),
        ("PUT", "/dav/calendars/usera/default/new_event.ics"),
        ("DELETE", "/dav/calendars/usera/default/old_event.ics"),
        ("PROPPATCH", "/dav/calendars/usera/default/"),
    ];

    for (method, uri) in scenarios {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri);
            
        if method == "PUT" {
            builder = builder.header("Content-Type", "text/calendar; charset=utf-8");
        } else if method == "PROPFIND" || method == "PROPPATCH" {
            builder = builder.header("Content-Type", "text/xml; charset=utf-8");
            builder = builder.header("Depth", "0");
        }

        let req = builder.body(Body::empty()).unwrap();
        let response = app_c.router.clone().oneshot(req).await.unwrap();
        
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Expected 404 Not Found for {} on unshared calendar to prevent IDOR resource enumeration",
            method
        );
    }
}
