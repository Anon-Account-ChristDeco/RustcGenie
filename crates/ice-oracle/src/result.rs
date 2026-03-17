use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::ResourceLimits;

/// A single ICE extracted from compiler output.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IceInfo {
    pub location: String,
    pub reason: String,
}

/// How the compiler process terminated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminationKind {
    /// Exited normally with an exit code.
    Normal,
    /// Killed by ice-oracle after the wall-clock timeout elapsed.
    WallTimeout,
    /// Killed by ice-oracle because RSS exceeded the configured memory limit.
    MemoryExceeded,
    /// Killed by a Unix signal. The inner value is the signal number
    /// (e.g. 9 = SIGKILL, 24 = SIGXCPU).
    Signal(i32),
    /// Failed to spawn the process at all.
    SpawnFailed,
    /// Could not determine the reason (e.g. wait error on non-Unix).
    Unknown,
}

impl TerminationKind {
    /// Short human-readable label.
    pub fn label(&self) -> String {
        match self {
            Self::Normal => "normal".to_string(),
            Self::WallTimeout => "WALL_TIMEOUT".to_string(),
            Self::MemoryExceeded => "MEMORY_EXCEEDED".to_string(),
            Self::Signal(sig) => format!("{} ({})", signal_name(*sig), sig),
            Self::SpawnFailed => "SPAWN_FAILED".to_string(),
            Self::Unknown => "UNKNOWN".to_string(),
        }
    }

    /// Hint about what likely caused this termination.
    pub fn hint(&self) -> &'static str {
        match self {
            Self::Normal => "",
            Self::WallTimeout => "compilation exceeded wall-clock timeout",
            Self::MemoryExceeded => "RSS exceeded configured --memory limit",
            Self::Signal(24) => "CPU time limit exceeded (RLIMIT_CPU)",
            Self::Signal(9) => "killed — possibly OOM (RLIMIT_AS) or external kill",
            Self::Signal(6) => "aborted — possibly OOM or assertion failure",
            Self::Signal(11) => "segmentation fault",
            Self::Signal(_) => "",
            Self::SpawnFailed => "could not start the compiler process",
            Self::Unknown => "",
        }
    }
}

/// Map well-known Unix signal numbers to names.
fn signal_name(sig: i32) -> &'static str {
    match sig {
        1 => "SIGHUP",
        2 => "SIGINT",
        3 => "SIGQUIT",
        4 => "SIGILL",
        6 => "SIGABRT",
        8 => "SIGFPE",
        9 => "SIGKILL",
        11 => "SIGSEGV",
        13 => "SIGPIPE",
        14 => "SIGALRM",
        15 => "SIGTERM",
        24 => "SIGXCPU",
        25 => "SIGXFSZ",
        _ => "SIG?",
    }
}

/// Returns true if the stderr text contains hints of an out-of-memory condition.
pub fn stderr_hints_oom(stderr: &str) -> bool {
    let s = stderr.to_ascii_lowercase();
    s.contains("memory allocation of")
        || s.contains("out of memory")
        || s.contains("alloc::oom")
        || s.contains("cannot allocate memory")
}

/// The outcome of one (file, variant) compilation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOutcome {
    pub file: PathBuf,
    pub variant_label: String,
    pub command_display: String,
    pub exit_status: Option<i32>,
    pub termination: TerminationKind,
    /// Peak RSS observed during execution, in MB. `None` if monitoring was unavailable.
    pub peak_rss_mb: Option<u64>,
    pub is_ice: bool,
    pub ices: Vec<IceInfo>,
    pub stderr: String,
    pub timestamp_utc: String,
}

/// Aggregated report across all files and variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleReport {
    pub rustc_path: String,
    pub rustc_version: String,
    pub resource_limits: ResourceLimits,
    pub total_files: usize,
    pub total_runs: usize,
    pub ice_outcomes: Vec<RunOutcome>,
    pub unique_ices: Vec<IceInfo>,
    pub all_outcomes: Vec<RunOutcome>,
}
