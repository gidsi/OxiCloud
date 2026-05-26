use reqwest::Client;
use sqlx::postgres::PgPoolOptions;
use std::fs::{self, File};
use std::io::Write;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;
use uuid::Uuid;

// Test 1: Verify the "Shared Nothing" Architecture & Environment Injection.
// We must prove that the application can boot against a dynamically provided unique database URL,
// and that End-to-End HTTP requests work against the actual application wiring, fulfilling
// the criteria that the orchestrator can inject state safely.
#[tokio::test]
async fn test_application_supports_environment_injection_for_isolated_state() {
    if std::env::var("OXICLOUD_RUN_DB_E2E").ok().as_deref() != Some("1") {
        eprintln!("skipping database E2E test; set OXICLOUD_RUN_DB_E2E=1 to run");
        return;
    }

    let uuid = Uuid::new_v4();
    let db_name = format!("test_iso_{}", uuid.simple());

    // Connect to the test postgres instance to create the new isolated database
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

    let port = 18086 + (uuid.as_u128() % 1000) as u16;

    // Resolve compiled binary path to avoid cargo lock contention in concurrent tests
    let mut exe_path = std::env::current_exe().expect("Failed to get current executable path");
    exe_path.pop();
    if exe_path.ends_with("deps") {
        exe_path.pop();
    }
    exe_path.push("oxicloud");

    let storage_path = format!("./storage_test_{}", uuid.simple());

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

    // Wait for server to boot with retry logic to avoid CI flakiness
    let client = Client::new();
    let mut success = false;

    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if let Ok(res) = client
            .get(format!("http://127.0.0.1:{}/health", port))
            .send()
            .await
        {
            if res.status().is_success() {
                assert!(
                    res.headers().contains_key("content-type"),
                    "Expected content-type header from health endpoint"
                );
                success = true;
                break;
            }
        }
    }

    // Teardown
    spawned.kill().ok();
    let _ = spawned.wait();
    let _ = fs::remove_dir_all(&storage_path);
    sqlx::query(&format!("DROP DATABASE IF EXISTS {}", db_name))
        .execute(&admin_pool)
        .await
        .ok();

    assert!(
        success,
        "Expected successful E2E response from dynamically isolated server instance. Environment Injection failed."
    );
}

// Test 2: Verify the xtask Orchestrator mitigates the CI hanging and Bash monolith risks
// by enforcing timeouts, discovering `.hurl` & `*_test.sh` files, and logging the isolated database creations.
#[tokio::test]
async fn test_xtask_orchestrator_discovers_files_and_enforces_timeouts() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let test_dir = temp_dir.path().join("tests");
    fs::create_dir(&test_dir).expect("Failed to create tests dir");

    let hurl_path = test_dir.join("api_test.hurl");
    let mut hurl_file = File::create(&hurl_path).unwrap();
    writeln!(hurl_file, "GET http://localhost:8086/health\nHTTP 200").unwrap();

    let sh_path = test_dir.join("hanging_test.sh");
    let mut sh_file = File::create(&sh_path).unwrap();
    writeln!(sh_file, "#!/bin/bash\nsleep 60").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&sh_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&sh_path, perms).unwrap();
    }

    let output = Command::new("cargo")
        .args(["run", "-p", "xtask", "--", "test", "--tests-dir"])
        .arg(temp_dir.path())
        .args(["--timeout-secs", "1"])
        .env("CARGO_TARGET_DIR", "target/spawn_target") // Avoid build lock deadlocks in parallel test runs
        .output()
        .expect("Failed to execute xtask runner. Ensure the xtask crate is created.");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The test MUST fail until the tool is implemented, enforcing the acceptance criteria outputs.
    assert!(
        stdout.contains("api_test.hurl") || stderr.contains("api_test.hurl"),
        "The xtask runner must discover .hurl files"
    );

    assert!(
        stdout.contains("hanging_test.sh") || stderr.contains("hanging_test.sh"),
        "The xtask runner must discover *_test.sh files"
    );

    assert!(
        stdout.contains("timeout") || stderr.contains("timeout"),
        "The xtask runner MUST enforce a strict timeout and log it for hanging scripts (e.g., tokio::time::timeout wraps execution)"
    );

    assert!(
        stdout.contains("isolated state") || stdout.contains("database") || stdout.contains("UUID"),
        "The xtask runner must dynamically orchestrate the infrastructure and log the creation of isolated database states"
    );
}
