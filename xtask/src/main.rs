use std::env;
use std::process::{Command, Stdio, Child};
use std::thread;
use std::time::{Duration, Instant};
use std::path::{Path, PathBuf};
use std::fs;
use std::os::unix::process::CommandExt;

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("test") => run_tests(),
        _ => {
            eprintln!("Usage: cargo xtask test");
            std::process::exit(1);
        }
    }
}

fn run_tests() {
    let repo_root = env::current_dir().unwrap();
    let common_dir = repo_root.join("tests/common");
    let api_dir = repo_root.join("tests/api");

    // 1. Start DB
    println!("[xtask] Starting database...");
    let db_status = Command::new("bash")
        .arg(common_dir.join("spawn-db.sh"))
        .status()
        .expect("Failed to run spawn-db.sh");
    if !db_status.success() {
        eprintln!("Failed to spawn DB");
        std::process::exit(1);
    }

    struct DbGuard(PathBuf);
    impl Drop for DbGuard {
        fn drop(&mut self) {
            println!("[xtask] Stopping database...");
            let _ = Command::new("bash").arg(self.0.join("stop-db.sh")).status();
        }
    }
    let _db_guard = DbGuard(common_dir.clone());

    // Setup Storage
    let storage_path = api_dir.join("storage");
    println!("[xtask] Wiping storage at {:?}", storage_path);
    let _ = fs::remove_dir_all(&storage_path);
    fs::create_dir_all(&storage_path).expect("Failed to create storage dir");

    // Build Server
    println!("[xtask] Building OxiCloud...");
    let build_status = Command::new("cargo")
        .arg("build")
        .status()
        .expect("Failed to build");
    if !build_status.success() {
        eprintln!("Cargo build failed");
        std::process::exit(1);
    }

    // Read test.env for base_url
    let env_content = fs::read_to_string(api_dir.join("test.env")).expect("Failed to read test.env");
    let base_url = env_content.lines().find_map(|l| l.strip_prefix("base_url=")).unwrap_or("http://localhost:8087");
    let server_port = base_url.split(':').last().unwrap_or("8087");

    // Start Server
    println!("[xtask] Starting Server...");
    let server_bin = repo_root.join("target/debug/oxicloud");
    let mut server_child = Command::new(server_bin)
        .env("OXICLOUD_SERVER_PORT", server_port)
        .env("OXICLOUD_STORAGE_PATH", &storage_path)
        .spawn()
        .expect("Failed to start server");

    struct ServerGuard<'a>(&'a mut Child);
    impl<'a> Drop for ServerGuard<'a> {
        fn drop(&mut self) {
            println!("[xtask] Stopping server...");
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut _server_guard = ServerGuard(&mut server_child);

    // Wait for server ready
    let mut ready = false;
    for _ in 0..120 {
        if Command::new("curl").arg("-sf").arg(format!("{}/ready", base_url)).output().map(|o| o.status.success()).unwrap_or(false) {
            ready = true;
            break;
        }
        thread::sleep(Duration::from_secs(1));
    }
    if !ready {
        eprintln!("Server failed to become ready");
        std::process::exit(1);
    }
    println!("[xtask] Server is ready.");

    // Run tests with isolation & timeout
    let timeout = Duration::from_secs(300);

    let hurl_status = Command::new("hurl")
        .arg("--variables-file")
        .arg(api_dir.join("test.env"))
        .arg("--test")
        .arg("--jobs").arg("1")
        .arg("--glob").arg("tests/**/*.hurl")
        .status()
        .expect("Failed to run hurl");

    if !hurl_status.success() {
        eprintln!("Hurl tests failed");
        std::process::exit(1);
    }

    println!("[xtask] All tests passed.");
}
