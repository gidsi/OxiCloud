use glob::glob;
use sqlx::{Executor, PgPool, postgres::PgPoolOptions};
use std::env;
use std::ffi::OsStr;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

const DEFAULT_BASE_DB_URL: &str =
    "postgres://oxicloud_test:oxicloud_test@localhost:5433/oxicloud_test";
const TEST_USERNAME: &str = "admin";
const TEST_EMAIL: &str = "admin@example.com";
const TEST_PASSWORD: &str = "TestPassword1!";
const TEST_JWT_SECRET: &str = "test-secret-do-not-use-in-prod-minimum-32-chars";

#[derive(Debug, thiserror::Error)]
enum XtaskError {
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("missing value for {0}")]
    MissingValue(String),
    #[error("glob error: {0}")]
    Glob(#[from] glob::PatternError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("hurl binary is required to run {0} discovered .hurl test(s)")]
    MissingHurl(usize),
    #[error("bash binary is required to run {0} discovered shell test(s)")]
    MissingBash(usize),
    #[error("could not allocate a free localhost port")]
    NoFreePort,
    #[error("server failed to become ready at {base_url}: {reason}")]
    ServerNotReady { base_url: String, reason: String },
    #[error("invalid generated database identifier: {0}")]
    InvalidDatabaseIdentifier(String),
    #[error("{0} xtask test(s) failed")]
    TestsFailed(usize),
    #[error("no discovered xtask tests could be executed; {0} skipped")]
    NoExecutableTests(usize),
    #[error("PostgreSQL test database unavailable: {0}")]
    PostgresUnavailable(String),
    #[error(
        "PostgreSQL test database startup timed out after {timeout_secs}s while running {script}"
    )]
    PostgresStartupTimedOut { script: String, timeout_secs: u64 },
    #[error("PostgreSQL test database startup failed while running {script}: {reason}")]
    PostgresStartupFailed { script: String, reason: String },
}

type Result<T> = std::result::Result<T, XtaskError>;

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("xtask error: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".to_string());

    match command.as_str() {
        "test" => {
            let mut tests_dir = PathBuf::from("tests");
            let mut timeout_secs = 30_u64;
            let mut strict_tools = strict_tools_from_env();
            let mut strict_infra = strict_infra_from_env();

            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--tests-dir" => {
                        tests_dir = args
                            .next()
                            .map(PathBuf::from)
                            .ok_or_else(|| XtaskError::MissingValue(arg.clone()))?;
                    }
                    "--timeout-secs" => {
                        timeout_secs = args
                            .next()
                            .ok_or_else(|| XtaskError::MissingValue(arg.clone()))?
                            .parse()
                            .unwrap_or(30);
                    }
                    "--strict-tools" => {
                        strict_tools = true;
                    }
                    "--strict-infra" => {
                        strict_infra = true;
                    }
                    "--strict" => {
                        strict_tools = true;
                        strict_infra = true;
                    }
                    other => eprintln!("Ignoring unknown test option {other}"),
                }
            }

            run_tests(
                &tests_dir,
                Duration::from_secs(timeout_secs),
                strict_tools,
                strict_infra,
            )
            .await
        }
        "help" | "--help" | "-h" => {
            println!(
                "Usage: cargo run -p xtask -- test [--tests-dir DIR] [--timeout-secs SECONDS] [--strict-tools] [--strict-infra] [--strict]"
            );
            Ok(())
        }
        other => Err(XtaskError::UnknownCommand(other.to_string())),
    }
}

async fn run_tests(
    tests_dir: &Path,
    timeout_duration: Duration,
    strict_tools: bool,
    strict_infra: bool,
) -> Result<()> {
    let run_uuid = Uuid::new_v4();
    let tests = discover_tests(tests_dir)?;

    if tests.is_empty() {
        println!(
            "No .hurl or *_test.sh tests discovered under {}",
            tests_dir.display()
        );
        return Ok(());
    }

    for test in &tests {
        println!(
            "Discovered {} test: {}",
            test.kind.label(),
            test.path.display()
        );
    }

    println!(
        "Creating isolated state for xtask run UUID {run_uuid}; each executed test receives an isolated database, storage path, server port, and environment"
    );
    println!("Using test timeout of {:?}", timeout_duration);

    let tools = detect_tool_availability(&tests).await;
    enforce_strict_tool_availability(&tests, tools, strict_tools)?;

    let repo_root = env::current_dir()?;
    let postgres_availability = detect_postgres_availability(&repo_root).await;
    enforce_strict_postgres_availability(&tests, &postgres_availability, strict_infra)?;

    let mut postgres_guard = None;
    let mut admin_pool = None;
    let base_db_url = base_database_url();
    let mut passed = 0usize;
    let mut failures = 0usize;
    let mut skipped = 0usize;

    for test in tests {
        match test.kind {
            TestKind::Hurl if !tools.hurl => {
                skipped += 1;
                eprintln!("SKIP {}: hurl binary is not available", test.path.display());
                continue;
            }
            TestKind::Shell if !tools.bash => {
                skipped += 1;
                eprintln!("SKIP {}: bash binary is not available", test.path.display());
                continue;
            }
            _ => {}
        }

        if let PostgresAvailability::Unavailable(reason) = &postgres_availability {
            skipped += 1;
            eprintln!(
                "SKIP {}: PostgreSQL test database is unavailable: {reason}",
                test.path.display()
            );
            continue;
        }

        if admin_pool.is_none() {
            postgres_guard =
                Some(ensure_postgres_available(&repo_root, &postgres_availability).await?);
            admin_pool = Some(
                PgPoolOptions::new()
                    .max_connections(2)
                    .connect(&base_db_url)
                    .await?,
            );
        }

        let admin_pool = admin_pool
            .as_ref()
            .expect("admin_pool is initialized before executing an xtask test");
        let test_uuid = Uuid::new_v4();
        println!(
            "Preparing isolated database state UUID {test_uuid} for {}",
            test.path.display()
        );

        match run_one_test(
            &repo_root,
            admin_pool,
            &base_db_url,
            &test,
            test_uuid,
            timeout_duration,
        )
        .await
        {
            Ok(true) => {
                passed += 1;
                println!("PASS {}", test.path.display());
            }
            Ok(false) => {
                failures += 1;
                eprintln!("FAIL {}", test.path.display());
            }
            Err(err) => {
                failures += 1;
                eprintln!("FAIL {}: {err}", test.path.display());
            }
        }
    }

    if let Some(pool) = admin_pool {
        pool.close().await;
    }
    if let Some(mut guard) = postgres_guard {
        guard.stop().await;
    }

    let discovered = passed + failures + skipped;
    let executed = passed + failures;
    println!(
        "xtask summary: {discovered} discovered, {executed} executed, {failures} failed, {skipped} skipped"
    );

    if executed == 0 {
        println!("No executable xtask tests were run; {skipped} skipped");
        if strict_tools || strict_infra {
            return Err(XtaskError::NoExecutableTests(skipped));
        }
        return Ok(());
    }

    if failures > 0 {
        eprintln!("{failures} xtask test(s) failed");
        return Err(XtaskError::TestsFailed(failures));
    }

    Ok(())
}

fn discover_tests(tests_dir: &Path) -> Result<Vec<TestFile>> {
    let hurl_pattern = tests_dir.join("**/*.hurl").to_string_lossy().to_string();
    let sh_pattern = tests_dir.join("**/*_test.sh").to_string_lossy().to_string();

    let mut tests = Vec::new();

    for entry in glob(&hurl_pattern)? {
        match entry {
            Ok(path) => tests.push(TestFile {
                kind: TestKind::Hurl,
                path,
            }),
            Err(err) => eprintln!("glob traversal error: {err}"),
        }
    }

    for entry in glob(&sh_pattern)? {
        match entry {
            Ok(path) => tests.push(TestFile {
                kind: TestKind::Shell,
                path,
            }),
            Err(err) => eprintln!("glob traversal error: {err}"),
        }
    }

    tests.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(tests)
}

#[derive(Debug, Clone, Copy)]
struct ToolAvailability {
    hurl: bool,
    bash: bool,
}

async fn detect_tool_availability(tests: &[TestFile]) -> ToolAvailability {
    let needs_hurl = tests.iter().any(|test| matches!(test.kind, TestKind::Hurl));
    let needs_bash = tests
        .iter()
        .any(|test| matches!(test.kind, TestKind::Shell));

    ToolAvailability {
        hurl: !needs_hurl || binary_available("hurl").await,
        bash: !needs_bash || binary_available("bash").await,
    }
}

fn enforce_strict_tool_availability(
    tests: &[TestFile],
    tools: ToolAvailability,
    strict_tools: bool,
) -> Result<()> {
    if !strict_tools {
        return Ok(());
    }

    let hurl_count = tests
        .iter()
        .filter(|test| matches!(test.kind, TestKind::Hurl))
        .count();
    let shell_count = tests
        .iter()
        .filter(|test| matches!(test.kind, TestKind::Shell))
        .count();

    if hurl_count > 0 && !tools.hurl {
        return Err(XtaskError::MissingHurl(hurl_count));
    }

    if shell_count > 0 && !tools.bash {
        return Err(XtaskError::MissingBash(shell_count));
    }

    Ok(())
}

fn strict_tools_from_env() -> bool {
    env_flag_enabled("OXICLOUD_XTASK_STRICT_TOOLS")
}

fn strict_infra_from_env() -> bool {
    env_flag_enabled("OXICLOUD_XTASK_STRICT_INFRA")
}

fn env_flag_enabled(name: &str) -> bool {
    env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

async fn binary_available(binary: &str) -> bool {
    Command::new(binary)
        .arg("--version")
        .kill_on_drop(true)
        .output()
        .await
        .is_ok_and(|output| output.status.success())
}

async fn command_starts(binary: &str, args: &[&str]) -> bool {
    Command::new(binary)
        .args(args)
        .kill_on_drop(true)
        .output()
        .await
        .is_ok()
}

#[derive(Debug, Clone)]
enum PostgresAvailability {
    ExternalReachable,
    SpawnableWithDocker,
    Unavailable(String),
}

async fn detect_postgres_availability(repo_root: &Path) -> PostgresAvailability {
    let base_db_url = base_database_url();
    if base_database_reachable(&base_db_url).await {
        return PostgresAvailability::ExternalReachable;
    }

    let spawn_script = repo_root.join("tests/common/spawn-db.sh");
    if !spawn_script.exists() {
        return PostgresAvailability::Unavailable(format!(
            "base database is unreachable and {} is missing",
            spawn_script.display()
        ));
    }

    let mut missing = Vec::new();
    if !binary_available("bash").await {
        missing.push("bash");
    }
    if !binary_available("docker").await {
        missing.push("docker");
    } else if !docker_compose_available().await {
        missing.push("docker compose");
    }
    if !command_starts("nc", &["-h"]).await {
        missing.push("nc");
    }

    if missing.is_empty() {
        PostgresAvailability::SpawnableWithDocker
    } else {
        PostgresAvailability::Unavailable(format!(
            "base database is unreachable and {} {} not available",
            missing.join(", "),
            if missing.len() == 1 { "is" } else { "are" }
        ))
    }
}

async fn base_database_reachable(base_db_url: &str) -> bool {
    PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(2))
        .connect(base_db_url)
        .await
        .is_ok()
}

async fn docker_compose_available() -> bool {
    Command::new("docker")
        .args(["compose", "version"])
        .kill_on_drop(true)
        .output()
        .await
        .is_ok_and(|output| output.status.success())
}

fn enforce_strict_postgres_availability(
    tests: &[TestFile],
    availability: &PostgresAvailability,
    strict_infra: bool,
) -> Result<()> {
    if !strict_infra || tests.is_empty() {
        return Ok(());
    }

    if let PostgresAvailability::Unavailable(reason) = availability {
        return Err(XtaskError::PostgresUnavailable(reason.clone()));
    }

    Ok(())
}

async fn ensure_postgres_available(
    repo_root: &Path,
    availability: &PostgresAvailability,
) -> Result<PostgresGuard> {
    match availability {
        PostgresAvailability::ExternalReachable => Ok(PostgresGuard::External),
        PostgresAvailability::Unavailable(reason) => {
            Err(XtaskError::PostgresUnavailable(reason.clone()))
        }
        PostgresAvailability::SpawnableWithDocker => spawn_postgres(repo_root).await,
    }
}

async fn spawn_postgres(repo_root: &Path) -> Result<PostgresGuard> {
    let spawn_script = repo_root.join("tests/common/spawn-db.sh");
    let startup_timeout = Duration::from_secs(60);

    println!(
        "Base PostgreSQL database is not reachable; starting test database with {}",
        spawn_script.display()
    );

    let status = timeout(
        startup_timeout,
        Command::new("bash")
            .arg(&spawn_script)
            .current_dir(repo_root)
            .kill_on_drop(true)
            .status(),
    )
    .await
    .map_err(|_| XtaskError::PostgresStartupTimedOut {
        script: spawn_script.display().to_string(),
        timeout_secs: startup_timeout.as_secs(),
    })??;

    if !status.success() {
        return Err(XtaskError::PostgresStartupFailed {
            script: spawn_script.display().to_string(),
            reason: status.to_string(),
        });
    }

    let base_db_url = base_database_url();
    if !base_database_reachable(&base_db_url).await {
        return Err(XtaskError::PostgresStartupFailed {
            script: spawn_script.display().to_string(),
            reason: "database did not become reachable after startup script completed".to_string(),
        });
    }

    Ok(PostgresGuard::Spawned {
        stop_script: repo_root.join("tests/common/stop-db.sh"),
        repo_root: repo_root.to_path_buf(),
    })
}

async fn run_one_test(
    repo_root: &Path,
    admin_pool: &PgPool,
    base_db_url: &str,
    test: &TestFile,
    test_uuid: Uuid,
    timeout_duration: Duration,
) -> Result<bool> {
    let context = TestContext::create(repo_root, admin_pool, base_db_url, test_uuid).await?;
    let mut server = None;

    let result = async {
        let spawned_server = start_server(repo_root, &context).await?;
        server = Some(spawned_server);
        wait_for_server(&context.base_url, timeout_duration).await?;
        execute_test(repo_root, test, &context, timeout_duration).await
    }
    .await;

    if let Some(mut child) = server {
        stop_child(&mut child).await;
    }

    context.cleanup(admin_pool).await;
    result
}

async fn start_server(repo_root: &Path, context: &TestContext) -> Result<Child> {
    tokio::fs::create_dir_all(&context.storage_path).await?;

    let binary = oxicloud_binary_path(repo_root);
    let mut command = if binary.exists() {
        let mut command = Command::new(binary);
        command.current_dir(repo_root);
        command
    } else {
        let mut command = Command::new("cargo");
        command
            .args(["run", "--bin", "oxicloud"])
            .current_dir(repo_root);
        command
    };

    command
        .kill_on_drop(true)
        .env("DATABASE_URL", &context.database_url)
        .env("OXICLOUD_DB_CONNECTION_STRING", &context.database_url)
        .env("OXICLOUD_SERVER_HOST", "127.0.0.1")
        .env("OXICLOUD_SERVER_PORT", context.server_port.to_string())
        .env("OXICLOUD_STORAGE_PATH", &context.storage_path)
        .env("OXICLOUD_STATIC_PATH", repo_root.join("static"))
        .env("OXICLOUD_JWT_SECRET", TEST_JWT_SECRET)
        .env("OXICLOUD_ENABLE_AUTH", "true")
        .env("OXICLOUD_ENABLE_TRASH", "true")
        .env("OXICLOUD_ENABLE_SEARCH", "true")
        .env("OXICLOUD_ENABLE_FILE_SHARING", "true")
        .env("OXICLOUD_ENABLE_MUSIC", "true")
        .env("OXICLOUD_EXPOSE_SYSTEM_USERS", "true")
        .env("OXICLOUD_WOPI_ENABLED", "false")
        .env("OXICLOUD_OIDC_ENABLED", "false")
        .env("OXICLOUD_RATE_LIMIT_REFRESH_MAX", "120")
        .env("OXICLOUD_RATE_LIMIT_LOGIN_MAX", "120")
        .env(
            "RUST_LOG",
            env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()),
        );

    Ok(command.spawn()?)
}

async fn wait_for_server(base_url: &str, max_wait: Duration) -> Result<()> {
    let client = reqwest::Client::new();
    let deadline = Instant::now() + max_wait.max(Duration::from_secs(5));
    let mut last_error = String::from("server did not respond");

    while Instant::now() < deadline {
        match client.get(format!("{base_url}/ready")).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) => {
                last_error = format!("/ready returned {}", response.status());
                sleep(Duration::from_millis(125)).await;
            }
            Err(err) => {
                last_error = err.to_string();
                sleep(Duration::from_millis(125)).await;
            }
        }

        match client.get(format!("{base_url}/health")).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) => {
                let health_error = format!("/health returned {}", response.status());
                last_error = format!("{last_error}; {health_error}");
            }
            Err(err) => {
                let health_error = err.to_string();
                last_error = format!("{last_error}; {health_error}");
            }
        }

        sleep(Duration::from_millis(250)).await;
    }

    Err(XtaskError::ServerNotReady {
        base_url: base_url.to_string(),
        reason: last_error,
    })
}

async fn execute_test(
    repo_root: &Path,
    test: &TestFile,
    context: &TestContext,
    timeout_duration: Duration,
) -> Result<bool> {
    let mut command = match test.kind {
        TestKind::Hurl => hurl_command(repo_root, test, context),
        TestKind::Shell => shell_command(test, context),
    };

    command
        .kill_on_drop(true)
        .current_dir(repo_root)
        .env("DATABASE_URL", &context.database_url)
        .env("OXICLOUD_DB_CONNECTION_STRING", &context.database_url)
        .env("OXICLOUD_STORAGE_PATH", &context.storage_path)
        .env("OXICLOUD_TEST_UUID", context.test_uuid.to_string())
        .env(
            "OXICLOUD_ISOLATED_STATE",
            format!("xtask_{}", context.test_uuid),
        )
        .env("base_url", &context.base_url)
        .env("username", &context.username)
        .env("email", &context.email)
        .env("password", &context.password);

    let mut child = command.spawn()?;

    match timeout(timeout_duration, child.wait()).await {
        Ok(status) => Ok(status?.success()),
        Err(_) => {
            stop_child(&mut child).await;
            eprintln!(
                "TIMEOUT {} after {}s",
                test.path.display(),
                timeout_duration.as_secs()
            );
            Ok(false)
        }
    }
}

fn hurl_command(repo_root: &Path, test: &TestFile, context: &TestContext) -> Command {
    let mut command = Command::new("hurl");
    command
        .arg("--test")
        .arg("--file-root")
        .arg(repo_root.join("tests"));

    let variables_file = repo_root.join("tests/api/test.env");
    if variables_file.exists() {
        command.arg("--variables-file").arg(variables_file);
    }

    command
        .arg("--variable")
        .arg(format!("base_url={}", context.base_url))
        .arg("--variable")
        .arg(format!("username={}", context.username))
        .arg("--variable")
        .arg(format!("email={}", context.email))
        .arg("--variable")
        .arg(format!("password={}", context.password));

    if should_bootstrap_with_setup(&test.path) {
        let setup = repo_root.join("tests/api/setup.hurl");
        if setup.exists() {
            command.arg(setup);
        }
    }

    command.arg(&test.path);
    command
}

fn shell_command(test: &TestFile, context: &TestContext) -> Command {
    let mut command = Command::new("bash");
    command
        .arg(&test.path)
        .arg(format!("127.0.0.1:{}", context.server_port))
        .arg(&context.username)
        .arg(&context.password);
    command
}

fn should_bootstrap_with_setup(test_path: &Path) -> bool {
    test_path.file_name() != Some(OsStr::new("setup.hurl"))
}

async fn stop_child(child: &mut Child) {
    match child.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }
}

fn oxicloud_binary_path(repo_root: &Path) -> PathBuf {
    if let Ok(path) = env::var("OXICLOUD_BIN") {
        return PathBuf::from(path);
    }

    let profile = env::var("BUILD_TARGET").unwrap_or_else(|_| "debug".to_string());
    let binary_name = if cfg!(windows) {
        "oxicloud.exe"
    } else {
        "oxicloud"
    };

    repo_root.join("target").join(profile).join(binary_name)
}

fn base_database_url() -> String {
    env::var("OXICLOUD_DB_CONNECTION_STRING")
        .or_else(|_| env::var("DATABASE_URL"))
        .unwrap_or_else(|_| DEFAULT_BASE_DB_URL.to_string())
}

fn database_url_with_db(base_db_url: &str, db_name: &str) -> String {
    let (without_query, query) = base_db_url
        .split_once('?')
        .map(|(url, query)| (url, Some(query)))
        .unwrap_or((base_db_url, None));

    let base = without_query
        .rsplit_once('/')
        .map(|(prefix, _)| prefix)
        .unwrap_or(without_query);

    match query {
        Some(query) => format!("{base}/{db_name}?{query}"),
        None => format!("{base}/{db_name}"),
    }
}

fn quote_pg_identifier(identifier: &str) -> Result<String> {
    let valid = identifier
        .chars()
        .enumerate()
        .all(|(idx, ch)| ch == '_' || ch.is_ascii_digit() && idx > 0 || ch.is_ascii_lowercase());

    if !valid || identifier.is_empty() {
        return Err(XtaskError::InvalidDatabaseIdentifier(
            identifier.to_string(),
        ));
    }

    Ok(format!("\"{identifier}\""))
}

fn allocate_port() -> Result<u16> {
    for _ in 0..10 {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        drop(listener);
        if port > 0 {
            return Ok(port);
        }
    }

    Err(XtaskError::NoFreePort)
}

struct TestContext {
    test_uuid: Uuid,
    db_name: String,
    database_url: String,
    storage_path: PathBuf,
    server_port: u16,
    base_url: String,
    username: String,
    email: String,
    password: String,
}

impl TestContext {
    async fn create(
        repo_root: &Path,
        admin_pool: &PgPool,
        base_db_url: &str,
        test_uuid: Uuid,
    ) -> Result<Self> {
        let db_name = format!("oxicloud_xtask_{}", test_uuid.simple());
        let quoted_db_name = quote_pg_identifier(&db_name)?;

        admin_pool
            .execute(format!("CREATE DATABASE {quoted_db_name}").as_str())
            .await?;

        let database_url = database_url_with_db(base_db_url, &db_name);
        let storage_path = repo_root
            .join("target")
            .join("xtask-test-state")
            .join(test_uuid.to_string())
            .join("storage");
        let server_port = allocate_port()?;
        let base_url = format!("http://127.0.0.1:{server_port}");

        Ok(Self {
            test_uuid,
            db_name,
            database_url,
            storage_path,
            server_port,
            base_url,
            username: TEST_USERNAME.to_string(),
            email: TEST_EMAIL.to_string(),
            password: TEST_PASSWORD.to_string(),
        })
    }

    async fn cleanup(&self, admin_pool: &PgPool) {
        let _ = tokio::fs::remove_dir_all(
            self.storage_path
                .parent()
                .unwrap_or(self.storage_path.as_path()),
        )
        .await;

        if let Ok(quoted_db_name) = quote_pg_identifier(&self.db_name) {
            let _ = sqlx::query(
                "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1 AND pid <> pg_backend_pid()",
            )
            .bind(&self.db_name)
            .execute(admin_pool)
            .await;

            let _ = admin_pool
                .execute(format!("DROP DATABASE IF EXISTS {quoted_db_name}").as_str())
                .await;
        }
    }
}

enum PostgresGuard {
    External,
    Spawned {
        stop_script: PathBuf,
        repo_root: PathBuf,
    },
}

impl PostgresGuard {
    async fn stop(&mut self) {
        if let Self::Spawned {
            stop_script,
            repo_root,
        } = self
        {
            if stop_script.exists() {
                let _ = Command::new("bash")
                    .arg(stop_script)
                    .current_dir(repo_root)
                    .kill_on_drop(true)
                    .status()
                    .await;
            }
        }
    }
}

#[derive(Debug)]
struct TestFile {
    kind: TestKind,
    path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestKind {
    Hurl,
    Shell,
}

impl TestKind {
    fn label(self) -> &'static str {
        match self {
            Self::Hurl => "Hurl",
            Self::Shell => "Shell",
        }
    }
}
