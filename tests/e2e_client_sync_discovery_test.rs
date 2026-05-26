#![cfg(feature = "integration_tests")]

use std::fs;
use std::process::Stdio;
use tokio::process::Command;

/// This test verifies the Acceptance Criteria for the Dynamic Auto-Discovery story.
/// It dynamically creates a real E2E client sync script (like Thunderbird/DAVx5)
/// inside `tests/e2e/*_test.sh`, runs the `xtask` test runner, and asserts that
/// the script is automatically discovered, executed against a booted real application,
/// and that the script correctly validates HTTP status codes and bodies.
#[cfg(unix)]
#[tokio::test]
async fn test_dynamic_auto_discovery_for_bash_e2e_scripts() {
    let test_workspace = tempfile::TempDir::new().expect("Failed to create test workspace");
    let tests_dir = test_workspace.path().join("tests");
    let e2e_dir = tests_dir.join("e2e");
    fs::create_dir_all(&e2e_dir).expect("Failed to create e2e directory");

    let bash_test_path = e2e_dir.join("client_sync_test.sh");

    fs::write(
        &bash_test_path,
        r#"#!/bin/bash
set -e

if [ -z "$base_url" ]; then
    echo "FAIL: base_url not injected by runner"
    exit 1
fi

echo "Simulating Thunderbird/DAVx5 Client Sync E2E test..."

RESPONSE=$(curl -s -w "\n%{http_code}" "$base_url/ready")
BODY=$(echo "$RESPONSE" | sed '$d')
STATUS=$(echo "$RESPONSE" | tail -n1)

if [ "$STATUS" != "200" ]; then
    echo "FAIL: expected HTTP status code 200 from /ready, got $STATUS"
    exit 1
fi

if ! echo "$BODY" | grep -q '"status":"ok"'; then
    echo "FAIL: response body did not contain status=ok: $BODY"
    exit 1
fi

if ! echo "$BODY" | grep -q '"db":"ok"'; then
    echo "FAIL: response body did not contain db=ok: $BODY"
    exit 1
fi

echo "CLIENT_SYNC_VERIFIED_SUCCESS"
exit 0
"#,
    )
    .unwrap();

    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(&bash_test_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&bash_test_path, perms).unwrap();

    let base_db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/oxicloud".to_string());

    let mut current_dir = std::env::current_dir().unwrap();
    if current_dir.ends_with("tests") {
        current_dir.pop();
    }

    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("test")
        .arg("--test-dir")
        .arg(tests_dir.to_str().unwrap())
        .arg("--timeout-secs")
        .arg("30")
        .env("BASE_DB_URL", &base_db_url)
        .env("DATABASE_URL", &base_db_url)
        .current_dir(&current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .expect("Failed to execute xtask runner");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    if combined.contains("because PostgreSQL is unavailable") {
        return;
    }

    assert!(
        combined.contains("client_sync_test.sh"),
        "The xtask runner must automatically discover and execute bash scripts placed in tests/e2e/ without manual CI registration. Output:\n{}",
        combined
    );

    assert!(
        combined.contains("CLIENT_SYNC_VERIFIED_SUCCESS"),
        "The E2E sync regression check failed. The test runner did not properly provision the E2E script or route requests. Output:\n{}",
        combined
    );

    assert!(
        output.status.success(),
        "Unified xtask test runner failed unexpectedly. Output:\n{}",
        combined
    );
}

/// Direct Axum router verification for the public DAV discovery route. This uses
/// the production route handler with `app.oneshot(request)` and asserts status,
/// `Location`, and body semantics without standing up a TCP listener.
#[tokio::test]
async fn test_well_known_caldav_redirect_app_oneshot_integration() {
    use axum::{
        Router,
        body::{self, Body},
        http::{Request, StatusCode, header},
        routing::get,
    };
    use oxicloud::interfaces::api::handlers::caldav_handler::handle_well_known_caldav;
    use tower::ServiceExt;

    let app = Router::new().route("/.well-known/caldav", get(handle_well_known_caldav));

    let request = Request::builder()
        .uri("/.well-known/caldav")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::PERMANENT_REDIRECT,
        "Expected permanent redirect for CalDAV auto-discovery"
    );

    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "/caldav/",
        "Expected CalDAV discovery to redirect to the existing OxiCloud CalDAV root"
    );

    let body = body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();

    assert!(
        body.is_empty(),
        "Expected redirect response body to be empty, got: {}",
        String::from_utf8_lossy(&body)
    );
}
