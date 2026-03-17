use std::path::Path;
use std::process::Child;
use std::time::{Duration, Instant};

use rayon::prelude::*;
use tempfile::TempDir;
use wait_timeout::ChildExt;

use crate::command::{build_command, display_command};
use crate::config::OracleConfig;
use crate::parser::extract_ice_messages;
use crate::result::{RunOutcome, TerminationKind};

/// How often to poll RSS on macOS (where RLIMIT_DATA is not enforced).
#[cfg(target_os = "macos")]
const MEMORY_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Run a single (variant_index, file) compilation and return its outcome.
pub fn run_single(config: &OracleConfig, variant_index: usize, file: &Path) -> RunOutcome {
    let variant = &config.variants[variant_index];
    let mut cmd = build_command(config, variant, file);
    let command_display = display_command(&cmd);

    let tempdir = TempDir::new().expect("failed to create temp dir");
    cmd.current_dir(tempdir.path());

    // Capture stderr (stdout not needed)
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::piped());

    let timestamp_utc = utc_now_rfc3339();

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return RunOutcome {
                file: file.to_path_buf(),
                variant_label: variant.label.clone(),
                command_display,
                exit_status: None,
                termination: TerminationKind::SpawnFailed,
                peak_rss_mb: None,
                is_ice: false,
                ices: Vec::new(),
                stderr: format!("failed to spawn: {e}"),
                timestamp_utc,
            };
        }
    };

    let timeout = Duration::from_secs(config.resource_limits.timeout_secs as u64);
    let memory_limit_mb = config.resource_limits.memory_limit_mb;

    let (exit_status, termination, peak_rss_mb) =
        wait_with_limits(&mut child, timeout, memory_limit_mb);

    let stderr = child
        .stderr
        .take()
        .map(|mut pipe| {
            let mut buf = String::new();
            use std::io::Read;
            let _ = pipe.read_to_string(&mut buf);
            buf
        })
        .unwrap_or_default();

    // On Linux with prlimit, OOM causes SIGABRT (allocator abort) or SIGKILL
    // rather than being detected as MemoryExceeded. Reclassify based on stderr.
    let termination = match &termination {
        TerminationKind::Signal(6 | 9)
            if crate::result::stderr_hints_oom(&stderr) =>
        {
            TerminationKind::MemoryExceeded
        }
        _ => termination,
    };

    let ices = extract_ice_messages(&stderr);
    let is_ice = !ices.is_empty();

    RunOutcome {
        file: file.to_path_buf(),
        variant_label: variant.label.clone(),
        command_display,
        exit_status,
        termination,
        peak_rss_mb,
        is_ice,
        ices,
        stderr,
        timestamp_utc,
    }
}

/// Classify how a process exited: normal exit code, or killed by a signal.
fn classify_exit(status: std::process::ExitStatus) -> (Option<i32>, TerminationKind) {
    if let Some(code) = status.code() {
        return (Some(code), TerminationKind::Normal);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return (None, TerminationKind::Signal(sig));
        }
    }
    (None, TerminationKind::Unknown)
}

// ---------------------------------------------------------------------------
// macOS: RLIMIT_DATA is not enforced on Apple Silicon, so we poll RSS via
// proc_pidinfo and kill the process ourselves when it exceeds the limit.
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn wait_with_limits(
    child: &mut Child,
    timeout: Duration,
    memory_limit_mb: u32,
) -> (Option<i32>, TerminationKind, Option<u64>) {
    let deadline = Instant::now() + timeout;
    let pid = child.id();
    let mut peak_rss_mb: u64 = 0;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let _ = child.kill();
            let _ = child.wait();
            return (None, TerminationKind::WallTimeout, Some(peak_rss_mb));
        }

        let wait_dur = remaining.min(MEMORY_POLL_INTERVAL);
        match child.wait_timeout(wait_dur) {
            Ok(Some(status)) => {
                let (code, term) = classify_exit(status);
                return (code, term, Some(peak_rss_mb));
            }
            Ok(None) => {
                // Still running — check memory
                if let Some(rss) = get_rss_mb(pid) {
                    peak_rss_mb = peak_rss_mb.max(rss);
                    if rss > memory_limit_mb as u64 {
                        let _ = child.kill();
                        let _ = child.wait();
                        return (
                            None,
                            TerminationKind::MemoryExceeded,
                            Some(peak_rss_mb),
                        );
                    }
                }
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return (None, TerminationKind::Unknown, Some(peak_rss_mb));
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn get_rss_mb(pid: u32) -> Option<u64> {
    unsafe {
        let mut info: libc::proc_taskinfo = std::mem::zeroed();
        let size = std::mem::size_of::<libc::proc_taskinfo>() as libc::c_int;
        let ret = libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTASKINFO,
            0,
            &mut info as *mut _ as *mut libc::c_void,
            size,
        );
        if ret == size {
            Some(info.pti_resident_size / (1024 * 1024))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Linux: prlimit sets RLIMIT_AS + RLIMIT_CPU. Wall-clock timeout is handled
// by wait_timeout. OOM signals (SIGABRT/SIGKILL) are reclassified as
// MemoryExceeded in run_single after reading stderr.
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn wait_with_limits(
    child: &mut Child,
    timeout: Duration,
    _memory_limit_mb: u32,
) -> (Option<i32>, TerminationKind, Option<u64>) {
    match child.wait_timeout(timeout) {
        Ok(Some(status)) => {
            let (code, term) = classify_exit(status);
            (code, term, None)
        }
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
            (None, TerminationKind::WallTimeout, None)
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            (None, TerminationKind::Unknown, None)
        }
    }
}

/// Run all (variant, file) pairs in parallel using a local rayon thread pool.
pub fn run_batch(config: &OracleConfig, files: &[&Path]) -> Vec<RunOutcome> {
    // Build work items: (variant_index, file)
    let mut work: Vec<(usize, &Path)> = Vec::with_capacity(files.len() * config.variants.len());
    for file in files {
        for vi in 0..config.variants.len() {
            work.push((vi, file));
        }
    }

    // Shuffle for fairness (avoids all threads hitting the same file simultaneously)
    fastrand::shuffle(&mut work);

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(config.parallelism)
        .build()
        .expect("failed to build rayon thread pool");

    pool.install(|| {
        work.par_iter()
            .map(|&(vi, file)| run_single(config, vi, file))
            .collect()
    })
}

fn utc_now_rfc3339() -> String {
    // Simple UTC timestamp without pulling in a datetime crate.
    // Uses the system command on unix; falls back to a basic representation.
    #[cfg(unix)]
    {
        if let Ok(output) = std::process::Command::new("date")
            .arg("-u")
            .arg("+%Y-%m-%dT%H:%M:%SZ")
            .output()
        {
            if output.status.success() {
                return String::from_utf8_lossy(&output.stdout).trim().to_string();
            }
        }
    }

    // Fallback: use std::time
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s-since-epoch", dur.as_secs())
}
