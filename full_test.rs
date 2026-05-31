use oxicloud::xtask_runner::{TestKind, discover_tests};
use std::fs;
use std::process::Stdio;
use tokio::process::Command;

#[test]
fn test_xtask_discovery_filters_supported_test_files() {
    let test_workspace = tempfile::TempDir::new().expect("Failed to create test workspace");
    let tests_dir = test_workspace.path().join("tests");
    let common_dir = tests_dir.join("common");
    let nested_dir = tests_dir.join("nested");
    fs::create_dir_all(&common_dir).expect("Failed to create common tests directory");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested tests directory");

    fs::write(
        tests_dir.join("example.hurl"),
        "GET {{base_url}}/ready\nHTTP 200\n",
    )
    .unwrap();
    fs::write(
        tests_dir.join("setup.hurl"),
        "GET {{base_url}}/ready\nHTTP 200\n",
    )
    .unwrap();
    fs::write(tests_dir.join("foo_test.sh"), "#!/bin/bash\nexit 0\n").unwrap();
    fs::write(common_dir.join("helper_test.sh"), "#!/bin/bash\nexit 1\n").unwrap();
    fs::write(nested_dir.join("bar_test.sh"), "#!/bin/bash\nexit 0\n").unwrap();
    fs::write(tests_dir.join("run.sh"), "#!/bin/bash\nexit 1\n").unwrap();

    let discovered = discover_tests(&tests_dir).expect("test discovery should succeed");
    let discovered_names = discovered
        .iter()
        .map(|test| {
            (
                test.path.file_name().unwrap().to_string_lossy().to_string(),
                test.kind,
            )
        })
        .collect::<Vec<_>>();

    assert!(
        discovered_names.contains(&("example.hurl".to_string(), TestKind::Hurl)),
        "expected ordinary .hurl files to be discovered"
    );
    assert!(
        discovered_names.contains(&("foo_test.sh".to_string(), TestKind::Bash)),
        "expected top-level *_test.sh files to be discovered"
    );
    assert!(
        discovered_names.contains(&("bar_test.sh".to_string(), TestKind::Bash)),
        "expected nested *_test.sh files to be discovered"
    );
    assert!(
        !discovered_names
            .iter()
            .any(|(name, _)| name == "setup.hurl"),
        "setup.hurl must only be used as a companion setup file, not as a standalone test"
    );
    assert!(
        !discovered_names
            .iter()
            .any(|(name, _)| name == "helper_test.sh"),
        "tests/common helper scripts must not be discovered as standalone tests"
    );
    assert!(
        !discovered_names.iter().any(|(name, _)| name == "run.sh"),
        "legacy run.sh scripts must not be discovered as standalone tests"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_xtask_skips_hurl_when_binary_is_unavailable_but_runs_bash() {
    let test_workspace = tempfile::TempDir::new().expect("Failed to create test workspace");
    let tests_dir = test_workspace.path().join("tests");
    let tools_dir = test_workspace.path().join("tools");
    fs::create_dir_all(&tests_dir).expect("Failed to create tests directory");
    fs::create_dir_all(&tools_dir).expect("Failed to create tools directory");

    fs::write(
        tests_dir.join("example.hurl"),
        "GET {{base_url}}/ready\nHTTP 200\n",
    )
    .unwrap();

    let bash_test_path = tests_dir.join("ok_test.sh");
    fs::write(
        &bash_test_path,
        r#"#!/bin/bash
set -e
if [ -z "$DATABASE_URL" ]; then
    echo "FAIL: DATABASE_URL was not injected"
    exit 1
fi
echo "BASH_TEST_RAN"
"#,
    )
    .unwrap();

    use std::os::unix::fs::{PermissionsExt, symlink};

    let mut perms = fs::metadata(&bash_test_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&bash_test_path, perms).unwrap();

    let bash_target = ["/bin/bash", "/usr/bin/bash"]
        .iter()
        .find(|path| std::path::Path::new(path).is_file())
        .expect("test requires a system bash binary");
    symlink(bash_target, tools_dir.join("bash")).expect("Failed to create isolated bash symlink");

    let mut current_dir = std::env::current_dir().unwrap();
    if current_dir.ends_with("tests") {
        current_dir.pop();
    }

    let base_db_url = "postgres://postgres:postgres@localhost/oxicloud";
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("test")
        .arg("--test-dir")
        .arg(tests_dir.to_str().unwrap())
        .arg("--timeout-secs")
        .arg("5")
        .arg("--skip-db")
        .arg("--skip-server")
        .env("PATH", tools_dir.to_str().unwrap())
        .env("BASE_DB_URL", base_db_url)
        .env("DATABASE_URL", base_db_url)
        .current_dir(&current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .expect("Failed to execute xtask runner");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{}\n{}", stdout, stderr);

    assert!(
        output.status.success(),
        "Runner should succeed when unavailable Hurl tests are skipped and Bash tests pass\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        combined_output
            .contains("Skipping 1 discovered Hurl test(s) because `hurl` is not installed"),
        "Runner did not warn that unavailable Hurl tests were skipped. Output: {combined_output}"
    );
    assert!(
        combined_output.contains("ok_test.sh"),
        "Runner did not execute the discovered Bash test. Output: {combined_output}"
    );
    assert!(
        combined_output.contains("BASH_TEST_RAN"),
        "Runner did not capture output from the executed Bash test. Output: {combined_output}"
    );
    assert!(
        combined_output.contains("Unified xtask test summary")
            && combined_output.contains("Passed: 1")
            && combined_output.contains("Failed: 0"),
        "Runner did not emit the expected unified summary for executed Bash tests. Output: {combined_output}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_xtask_default_skips_real_e2e_without_docker_or_postgres() {
    let test_workspace = tempfile::TempDir::new().expect("Failed to create test workspace");
    let tests_dir = test_workspace.path().join("tests");
    let tools_dir = test_workspace.path().join("tools");
    fs::create_dir_all(&tests_dir).expect("Failed to create tests directory");
    fs::create_dir_all(&tools_dir).expect("Failed to create tools directory");

    let bash_test_path = tests_dir.join("real_e2e_test.sh");
    fs::write(
        &bash_test_path,
        r#"#!/bin/bash
echo "FAIL: real E2E test should have been skipped"
exit 1
"#,
    )
    .unwrap();

    use std::os::unix::fs::{PermissionsExt, symlink};

    let mut perms = fs::metadata(&bash_test_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&bash_test_path, perms).unwrap();

    let bash_target = ["/bin/bash", "/usr/bin/bash"]
        .iter()
        .find(|path| std::path::Path::new(path).is_file())
        .expect("test requires a system bash binary");
    symlink(bash_target, tools_dir.join("bash")).expect("Failed to create isolated bash symlink");

    let mut current_dir = std::env::current_dir().unwrap();
    if current_dir.ends_with("tests") {
        current_dir.pop();
    }

    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("test")
        .arg("--test-dir")
        .arg(tests_dir.to_str().unwrap())
        .arg("--timeout-secs")
        .arg("1")
        .env("PATH", tools_dir.to_str().unwrap())
        .env(
            "BASE_DB_URL",
            "postgres://postgres:postgres@127.0.0.1:1/oxicloud",
        )
        .env_remove("DATABASE_URL")
        .env_remove("OXICLOUD_XTASK_MANAGE_DB")
        .current_dir(&current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .expect("Failed to execute xtask runner");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{}\n{}", stdout, stderr);

    assert!(
        output.status.success(),
        "Runner should skip real E2E tests instead of trying Docker by default\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        combined_output.contains("Skipping 1 real E2E test(s) because PostgreSQL is unavailable and managed PostgreSQL was not requested"),
        "Runner did not emit the expected PostgreSQL skip message. Output: {combined_output}"
    );
    assert!(
        combined_output
            .contains("Provide a reachable DATABASE_URL/BASE_DB_URL or run with --manage-db"),
        "Runner did not tell the user how to enable managed PostgreSQL. Output: {combined_output}"
    );
    assert!(
        !combined_output.contains("FAIL: real E2E test should have been skipped"),
        "Runner executed a real E2E test that should have been skipped. Output: {combined_output}"
    );
    assert!(
        !combined_output.contains("docker: command not found")
            && !combined_output.contains("spawn-db.sh"),
        "Runner appears to have called the managed DB script without opt-in. Output: {combined_output}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_xtask_manage_db_fails_clearly_when_docker_unavailable() {
    let test_workspace = tempfile::TempDir::new().expect("Failed to create test workspace");
    let tests_dir = test_workspace.path().join("tests");
    let tools_dir = test_workspace.path().join("tools");
    fs::create_dir_all(&tests_dir).expect("Failed to create tests directory");
    fs::create_dir_all(&tools_dir).expect("Failed to create tools directory");

    let bash_test_path = tests_dir.join("real_e2e_test.sh");
    fs::write(&bash_test_path, "#!/bin/bash\nexit 0\n").unwrap();

    use std::os::unix::fs::{PermissionsExt, symlink};

    let mut perms = fs::metadata(&bash_test_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&bash_test_path, perms).unwrap();

    let bash_target = ["/bin/bash", "/usr/bin/bash"]
        .iter()
        .find(|path| std::path::Path::new(path).is_file())
        .expect("test requires a system bash binary");
    symlink(bash_target, tools_dir.join("bash")).expect("Failed to create isolated bash symlink");

    let mut current_dir = std::env::current_dir().unwrap();
    if current_dir.ends_with("tests") {
        current_dir.pop();
    }

    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("test")
        .arg("--test-dir")
        .arg(tests_dir.to_str().unwrap())
        .arg("--timeout-secs")
        .arg("1")
        .arg("--manage-db")
        .env("PATH", tools_dir.to_str().unwrap())
        .env(
            "BASE_DB_URL",
            "postgres://postgres:postgres@127.0.0.1:1/oxicloud",
        )
        .env_remove("DATABASE_URL")
        .env_remove("OXICLOUD_XTASK_MANAGE_DB")
        .current_dir(&current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .expect("Failed to execute xtask runner");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{}\n{}", stdout, stderr);

    assert!(
        !output.status.success(),
        "Runner should fail when --manage-db is requested but Docker Compose is unavailable"
    );
    assert!(
        combined_output
            .contains("Managed PostgreSQL was requested, but Docker Compose is unavailable"),
        "Runner did not emit the clear Docker Compose preflight failure. Output: {combined_output}"
    );
    assert!(
        combined_output.contains(
            "Install Docker/Compose, provide a reachable DATABASE_URL, or run without --manage-db"
        ),
        "Runner did not include remediation guidance. Output: {combined_output}"
    );
    assert!(
        !combined_output.contains("docker: command not found")
            && !combined_output.contains("managed PostgreSQL test database startup failed"),
        "Runner should fail before calling spawn-db.sh when Docker Compose is unavailable. Output: {combined_output}"
    );
}

#[tokio::test]
async fn test_xtask_discovery_and_timeout_without_external_services() {
    let test_workspace = tempfile::TempDir::new().expect("Failed to create test workspace");
    let tests_dir = test_workspace.path().join("tests");
    fs::create_dir_all(&tests_dir).expect("Failed to create tests directory");

    let bash_isolation_path = tests_dir.join("isolation_test.sh");
    fs::write(&bash_isolation_path, r#"#!/bin/bash
set -e
if [ -z "$DATABASE_URL" ]; then
    echo "FAIL: DATABASE_URL not injected by runner"
    exit 1
fi
if [ "$DATABASE_URL" = "$BASE_DB_URL" ]; then
    echo "FAIL: DATABASE_URL was not uniquely generated for this test execution"
    exit 1
fi
case "$DATABASE_URL" in
    */oxit_isolation_test_sh_*)
        echo "SUCCESS: Isolation verified"
        exit 0
        ;;
    *)
        echo "FAIL: DATABASE_URL did not use the expected isolated database naming scheme: $DATABASE_URL"
        exit 1
        ;;
esac
"#).unwrap();

    let bash_timeout_path = tests_dir.join("hang_test.sh");
    fs::write(
        &bash_timeout_path,
        r#"#!/bin/bash
echo "Sleeping to trigger runner timeout..."
sleep 60
echo "FAIL: Should have been killed by runner"
exit 1
"#,
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [&bash_isolation_path, &bash_timeout_path] {
            let mut perms = fs::metadata(path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms).unwrap();
        }
    }

    let mut current_dir = std::env::current_dir().unwrap();
    if current_dir.ends_with("tests") {
        current_dir.pop();
    }

    let base_db_url = "postgres://postgres:postgres@localhost/oxicloud";
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("test")
        .arg("--test-dir")
        .arg(tests_dir.to_str().unwrap())
        .arg("--timeout-secs")
        .arg("1")
        .arg("--skip-db")
        .arg("--skip-server")
        .env("BASE_DB_URL", base_db_url)
        .env("DATABASE_URL", base_db_url)
        .current_dir(&current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .expect("Failed to execute xtask runner");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{}\n{}", stdout, stderr);

    assert!(
        !output.status.success(),
        "Runner must return a non-zero exit code when a test times out or fails"
    );
    assert!(
        combined_output.contains("isolation_test.sh"),
        "Runner did not auto-discover the *_test.sh isolation test"
    );
    assert!(
        combined_output.contains("hang_test.sh"),
        "Runner did not auto-discover the *_test.sh timeout test"
    );
    assert!(
        combined_output.contains("SUCCESS: Isolation verified"),
        "Runner did not inject a dynamically generated isolated PostgreSQL DB URL per test execution"
    );

    let lower_output = combined_output.to_lowercase();
    assert!(
        lower_output.contains("timeout") || lower_output.contains("killed"),
        "Runner did not enforce a strict timeout on hang_test.sh. Output: {combined_output}"
    );
    assert!(
        !combined_output.contains("FAIL: Should have been killed by runner"),
        "Runner allowed a hanging bash script to run past its designated execution timeframe"
    );
    assert!(
        combined_output.contains("Unified xtask test summary")
            && combined_output.contains("Passed")
            && combined_output.contains("Failed"),
        "Runner did not emit a unified pass/fail aggregated summary table"
    );
}

#[tokio::test]
async fn test_xtask_real_application_e2e_is_opt_in() {
    if std::env::var_os("OXICLOUD_RUN_REAL_E2E").is_none() {
        eprintln!(
            "skipping real application E2E xtask test; set OXICLOUD_RUN_REAL_E2E=1 to run it"
        );
        return;
    }

    let base_db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/oxicloud".to_string());

    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("test")
        .arg("--test-dir")
        .arg("tests")
        .arg("--timeout-secs")
        .arg("10")
        .env("BASE_DB_URL", &base_db_url)
        .env("DATABASE_URL", &base_db_url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .expect("Failed to execute opt-in xtask runner");

    assert!(
        output.status.success(),
        "opt-in real application E2E xtask run failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[cfg(unix)]
#[tokio::test]
async fn test_xtask_caldav_compliance_returns_zero_on_suite_failure_and_calculates_score() {
    let test_workspace = tempfile::TempDir::new().expect("Failed to create test workspace");
    let tools_dir = test_workspace.path().join("tools");
    let output_dir = test_workspace.path().join("caldav_output");
    fs::create_dir_all(&tools_dir).expect("Failed to create tools directory");
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    let mock_script = format!(
        r#"#!/bin/bash
echo "Simulating CalDAVTester suite run..."
cat << 'XML' > {}/serverinfo.xml
<?xml version="1.0" encoding="utf-8"?>
<results>
  <test><name>Test 1</name><result>0</result></test>
  <test><name>Test 2</name><result>1</result></test>
</results>
XML
echo "Simulating suite failure..."
exit 1
"#,
        output_dir.display()
    );

    for bin_name in &["docker", "python", "python3", "caldavtester"] {
        let mock_path = tools_dir.join(bin_name);
        fs::write(&mock_path, &mock_script).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&mock_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&mock_path, perms).unwrap();
    }

    let mut current_dir = std::env::current_dir().unwrap();
    if current_dir.ends_with("tests") {
        current_dir.pop();
    }

    let base_db_url = "postgres://postgres:postgres@localhost/oxicloud";
    let pr_comment_path = test_workspace.path().join("pr_comment.md");

    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("caldav-compliance")
        .arg("--suite-output-dir")
        .arg(output_dir.to_str().unwrap())
        .arg("--markdown-out")
        .arg(pr_comment_path.to_str().unwrap())
        .env("PATH", format!("{}:{}", tools_dir.to_str().unwrap(), std::env::var("PATH").unwrap_or_default()))
        .env("TEST_DATABASE_URL", base_db_url)
        .current_dir(&current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .expect("Failed to execute xtask runner");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{}\n{}", stdout, stderr);

    assert!(
        output.status.success(),
        "xtask must return exit 0 even if the CalDAVTester suite fails, to prevent blocking CI.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    assert!(
        combined_output.contains("Compliance Score"),
        "Runner did not emit compliance score to logs. Output: {combined_output}"
    );

    assert!(pr_comment_path.exists(), "Markdown PR comment file was not generated");
    let markdown_content = fs::read_to_string(pr_comment_path).unwrap();

    assert!(
        markdown_content.contains("50%"),
        "Markdown should calculate 50% pass rate. Content: {markdown_content}"
    );
    assert!(
        markdown_content.contains("Compliance Score"),
        "Markdown should include 'Compliance Score'. Content: {markdown_content}"
    );

    assert!(
        !markdown_content.contains("postgres://") && !markdown_content.contains("TEST_DATABASE_URL"),
        "Markdown leaked sensitive environment variables. Content: {markdown_content}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_caldav_compliance_starts_axum_server_on_ephemeral_db() {
    let test_workspace = tempfile::TempDir::new().expect("Failed to create test workspace");
    let tools_dir = test_workspace.path().join("tools");
    let output_dir = test_workspace.path().join("caldav_output");
    fs::create_dir_all(&tools_dir).expect("Failed to create tools directory");
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    let mock_script = format!(
        r#"#!/bin/bash
if [ -z "$BASE_URL" ]; then
    echo "FAIL: BASE_URL not set for CalDAV suite"
    exit 1
fi

curl -s --fail "$BASE_URL/ready" || exit 1

if [ "$DATABASE_URL" = "$TEST_DATABASE_URL" ]; then
    echo "FAIL: DB was not ephemeral"
    exit 1
fi

cat << 'XML' > {}/serverinfo.xml
<results><test><result>0</result></test></results>
XML
exit 0
"#,
        output_dir.display()
    );

    for bin_name in &["docker", "python", "python3", "caldavtester"] {
        let mock_path = tools_dir.join(bin_name);
        fs::write(&mock_path, &mock_script).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&mock_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&mock_path, perms).unwrap();
    }

    let curl_target = ["/bin/curl", "/usr/bin/curl"]
        .iter()
        .find(|path| std::path::Path::new(path).is_file())
        .expect("test requires a system curl binary");
    std::os::unix::fs::symlink(curl_target, tools_dir.join("curl")).expect("Failed to create isolated curl symlink");

    let mut current_dir = std::env::current_dir().unwrap();
    if current_dir.ends_with("tests") {
        current_dir.pop();
    }

    let base_db_url = "postgres://postgres:postgres@localhost/oxicloud";
    let pr_comment_path = test_workspace.path().join("pr_comment.md");
    
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("caldav-compliance")
        .arg("--suite-output-dir")
        .arg(output_dir.to_str().unwrap())
        .arg("--markdown-out")
        .arg(pr_comment_path.to_str().unwrap())
        .env("PATH", format!("{}:{}", tools_dir.to_str().unwrap(), std::env::var("PATH").unwrap_or_default()))
        .env("TEST_DATABASE_URL", base_db_url)
        .current_dir(&current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .expect("Failed to execute xtask runner");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{}\n{}", stdout, stderr);

    assert!(
        output.status.success(),
        "xtask should successfully start the Axum server and run the mock suite.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !combined_output.contains("FAIL:"),
        "Mock suite detected failure in ephemeral server initialization. Output: {combined_output}"
    );
}
