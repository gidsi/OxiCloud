use reqwest::{Client, Method, StatusCode};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::fs;
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn test_mkcalendar_creates_new_calendar_e2e() {
    if std::env::var("OXICLOUD_RUN_DB_E2E").ok().as_deref() != Some("1") {
        eprintln!("skipping database E2E test; set OXICLOUD_RUN_DB_E2E=1 to run");
        return;
    }

    let uuid = Uuid::new_v4();
    let db_name = format!("test_caldav_{}", uuid.simple());

    let base_db_url = std::env::var("OXICLOUD_DB_CONNECTION_STRING")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| {
            "postgres://oxicloud_test:oxicloud_test@localhost:5433/oxicloud_test".to_string()
        });

    let admin_pool = PgPoolOptions::new()
        .connect(&base_db_url)
        .await
        .expect("Failed to connect to admin db");

    sqlx::query(&format!("CREATE DATABASE {}", db_name))
        .execute(&admin_pool)
        .await
        .expect("Failed to create isolated database");

    let base_url_parsed = base_db_url
        .rsplit_once('/')
        .map(|(base, _)| base)
        .unwrap_or(&base_db_url);
    let isolated_db_url = format!("{}/{}", base_url_parsed, db_name);

    let port = 19000 + (uuid.as_u128() % 1000) as u16;

    let mut exe_path = std::env::current_exe().expect("Failed to get current executable path");
    exe_path.pop();
    if exe_path.ends_with("deps") {
        exe_path.pop();
    }
    exe_path.push("oxicloud");

    let storage_path = format!("./storage_caldav_test_{}", uuid.simple());

    let mut server_process = if exe_path.exists() {
        let mut cmd = Command::new(&exe_path);
        cmd.env("DATABASE_URL", &isolated_db_url)
            .env("OXICLOUD_DB_CONNECTION_STRING", &isolated_db_url)
            .env("OXICLOUD_SERVER_PORT", port.to_string())
            .env("OXICLOUD_STORAGE_PATH", &storage_path);
        cmd
    } else {
        let mut cmd = Command::new("cargo");
        cmd.args(["run", "--bin", "oxicloud"])
            .env("CARGO_TARGET_DIR", "target/spawn_target")
            .env("DATABASE_URL", &isolated_db_url)
            .env("OXICLOUD_DB_CONNECTION_STRING", &isolated_db_url)
            .env("OXICLOUD_SERVER_PORT", port.to_string())
            .env("OXICLOUD_STORAGE_PATH", &storage_path);
        cmd
    };

    let mut spawned = server_process
        .spawn()
        .expect("Failed to spawn server process");

    let client = Client::new();
    let base_url = format!("http://127.0.0.1:{}", port);

    let mut is_ready = false;
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if let Ok(res) = client.get(format!("{}/health", base_url)).send().await {
            if res.status().is_success() {
                is_ready = true;
                break;
            }
        }
    }

    assert!(is_ready, "Server failed to become ready");

    // 1. Setup the first admin user
    let setup_res = client
        .post(format!("{}/api/auth/setup", base_url))
        .json(&json!({
            "username": "admin",
            "email": "admin@test.local",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();

    // Ignore error if already setup via global DB state
    assert!(setup_res.status().is_success() || setup_res.status() == StatusCode::FORBIDDEN);

    // 2. Login
    let login_res = client
        .post(format!("{}/api/auth/login", base_url))
        .json(&json!({
            "username": "admin",
            "password": "password123"
        }))
        .send()
        .await
        .expect("Login request failed");

    assert_eq!(login_res.status(), StatusCode::OK);
    let login_body: serde_json::Value = login_res.json().await.unwrap();
    let token = login_body["access_token"]
        .as_str()
        .expect("Missing access token");

    // 3. Perform MKCALENDAR request using custom HTTP method
    let mkcalendar_xml = r#"<?xml version="1.0" encoding="utf-8" ?>
    <C:mkcalendar xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
      <D:set>
        <D:prop>
          <D:displayname>Test Personal</D:displayname>
          <C:calendar-description>Personal schedule test</C:calendar-description>
          <C:calendar-color>#ff00ff</C:calendar-color>
        </D:prop>
      </D:set>
    </C:mkcalendar>"#;

    let req = client
        .request(
            Method::from_bytes(b"MKCALENDAR").unwrap(),
            format!("{}/caldav/test-personal", base_url),
        )
        .bearer_auth(token)
        .header("Content-Type", "application/xml")
        .body(mkcalendar_xml)
        .build()
        .unwrap();

    let mkcalendar_res = client
        .execute(req)
        .await
        .expect("Failed to execute MKCALENDAR");

    // This is expected to FAIL if the MKCALENDAR endpoints, HTTP body reading or properties
    // have not been completely wired up or return an invalid HTTP Status.
    assert_eq!(
        mkcalendar_res.status(),
        StatusCode::CREATED,
        "MKCALENDAR should return 201 Created but returned {}",
        mkcalendar_res.status()
    );

    // 4. Validate with PROPFIND
    let propfind_xml = r#"<?xml version="1.0" encoding="utf-8" ?>
    <D:propfind xmlns:D="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">
      <D:prop>
        <D:displayname/>
        <C:calendar-color/>
      </D:prop>
    </D:propfind>"#;

    let propfind_req = client
        .request(
            Method::from_bytes(b"PROPFIND").unwrap(),
            format!("{}/caldav/test-personal", base_url),
        )
        .bearer_auth(token)
        .header("Depth", "0")
        .header("Content-Type", "application/xml")
        .body(propfind_xml)
        .build()
        .unwrap();

    let propfind_res = client
        .execute(propfind_req)
        .await
        .expect("Failed to execute PROPFIND");

    assert_eq!(propfind_res.status(), StatusCode::MULTI_STATUS);

    let propfind_body = propfind_res.text().await.unwrap();
    assert!(
        propfind_body.contains("Test Personal"),
        "Calendar displayname was not saved"
    );
    assert!(
        propfind_body.contains("#ff00ff"),
        "Calendar color was not saved"
    );

    // Teardown
    spawned.kill().ok();
    let _ = spawned.wait();
    let _ = fs::remove_dir_all(&storage_path);
    sqlx::query(&format!("DROP DATABASE IF EXISTS {}", db_name))
        .execute(&admin_pool)
        .await
        .ok();
}
