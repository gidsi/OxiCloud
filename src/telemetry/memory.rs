use std::{
    io,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Duration,
};

use tokio::{task, time};

static PROCESS_RESIDENT_MEMORY_BYTES: AtomicU64 = AtomicU64::new(0);
static PROCESS_MEMORY_COLLECTOR_STARTED: AtomicBool = AtomicBool::new(false);

const PROCESS_MEMORY_POLL_INTERVAL: Duration = Duration::from_secs(5);
const MAX_REASONABLE_PROCESS_MEMORY_BYTES: u64 = 100_000_000_000_000;

pub async fn initialize_process_memory_metrics() {
    if let Err(error) = refresh_process_memory_gauge().await {
        tracing::warn!(
            error = %error,
            "failed to collect initial process resident memory metric"
        );
    }

    spawn_process_memory_collector();
}

pub fn process_resident_memory_bytes() -> u64 {
    PROCESS_RESIDENT_MEMORY_BYTES.load(Ordering::Relaxed)
}

pub fn render_prometheus_metrics() -> String {
    let resident_memory_bytes = process_resident_memory_bytes();

    format!(
        concat!(
            "# HELP process_resident_memory_bytes Resident memory size in bytes.\n",
            "# TYPE process_resident_memory_bytes gauge\n",
            "process_resident_memory_bytes {}\n",
        ),
        resident_memory_bytes
    )
}

pub async fn poll_process_memory() -> Result<u64, String> {
    let memory_bytes = task::spawn_blocking(read_process_resident_memory_bytes)
        .await
        .map_err(|error| format!("process memory polling task failed: {error}"))?
        .map_err(|error| format!("failed to read process resident memory: {error}"))?;

    if memory_bytes == 0 {
        return Err("process resident memory was reported as 0 bytes".to_string());
    }

    if memory_bytes >= MAX_REASONABLE_PROCESS_MEMORY_BYTES {
        return Err(format!(
            "process resident memory value is implausibly large: {memory_bytes}"
        ));
    }

    Ok(memory_bytes)
}

async fn refresh_process_memory_gauge() -> Result<u64, String> {
    let memory_bytes = poll_process_memory().await?;
    PROCESS_RESIDENT_MEMORY_BYTES.store(memory_bytes, Ordering::Relaxed);
    Ok(memory_bytes)
}

fn spawn_process_memory_collector() {
    if PROCESS_MEMORY_COLLECTOR_STARTED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    task::spawn(async {
        let mut interval = time::interval(PROCESS_MEMORY_POLL_INTERVAL);
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if let Err(error) = refresh_process_memory_gauge().await {
                tracing::warn!(
                    error = %error,
                    "failed to refresh process resident memory metric"
                );
            }
        }
    });
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(target_os = "linux")]
fn read_process_resident_memory_bytes() -> io::Result<u64> {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        if let Some(bytes) = parse_proc_status_vm_rss_bytes(&status) {
            return Ok(bytes);
        }
    }

    let statm = std::fs::read_to_string("/proc/self/statm")?;

    parse_proc_statm_resident_bytes(&statm).ok_or_else(|| {
        invalid_data("could not parse resident memory from /proc/self/status or /proc/self/statm")
    })
}

#[cfg(target_os = "linux")]
fn parse_proc_status_vm_rss_bytes(status: &str) -> Option<u64> {
    status.lines().find_map(|line| {
        let line = line.trim_start();
        let value = line.strip_prefix("VmRSS:")?;

        let mut parts = value.split_whitespace();
        let amount = parts.next()?.parse::<u64>().ok()?;
        let unit = parts.next().unwrap_or("kB");

        let multiplier = match unit {
            "B" => 1,
            "kB" | "KB" | "KiB" => 1024,
            "mB" | "MB" | "MiB" => 1024 * 1024,
            _ => 1024,
        };

        amount.checked_mul(multiplier)
    })
}

#[cfg(target_os = "linux")]
fn parse_proc_statm_resident_bytes(statm: &str) -> Option<u64> {
    let resident_pages = statm.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    resident_pages.checked_mul(linux_page_size_bytes())
}

#[cfg(target_os = "linux")]
fn linux_page_size_bytes() -> u64 {
    4096
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
fn read_process_resident_memory_bytes() -> io::Result<u64> {
    let pid = std::process::id().to_string();

    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", pid.as_str()])
        .output()?;

    if !output.status.success() {
        return Err(invalid_data(
            "ps command failed while reading process resident memory",
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| invalid_data(format!("ps output was not valid UTF-8: {error}")))?;

    let rss_kib = stdout.trim().parse::<u64>().map_err(|error| {
        invalid_data(format!(
            "could not parse resident memory from ps output: {error}"
        ))
    })?;

    rss_kib
        .checked_mul(1024)
        .ok_or_else(|| invalid_data("resident memory byte count overflowed"))
}

#[cfg(target_os = "windows")]
fn read_process_resident_memory_bytes() -> io::Result<u64> {
    let pid = std::process::id();
    let command = format!("(Get-Process -Id {pid}).WorkingSet64");

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", command.as_str()])
        .output()?;

    if !output.status.success() {
        return Err(invalid_data(
            "powershell command failed while reading process resident memory",
        ));
    }

    let stdout = String::from_utf8(output.stdout).map_err(|error| {
        invalid_data(format!(
            "powershell output was not valid UTF-8: {error}"
        ))
    })?;

    stdout.trim().parse::<u64>().map_err(|error| {
        invalid_data(format!(
            "could not parse resident memory from powershell output: {error}"
        ))
    })
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
    target_os = "windows"
)))]
fn read_process_resident_memory_bytes() -> io::Result<u64> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "process resident memory polling is not supported on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_poll_process_memory_success() {
        let result = poll_process_memory().await;

        assert!(
            result.is_ok(),
            "Memory polling should succeed without panicking or returning an error, but got: {:?}",
            result
        );

        let memory_bytes = result.unwrap();

        assert!(
            memory_bytes > 0,
            "Polled memory bytes should be strictly greater than 0"
        );

        assert!(
            memory_bytes < MAX_REASONABLE_PROCESS_MEMORY_BYTES,
            "Polled memory bytes seems impossibly large, potential integer underflow/overflow bug"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_linux_proc_status_vm_rss() {
        let status = "\
Name:\toxicloud
VmPeak:\t  100000 kB
VmSize:\t   90000 kB
VmRSS:\t   12345 kB
";

        assert_eq!(parse_proc_status_vm_rss_bytes(status), Some(12_641_280));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parses_linux_proc_statm_resident_pages() {
        let statm = "1000 42 0 0 0 0 0";

        assert_eq!(
            parse_proc_statm_resident_bytes(statm),
            Some(42 * linux_page_size_bytes())
        );
    }
}
