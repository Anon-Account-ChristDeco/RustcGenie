use std::path::Path;
use std::process::Command;

use crate::config::{CompileVariant, OracleConfig, ResourceLimits};

/// Returns the default compile variants (matching the original oracle's COMMON_OPTIONS).
pub fn default_variants() -> Vec<CompileVariant> {
    vec![CompileVariant {
        rustc_flags: vec!["--emit=mir".to_string()],
        // rustc_flags: vec!["--emit=mir".to_string(), "--edition=2021".to_string()],
        label: "emit-mir".to_string(),
    }]
}

/// Determine whether the source file defines `fn main` (bin crate) or not (lib crate).
fn needs_crate_type_lib(source: &str) -> bool {
    !source.contains("fn main")
}

/// Build a `Command` for a single (variant, file) pair.
///
/// On Linux, wraps with `prlimit` for memory/CPU limits.
/// On macOS, uses `pre_exec` with `setrlimit`.
/// Wall-clock timeout is handled by the caller using `wait-timeout`.
pub fn build_command(
    config: &OracleConfig,
    variant: &CompileVariant,
    file: &Path,
) -> Command {
    let source = std::fs::read_to_string(file).unwrap_or_default();

    let mut cmd = base_command(config);

    // Variant flags (e.g. --emit=mir)
    for flag in &variant.rustc_flags {
        cmd.arg(flag);
    }

    // Automatically add --crate-type=lib when no fn main
    if needs_crate_type_lib(&source) {
        cmd.arg("--crate-type=lib");
    }

    cmd.arg(file);

    // Extra args from options file (appended last)
    for arg in &config.extra_args {
        cmd.arg(arg);
    }

    // Suppress rustc ICE report file generation
    cmd.env("RUSTC_ICE", "0");

    cmd
}

/// On Linux, wrap with `prlimit` to set RLIMIT_AS and RLIMIT_CPU.
/// `prlimit` calls `execvp`, so the process replaces itself with rustc
/// and signals are reported normally.
#[cfg(target_os = "linux")]
fn base_command(config: &OracleConfig) -> Command {
    let memory_bytes = (config.resource_limits.memory_limit_mb as u64) * 1024 * 1024;
    let cpu_secs = config.resource_limits.timeout_secs;

    let mut cmd = Command::new("prlimit");
    cmd.arg(format!("--as={memory_bytes}"));
    cmd.arg(format!("--cpu={cpu_secs}"));
    cmd.arg(&config.rustc_path);
    cmd
}

/// On macOS, spawn rustc directly and apply limits via pre_exec.
#[cfg(target_os = "macos")]
fn base_command(config: &OracleConfig) -> Command {
    let mut cmd = Command::new(&config.rustc_path);
    apply_macos_limits(&mut cmd, &config.resource_limits);
    cmd
}

/// Format a `Command` for display in reports.
pub fn display_command(cmd: &Command) -> String {
    format!("{:?}", cmd)
}

/// Apply resource limits on macOS via pre_exec (RLIMIT_DATA + RLIMIT_CPU).
/// RLIMIT_AS is not enforced on Apple Silicon, so macOS also uses RSS polling
/// in the executor.
#[cfg(target_os = "macos")]
fn apply_macos_limits(cmd: &mut Command, limits: &ResourceLimits) {
    use std::os::unix::process::CommandExt;

    let memory_bytes = (limits.memory_limit_mb as u64) * 1024 * 1024;
    let cpu_secs = limits.timeout_secs as u64;

    // SAFETY: setrlimit is async-signal-safe and called before exec.
    unsafe {
        cmd.pre_exec(move || {
            let data_limit = libc::rlimit {
                rlim_cur: memory_bytes,
                rlim_max: memory_bytes,
            };
            // RLIMIT_DATA is a best-effort limit on macOS
            let _ = libc::setrlimit(libc::RLIMIT_DATA, &data_limit);

            let cpu_limit = libc::rlimit {
                rlim_cur: cpu_secs,
                rlim_max: cpu_secs,
            };
            if libc::setrlimit(libc::RLIMIT_CPU, &cpu_limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }

            Ok(())
        });
    }
}
