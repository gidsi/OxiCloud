use glob::glob;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

const DEFAULT_BASE_DATABASE_URL: &str =
    "postgres://oxicloud_test:oxicloud_test@localhost:5433/oxicloud_test";
const DB_PREFLIGHT_TIMEOUT: Duration = Duration::from_secs(3);
const DB_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const DB_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);
const SERVER_READY_TIMEOUT: Duration = Duration::from_secs(30);
use tempfile::TempDir;
use tokio::process::Command;
use tokio::time;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestKind {
    Hurl,
    Bash,
}

#[derive(Debug, Clone)]
pub struct DiscoveredTest {
    pub path: PathBuf,
    pub kind: TestKind,
}

#[derive(Debug, Clone)]
struct IsolatedDatabase {
    admin_url: String,
    database_url: String,
    database_name: String,
}

#[derive(Debug)]
struct TestContext {
    database_url: String,
    database: Option<IsolatedDatabase>,
    storage_dir: TempDir,
    server: Option<RunningServer>,
    base_url: String,
}

#[derive(Debug)]
struct RunningServer {
    child: tokio::process::Child,
}

#[derive(Debug)]
struct TestResult {
    name: String,
    kind: TestKind,
    passed: bool,
    timed_out: bool,
    duration: Duration,
    stdout: String,
    stderr: String,
    detail: String,
}

#[derive(Debug, Clone)]
struct TestOptions {
    test_dir: PathBuf,
    timeout: Duration,
    skip_hurl: bool,
    skip_bash: bool,
    skip_db: bool,
    skip_server: bool,
    manage_db: bool,
}

#[derive(Debug)]
enum DatabasePreflight {
    Available,
    Unavailable(String),
}

pub async fn run_from_env() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");

    match command {
        "test" => run_tests(&args[1..]).await,
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => Err(format!(
            "unknown xtask command: {other}\n\n{}",
            usage_text()
        )),
    }
}

async fn run_tests(args: &[String]) -> Result<(), String> {
    let options = parse_test_options(args)?;
    let mut tests = discover_tests(&options.test_dir)?
        .into_iter()
        .filter(|test| match test.kind {
            TestKind::Hurl => !options.skip_hurl,
            TestKind::Bash => !options.skip_bash,
        })
        .collect::<Vec<_>>();

    if tests.iter().any(|test| test.kind == TestKind::Hurl) && !command_exists("hurl").await {
        let skipped = tests
            .iter()
            .filter(|test| test.kind == TestKind::Hurl)
            .count();
        println!(
            "Skipping {skipped} discovered Hurl test(s) because `hurl` is not installed. Install hurl to run them."
        );
        tests.retain(|test| test.kind != TestKind::Hurl);
    }

    if tests.is_empty() {
        println!(
            "No runnable .hurl or *_test.sh tests discovered under {}",
            options.test_dir.display()
        );
        return Ok(());
    }

    if let Some(skip_reason) = preflight(&tests, &options).await? {
        let skipped = tests.len();
        println!(
            "Skipping {skipped} real E2E test(s) because PostgreSQL is unavailable and managed PostgreSQL was not requested. {skip_reason}"
        );
        println!(
            "Provide a reachable DATABASE_URL/BASE_DB_URL or run with --manage-db to start a managed PostgreSQL test database."
        );
        return Ok(());
    }

    println!(
        "Discovered {} tests under {}",
        tests.len(),
        options.test_dir.display()
    );
    for test in &tests {
        println!(" - [{:?}] {}", test.kind, test.path.display());
    }

    let mut results = Vec::with_capacity(tests.len());
    for test in tests {
        let result = run_one_test(&test, &options).await;
        print_test_output(&result);
        results.push(result);
    }

    print_summary(&results);

    if results.iter().all(|result| result.passed) {
        Ok(())
    } else {
        Err("one or more xtask tests failed".to_string())
    }
}

fn parse_test_options(args: &[String]) -> Result<TestOptions, String> {
    let mut options = TestOptions {
        test_dir: PathBuf::from("tests"),
        timeout: Duration::from_secs(30),
        skip_hurl: false,
        skip_bash: false,
        skip_db: false,
        skip_server: false,
        manage_db: env_flag_enabled("OXICLOUD_XTASK_MANAGE_DB"),
    };

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--test-dir" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--test-dir requires a path".to_string())?;
                options.test_dir = PathBuf::from(value);
                i += 2;
            }
            "--timeout-secs" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| "--timeout-secs requires a positive integer".to_string())?;
                let timeout_secs = value
                    .parse::<u64>()
                    .map_err(|_| format!("invalid --timeout-secs value: {value}"))?;
                if timeout_secs == 0 {
                    return Err("--timeout-secs must be greater than zero".to_string());
                }
                options.timeout = Duration::from_secs(timeout_secs);
                i += 2;
            }
            "--skip-hurl" => {
                options.skip_hurl = true;
                i += 1;
            }
            "--skip-bash" => {
                options.skip_bash = true;
                i += 1;
            }
            "--skip-db" => {
                options.skip_db = true;
                i += 1;
            }
            "--skip-server" => {
                options.skip_server = true;
                i += 1;
            }
            "--manage-db" => {
                options.manage_db = true;
                i += 1;
            }
            "--help" | "-h" => {
                print_test_usage();
                std::process::exit(0);
            }
            other => {
                return Err(format!(
                    "unknown test argument: {other}\n\n{}",
                    test_usage_text()
                ));
            }
        }
    }

    Ok(options)
}

pub fn discover_tests(test_dir: &Path) -> Result<Vec<DiscoveredTest>, String> {
    let mut tests = Vec::new();
    let hurl_pattern = format!("{}/**/*.hurl", test_dir.display());
    let bash_pattern = format!("{}/**/*_test.sh", test_dir.display());

    for entry in glob(&hurl_pattern).map_err(|e| format!("invalid hurl glob pattern: {e}"))? {
        let path = entry.map_err(|e| format!("failed to read hurl glob entry: {e}"))?;
        if path.is_file() && should_run_path(&path) && !is_setup_hurl(&path) {
            tests.push(DiscoveredTest {
                path,
                kind: TestKind::Hurl,
            });
        }
    }

    for entry in glob(&bash_pattern).map_err(|e| format!("invalid bash glob pattern: {e}"))? {
        let path = entry.map_err(|e| format!("failed to read bash glob entry: {e}"))?;
        if path.is_file() && should_run_path(&path) {
            tests.push(DiscoveredTest {
                path,
                kind: TestKind::Bash,
            });
        }
    }

    tests.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(tests)
}

fn should_run_path(path: &Path) -> bool {
    if path.file_name().and_then(OsStr::to_str) == Some("run.sh") {
        return false;
    }

    let mut previous_component_was_tests = false;
    for component in path.components() {
        let Some(component) = component.as_os_str().to_str() else {
            previous_component_was_tests = false;
            continue;
        };

        if component == "node_modules" || (previous_component_was_tests && component == "common") {
            return false;
        }

        previous_component_was_tests = component == "tests";
    }

    true
}

fn is_setup_hurl(path: &Path) -> bool {
    path.file_name().and_then(OsStr::to_str) == Some("setup.hurl")
}

async fn preflight(
    tests: &[DiscoveredTest],
    options: &TestOptions,
) -> Result<Option<String>, String> {
    if !options.skip_db {
        match ensure_test_database_available(options.manage_db).await? {
            DatabasePreflight::Available => {}
            DatabasePreflight::Unavailable(reason) => return Ok(Some(reason)),
        }
    }

    if tests.iter().any(|test| test.kind == TestKind::Bash) && !command_exists("bash").await {
        return Err(
            "Cannot run discovered Bash tests because `bash` is not installed. Install bash or run with --skip-bash."
                .to_string(),
        );
    }

    if !options.skip_server {
        let binary = oxicloud_binary_path();
        if !binary.is_file() && !command_exists("cargo").await {
            return Err(
                "Cannot start OxiCloud server: target binary is missing and `cargo` is not installed."
                    .to_string(),
            );
        }
    }

    Ok(None)
}

async fn ensure_test_database_available(manage_db: bool) -> Result<DatabasePreflight, String> {
    let base_url = base_database_url();
    let admin_url = replace_database_name_preserving_query(&base_url, "postgres");

    match check_postgres_connection(&admin_url, DB_PREFLIGHT_TIMEOUT).await {
        Ok(()) => return Ok(DatabasePreflight::Available),
        Err(first_error) if !manage_db => {
            return Ok(DatabasePreflight::Unavailable(format!(
                "PostgreSQL test database is not reachable at {admin_url}: {first_error}."
            )));
        }
        Err(first_error) => {
            println!(
                "PostgreSQL test database is not reachable at {admin_url}: {first_error}. Starting managed test database..."
            );
        }
    }

    if !docker_compose_available().await {
        return Err(
            "Managed PostgreSQL was requested, but Docker Compose is unavailable. Install Docker/Compose, provide a reachable DATABASE_URL, or run without --manage-db to skip external E2E tests."
                .to_string(),
        );
    }

    run_spawn_db_script().await?;
    check_postgres_connection(&admin_url, DB_PREFLIGHT_TIMEOUT)
        .await
        .map(|()| DatabasePreflight::Available)
        .map_err(|error| {
            format!(
                "PostgreSQL test database did not become reachable at {admin_url} after running tests/common/spawn-db.sh: {error}"
            )
        })
}

async fn check_postgres_connection(admin_url: &str, timeout: Duration) -> Result<(), String> {
    let pool = time::timeout(
        timeout,
        PgPoolOptions::new()
            .max_connections(1)
            .min_connections(0)
            .connect(admin_url),
    )
    .await
    .map_err(|_| {
        format!(
            "timeout after {} seconds during PostgreSQL preflight connection",
            timeout.as_secs()
        )
    })?
    .map_err(|e| format!("failed PostgreSQL preflight connection: {e}"))?;
    pool.close().await;
    Ok(())
}

async fn run_spawn_db_script() -> Result<(), String> {
    let script = PathBuf::from("tests/common/spawn-db.sh");
    if !script.is_file() {
        return Err(format!(
            "Cannot start managed PostgreSQL test database: {} was not found",
            script.display()
        ));
    }

    let output = time::timeout(
        DB_STARTUP_TIMEOUT,
        Command::new("bash")
            .arg(&script)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| {
        format!(
            "timeout after {} seconds while starting managed PostgreSQL test database with {}",
            DB_STARTUP_TIMEOUT.as_secs(),
            script.display()
        )
    })?
    .map_err(|e| {
        format!(
            "failed to start managed PostgreSQL test database with {}: {e}",
            script.display()
        )
    })?;

    if output.status.success() {
        return Ok(());
    }

    Err(format!(
        "managed PostgreSQL test database startup failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

async fn docker_compose_available() -> bool {
    Command::new("docker")
        .arg("compose")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false)
}

async fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|status| status.success() || status.code().is_some())
        .unwrap_or(false)
}

async fn run_one_test(test: &DiscoveredTest, options: &TestOptions) -> TestResult {
    let started = Instant::now();
    let name = test
        .path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("<unknown>")
        .to_string();

    let mut context = match create_test_context(&name, options).await {
        Ok(context) => context,
        Err(error) => {
            return TestResult {
                name,
                kind: test.kind,
                passed: false,
                timed_out: false,
                duration: started.elapsed(),
                stdout: String::new(),
                stderr: String::new(),
                detail: error,
            };
        }
    };

    let mut result = match test.kind {
        TestKind::Hurl => run_hurl_test(test, &context, options.timeout).await,
        TestKind::Bash => run_bash_test(test, &context, options.timeout).await,
    };

    result.duration = started.elapsed();

    if let Some(server) = context.server.as_mut() {
        if let Err(error) = stop_child_process(&mut server.child).await {
            result.passed = false;
            result.detail = format!("{}; failed to stop OxiCloud server: {error}", result.detail);
        }
    }

    if let Some(database) = context.database.as_ref() {
        if let Err(error) = cleanup_isolated_database(database).await {
            result.passed = false;
            result.detail = format!(
                "{}; failed to drop isolated PostgreSQL database {}: {error}",
                result.detail, database.database_name
            );
        }
    }

    result
}

async fn create_test_context(
    test_name: &str,
    options: &TestOptions,
) -> Result<TestContext, String> {
    let storage_dir =
        tempfile::tempdir().map_err(|e| format!("failed to create temp storage dir: {e}"))?;

    let (database_url, database) = if options.skip_db {
        (isolated_database_url_for(test_name), None)
    } else {
        let database = time::timeout(DB_OPERATION_TIMEOUT, create_isolated_database(test_name))
            .await
            .map_err(|_| {
                format!(
                    "timeout after {} seconds while creating isolated PostgreSQL database",
                    DB_OPERATION_TIMEOUT.as_secs()
                )
            })??;
        (database.database_url.clone(), Some(database))
    };

    let (server, base_url) = if options.skip_server {
        (
            None,
            env::var("BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:0".to_string()),
        )
    } else {
        let server = start_server(&database_url, storage_dir.path(), SERVER_READY_TIMEOUT).await?;
        let base_url = server.base_url.clone();
        (Some(server.server), base_url)
    };

    Ok(TestContext {
        database_url,
        database,
        storage_dir,
        server,
        base_url,
    })
}

#[derive(Debug)]
struct StartedServer {
    server: RunningServer,
    base_url: String,
}

async fn start_server(
    database_url: &str,
    storage_path: &Path,
    timeout: Duration,
) -> Result<StartedServer, String> {
    let port = allocate_port()?;
    let base_url = format!("http://127.0.0.1:{port}");
    let mut command = server_command();

    command
        .env("DATABASE_URL", database_url)
        .env("OXICLOUD_DB_CONNECTION_STRING", database_url)
        .env("OXICLOUD_STORAGE_PATH", storage_path)
        .env("OXICLOUD_SERVER_HOST", "127.0.0.1")
        .env("OXICLOUD_SERVER_PORT", port.to_string())
        .env("OXICLOUD_ENABLE_AUTH", "true")
        .env("OXICLOUD_JWT_SECRET", "0123456789abcdef0123456789abcdef")
        .env(
            "RUST_LOG",
            env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    make_new_process_group(&mut command);

    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to start OxiCloud server: {e}"))?;

    if let Err(error) = wait_for_ready(&base_url, timeout).await {
        let _ = stop_child_process(&mut child).await;
        return Err(error);
    }

    Ok(StartedServer {
        server: RunningServer { child },
        base_url,
    })
}

fn server_command() -> Command {
    let binary = oxicloud_binary_path();
    if binary.is_file() {
        Command::new(binary)
    } else {
        let mut command = Command::new("cargo");
        command.arg("run").arg("--bin").arg("oxicloud").arg("--");
        command
    }
}

fn oxicloud_binary_path() -> PathBuf {
    let exe_suffix = env::consts::EXE_SUFFIX;
    PathBuf::from("target")
        .join("debug")
        .join(format!("oxicloud{exe_suffix}"))
}

async fn wait_for_ready(base_url: &str, timeout: Duration) -> Result<(), String> {
    let client = reqwest::Client::new();
    let ready_url = format!("{base_url}/ready");
    let deadline = Instant::now() + timeout;

    loop {
        match client.get(&ready_url).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            _ if Instant::now() >= deadline => {
                return Err(format!(
                    "timeout waiting for OxiCloud server readiness at {ready_url}"
                ));
            }
            _ => time::sleep(Duration::from_millis(250)).await,
        }
    }
}

fn allocate_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("failed to allocate local TCP port: {e}"))?;
    listener
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|e| format!("failed to read allocated local TCP port: {e}"))
}

async fn run_hurl_test(
    test: &DiscoveredTest,
    context: &TestContext,
    timeout: Duration,
) -> TestResult {
    let name = test_name(test);
    let mut command = Command::new("hurl");
    command
        .arg("--test")
        .arg("--file-root")
        .arg("tests")
        .arg("--variable")
        .arg(format!("base_url={}", context.base_url))
        .arg("--variable")
        .arg("username=admin")
        .arg("--variable")
        .arg("email=admin@example.com")
        .arg("--variable")
        .arg("password=TestPassword1!");

    if let Some(setup) = setup_hurl_for(&test.path) {
        command.arg(setup);
    }
    command.arg(&test.path);

    apply_test_env(&mut command, context);
    command_result(name, TestKind::Hurl, command, timeout).await
}

async fn run_bash_test(
    test: &DiscoveredTest,
    context: &TestContext,
    timeout: Duration,
) -> TestResult {
    let name = test_name(test);
    let host_port = context
        .base_url
        .strip_prefix("http://")
        .or_else(|| context.base_url.strip_prefix("https://"))
        .unwrap_or(&context.base_url)
        .to_string();
    let mut command = Command::new("bash");
    command
        .arg(&test.path)
        .arg(host_port)
        .arg("admin")
        .arg("TestPassword1!");
    apply_test_env(&mut command, context);
    command_result(name, TestKind::Bash, command, timeout).await
}

fn setup_hurl_for(test_path: &Path) -> Option<PathBuf> {
    let setup = test_path.parent()?.join("setup.hurl");
    setup.is_file().then_some(setup)
}

fn apply_test_env(command: &mut Command, context: &TestContext) {
    command
        .env("DATABASE_URL", &context.database_url)
        .env("OXICLOUD_DB_CONNECTION_STRING", &context.database_url)
        .env("OXICLOUD_STORAGE_PATH", context.storage_dir.path())
        .env("BASE_URL", &context.base_url)
        .env("base_url", &context.base_url)
        .env("username", "admin")
        .env("email", "admin@example.com")
        .env("password", "TestPassword1!")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
}

async fn command_result(
    name: String,
    kind: TestKind,
    mut command: Command,
    timeout: Duration,
) -> TestResult {
    let started = Instant::now();
    let mut stdout_file = tempfile::tempfile().expect("failed to create stdout temp file");
    let mut stderr_file = tempfile::tempfile().expect("failed to create stderr temp file");

    command
        .stdout(Stdio::from(
            stdout_file
                .try_clone()
                .expect("failed to clone stdout temp file"),
        ))
        .stderr(Stdio::from(
            stderr_file
                .try_clone()
                .expect("failed to clone stderr temp file"),
        ));

    make_new_process_group(&mut command);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return TestResult {
                name,
                kind,
                passed: false,
                timed_out: false,
                duration: started.elapsed(),
                stdout: String::new(),
                stderr: String::new(),
                detail: format!("failed to spawn test process: {error}"),
            };
        }
    };

    let wait_result = time::timeout(timeout, child.wait()).await;
    let (passed, timed_out, detail) = match wait_result {
        Ok(Ok(status)) => (status.success(), false, format!("exit status: {status}")),
        Ok(Err(error)) => (
            false,
            false,
            format!("failed while waiting for test process: {error}"),
        ),
        Err(_) => {
            let _ = kill_process_group(&mut child).await;
            (
                false,
                true,
                format!(
                    "timeout after {} seconds; process group killed",
                    timeout.as_secs()
                ),
            )
        }
    };

    let stdout = read_temp_file(&mut stdout_file);
    let stderr = read_temp_file(&mut stderr_file);

    TestResult {
        name,
        kind,
        passed,
        timed_out,
        duration: started.elapsed(),
        stdout,
        stderr,
        detail,
    }
}

fn read_temp_file(file: &mut File) -> String {
    let mut output = String::new();
    let _ = file.seek(SeekFrom::Start(0));
    let _ = file.read_to_string(&mut output);
    output
}

async fn create_isolated_database(test_name: &str) -> Result<IsolatedDatabase, String> {
    let base_url = base_database_url();
    let database_name = isolated_database_name_for(test_name);
    let database_url = replace_database_name_preserving_query(&base_url, &database_name);
    let admin_url = replace_database_name_preserving_query(&base_url, "postgres");

    validate_database_identifier(&database_name)?;

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .min_connections(0)
        .connect(&admin_url)
        .await
        .map_err(|e| format!("failed to connect to PostgreSQL admin database: {e}"))?;

    sqlx::query(&format!(
        "CREATE DATABASE {}",
        quote_identifier(&database_name)?
    ))
    .execute(&admin_pool)
    .await
    .map_err(|e| format!("CREATE DATABASE {database_name} failed: {e}"))?;

    admin_pool.close().await;

    let isolated_database = IsolatedDatabase {
        admin_url,
        database_url,
        database_name,
    };

    let test_pool = match PgPoolOptions::new()
        .max_connections(1)
        .min_connections(0)
        .connect(&isolated_database.database_url)
        .await
    {
        Ok(pool) => pool,
        Err(error) => {
            let _ = cleanup_isolated_database(&isolated_database).await;
            return Err(format!(
                "failed to connect to isolated database {}: {error}",
                isolated_database.database_name
            ));
        }
    };

    if let Err(error) = sqlx::migrate!().run(&test_pool).await {
        test_pool.close().await;
        let _ = cleanup_isolated_database(&isolated_database).await;
        return Err(format!(
            "failed to migrate isolated database {}: {error}",
            isolated_database.database_name
        ));
    }

    test_pool.close().await;

    Ok(isolated_database)
}

async fn cleanup_isolated_database(database: &IsolatedDatabase) -> Result<(), String> {
    validate_database_identifier(&database.database_name)?;

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .min_connections(0)
        .connect(&database.admin_url)
        .await
        .map_err(|e| format!("failed to connect to PostgreSQL admin database: {e}"))?;

    sqlx::query(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1 AND pid <> pg_backend_pid()",
    )
    .bind(&database.database_name)
    .execute(&admin_pool)
    .await
    .map_err(|e| format!("failed to terminate isolated database connections: {e}"))?;

    sqlx::query(&format!(
        "DROP DATABASE IF EXISTS {}",
        quote_identifier(&database.database_name)?
    ))
    .execute(&admin_pool)
    .await
    .map_err(|e| format!("DROP DATABASE {} failed: {e}", database.database_name))?;

    admin_pool.close().await;
    Ok(())
}

fn base_database_url() -> String {
    env::var("BASE_DB_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .unwrap_or_else(|_| DEFAULT_BASE_DATABASE_URL.to_string())
}

fn env_flag_enabled(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.as_str(),
                "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
            )
        })
        .unwrap_or(false)
}

fn isolated_database_url_for(test_name: &str) -> String {
    replace_database_name_preserving_query(
        &base_database_url(),
        &isolated_database_name_for(test_name),
    )
}

fn isolated_database_name_for(test_name: &str) -> String {
    format!(
        "oxit_{}_{}",
        sanitize_identifier(test_name),
        Uuid::new_v4().simple()
    )
}

fn replace_database_name_preserving_query(url: &str, database_name: &str) -> String {
    if let Some((prefix, query)) = url.split_once('?') {
        format!("{}?{}", replace_database_name(prefix, database_name), query)
    } else {
        replace_database_name(url, database_name)
    }
}

fn replace_database_name(url_without_query: &str, database_name: &str) -> String {
    match url_without_query.rsplit_once('/') {
        Some((prefix, _)) => format!("{prefix}/{database_name}"),
        None => format!("{url_without_query}/{database_name}"),
    }
}

fn sanitize_identifier(input: &str) -> String {
    let mut out = String::with_capacity(input.len());

    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }

    let sanitized: String = out.trim_matches('_').chars().take(20).collect();

    if sanitized.is_empty() {
        "test".to_string()
    } else {
        sanitized
    }
}

fn validate_database_identifier(identifier: &str) -> Result<(), String> {
    if identifier.is_empty() || identifier.len() > 63 {
        return Err(format!(
            "database identifier must be between 1 and 63 bytes: {identifier}"
        ));
    }

    if !identifier
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(format!(
            "database identifier contains unsafe characters: {identifier}"
        ));
    }

    Ok(())
}

fn quote_identifier(identifier: &str) -> Result<String, String> {
    validate_database_identifier(identifier)?;
    Ok(format!("\"{identifier}\""))
}

fn test_name(test: &DiscoveredTest) -> String {
    test.path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("<unknown>")
        .to_string()
}

#[cfg(unix)]
fn make_new_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    unsafe {
        command.as_std_mut().pre_exec(|| {
            if setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn make_new_process_group(_command: &mut Command) {}

#[cfg(unix)]
async fn kill_process_group(child: &mut tokio::process::Child) -> Result<(), String> {
    if let Some(pid) = child.id() {
        unsafe {
            kill(-(pid as i32), SIGTERM);
        }
        time::sleep(Duration::from_millis(200)).await;
        if child.try_wait().map_err(|e| e.to_string())?.is_none() {
            unsafe {
                kill(-(pid as i32), SIGKILL);
            }
        }
    }
    let _ = child.wait().await;
    Ok(())
}

#[cfg(not(unix))]
async fn kill_process_group(child: &mut tokio::process::Child) -> Result<(), String> {
    let _ = child.start_kill();
    let _ = child.wait().await;
    Ok(())
}

async fn stop_child_process(child: &mut tokio::process::Child) -> Result<(), String> {
    if child.try_wait().map_err(|e| e.to_string())?.is_some() {
        return Ok(());
    }
    kill_process_group(child).await
}

#[cfg(unix)]
const SIGTERM: i32 = 15;
#[cfg(unix)]
const SIGKILL: i32 = 9;

#[cfg(unix)]
unsafe extern "C" {
    fn setsid() -> i32;
    fn kill(pid: i32, sig: i32) -> i32;
}

fn print_test_output(result: &TestResult) {
    let status = if result.passed {
        "Passed"
    } else if result.timed_out {
        "Failed (timeout)"
    } else {
        "Failed"
    };

    println!("\n=== {}: {} ===", status, result.name);

    if !result.stdout.is_empty() {
        print!("{}", result.stdout);
        if !result.stdout.ends_with('\n') {
            println!();
        }
    }

    if !result.stderr.is_empty() {
        eprint!("{}", result.stderr);
        if !result.stderr.ends_with('\n') {
            eprintln!();
        }
    }

    println!("{} ({:.2?})", result.detail, result.duration);
}

fn print_summary(results: &[TestResult]) {
    let passed = results.iter().filter(|result| result.passed).count();
    let failed = results.len() - passed;

    println!("\nUnified xtask test summary");
    println!("--------------------------");
    println!("Passed: {passed}");
    println!("Failed: {failed}");
    println!("\n{:<8} {:<6} {:<10} Test", "Status", "Kind", "Duration");
    println!("{:<8} {:<6} {:<10} ----", "------", "----", "--------");

    for result in results {
        let status = if result.passed { "Passed" } else { "Failed" };
        println!(
            "{:<8} {:<6} {:<10} {}",
            status,
            format!("{:?}", result.kind),
            format!("{:.2?}", result.duration),
            result.name
        );
    }
}

fn print_usage() {
    println!("{}", usage_text());
}

fn usage_text() -> &'static str {
    "Usage: cargo run --bin xtask -- <command>\n\nCommands:\n  test    Discover and run .hurl and *_test.sh tests\n  help    Print this help text"
}

fn print_test_usage() {
    println!("{}", test_usage_text());
}

fn test_usage_text() -> &'static str {
    "Usage: cargo run --bin xtask -- test [--test-dir <dir>] [--timeout-secs <seconds>] [--skip-hurl] [--skip-bash] [--skip-db] [--skip-server] [--manage-db]\n\nOptions:\n  --manage-db    Start managed PostgreSQL via tests/common/spawn-db.sh when DATABASE_URL/BASE_DB_URL is unreachable. Requires Docker Compose.\n\nBy default, xtask uses a reachable DATABASE_URL/BASE_DB_URL if available. It does not start Docker automatically. Real DB/server E2E tests are skipped when external prerequisites are unavailable."
}
