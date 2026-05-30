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
