use std::fs;
use std::path::PathBuf;

use clap::Parser;
use walkdir::WalkDir;

use ice_oracle::config::OracleConfig;
use ice_oracle::report;

#[derive(Parser)]
#[command(name = "ice-oracle", about = "Detect rustc Internal Compiler Errors")]
struct Cli {
    /// Path to the Rust compiler executable.
    #[arg(short, long, conflicts_with = "nightly_date")]
    compiler: Option<String>,

    /// Use a nightly toolchain by date (YYYY-MM-DD), e.g. 2026-01-20.
    /// Resolves the rustc binary from the installed rustup toolchain.
    /// Note: the toolchain date is typically one day after the rustc commit date
    /// shown by `rustc -Vv`.
    #[arg(short, long, conflicts_with = "compiler")]
    nightly_date: Option<String>,

    /// Allow installing the nightly toolchain via `rustup toolchain install`
    /// if it is not already present. Only used with --nightly-date.
    #[arg(long)]
    install_toolchain: bool,

    /// Path to a single Rust source file.
    #[arg(short, long)]
    file: Option<String>,

    /// Path to a directory of Rust source files.
    #[arg(short, long)]
    dir: Option<String>,

    /// Output format.
    #[arg(long, default_value = "text")]
    format: Format,

    /// Path to an output log file (default: stdout).
    #[arg(short, long)]
    log: Option<String>,

    /// Number of parallel threads.
    #[arg(short, long, default_value_t = 8)]
    threads: usize,

    /// Timeout in seconds per compilation.
    #[arg(long, default_value_t = 10)]
    timeout: u32,

    /// Memory limit in MB per compilation.
    #[arg(long, default_value_t = 1024)]
    memory: u32,

    /// File with extra compile options, one per line (appended to rustc command).
    #[arg(long)]
    options_file: Option<String>,

    /// Include all files when walking --dir, not just .rs files.
    #[arg(long)]
    all_files: bool,

    /// Also report non-ICE results.
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    /// Show resource-limit diagnostics (timeout, signal, OOM detection).
    #[arg(long, default_value_t = false)]
    debug: bool,
}

#[derive(Clone, clap::ValueEnum)]
enum Format {
    Text,
    Json,
}

fn main() {
    let cli = Cli::parse();

    // Resolve compiler path
    let rustc = match (&cli.compiler, &cli.nightly_date) {
        (Some(path), None) => PathBuf::from(path).canonicalize().unwrap_or_else(|e| {
            eprintln!("Error: Compiler path does not exist: {e}");
            std::process::exit(1);
        }),
        (None, Some(date)) => resolve_nightly_rustc(date, cli.install_toolchain),
        (None, None) => {
            eprintln!("Error: Either --compiler or --nightly-date must be specified.");
            std::process::exit(1);
        }
        _ => unreachable!("clap conflicts_with prevents this"),
    };

    // Require at least --file or --dir
    if cli.file.is_none() && cli.dir.is_none() {
        eprintln!("Error: Either --file or --dir must be specified.");
        std::process::exit(2);
    }

    // Gather input files
    let files: Vec<PathBuf> = gather_files(&cli.file, &cli.dir, cli.all_files);
    if files.is_empty() {
        if cli.all_files {
            eprintln!("No files found.");
        } else {
            eprintln!("No .rs files found.");
        }
        std::process::exit(3);
    }

    // Load extra options from file
    let extra_args = if let Some(path) = &cli.options_file {
        parse_options_file(path)
    } else {
        Vec::new()
    };

    // Build config
    let mut config = OracleConfig::new(rustc);
    config.parallelism = cli.threads;
    config.resource_limits.timeout_secs = cli.timeout;
    config.resource_limits.memory_limit_mb = cli.memory;
    config.extra_args = extra_args;

    // Run oracle
    let file_refs: Vec<&std::path::Path> = files.iter().map(|p| p.as_path()).collect();
    let report = ice_oracle::run_oracle(&config, &file_refs);

    // Format output
    let output = match cli.format {
        Format::Text => report::to_text(&report, cli.verbose, cli.debug),
        Format::Json => report::to_json(&report),
    };

    // Write output
    if let Some(ref log_path) = cli.log {
        if let Some(parent) = std::path::Path::new(log_path).parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(log_path, &output).unwrap_or_else(|e| {
            eprintln!("Error writing log file: {e}");
            std::process::exit(4);
        });
    } else {
        print!("{output}");
    }
}

/// Read an options file: one flag per line. Blank lines and lines starting with '#' are skipped.
/// Lines that look like a binary path (contain '/' or end with an executable-like name without
/// leading '-') are rejected — only flags/options are accepted.
fn parse_options_file(path: &str) -> Vec<String> {
    let contents = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Error reading options file: {e}");
        std::process::exit(7);
    });
    let mut args = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Reject lines that look like binary paths (e.g. "rustc", "/usr/bin/gcc")
        if trimmed.contains('/') || trimmed.contains('\\') {
            eprintln!("Error: options file contains a path-like entry (not an option): {trimmed}");
            std::process::exit(8);
        }
        if !trimmed.starts_with('-') {
            eprintln!("Error: options file entry does not look like a flag: {trimmed}");
            std::process::exit(8);
        }
        args.push(trimmed.to_string());
    }
    args
}

fn gather_files(file: &Option<String>, dir: &Option<String>, all_files: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if let Some(path) = file {
        let p = PathBuf::from(path).canonicalize().unwrap_or_else(|e| {
            eprintln!("Error: File path invalid: {e}");
            std::process::exit(5);
        });
        files.push(p);
    }

    if let Some(path) = dir {
        let d = PathBuf::from(path).canonicalize().unwrap_or_else(|e| {
            eprintln!("Error: Directory path invalid: {e}");
            std::process::exit(6);
        });
        for entry in WalkDir::new(d).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_dir() {
                continue;
            }
            if all_files || entry.path().extension().is_some_and(|ext| ext == "rs") {
                files.push(entry.path().to_path_buf());
            }
        }
    }

    files
}

/// Resolve the rustc binary path for a nightly toolchain date.
///
/// Looks for `~/.rustup/toolchains/nightly-{date}-{triple}/bin/rustc`.
/// The host triple is detected by running `rustc -vV` and parsing the `host:` line.
fn resolve_nightly_rustc(date: &str, allow_install: bool) -> PathBuf {
    // Validate date format (YYYY-MM-DD)
    if date.len() != 10 || date.as_bytes()[4] != b'-' || date.as_bytes()[7] != b'-' {
        eprintln!("Error: --nightly-date must be in YYYY-MM-DD format, got: {date}");
        std::process::exit(1);
    }

    let toolchain_name = format!("nightly-{date}");

    // Detect host triple from the default rustc
    let host_triple = detect_host_triple();

    // Resolve rustup home (~/.rustup by default)
    let rustup_home = std::env::var("RUSTUP_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| {
            eprintln!("Error: Cannot determine home directory (HOME not set).");
            std::process::exit(1);
        });
        format!("{home}/.rustup")
    });

    let toolchain_dir = PathBuf::from(&rustup_home)
        .join("toolchains")
        .join(format!("{toolchain_name}-{host_triple}"));

    let rustc_path = toolchain_dir.join("bin").join("rustc");

    if rustc_path.exists() {
        eprintln!("Using toolchain: {toolchain_name} ({rustc_path:?})");
        return rustc_path;
    }

    // Toolchain not found — try installing if allowed
    if !allow_install {
        eprintln!(
            "Error: Toolchain {toolchain_name} not found at {}",
            toolchain_dir.display()
        );
        eprintln!("Hint: Pass --install-toolchain to install it automatically.");
        std::process::exit(1);
    }

    eprintln!("Installing toolchain {toolchain_name} via rustup...");
    let status = std::process::Command::new("rustup")
        .args(["toolchain", "install", &toolchain_name, "--profile", "minimal"])
        .status()
        .unwrap_or_else(|e| {
            eprintln!("Error: Failed to run rustup: {e}");
            std::process::exit(1);
        });

    if !status.success() {
        eprintln!("Error: rustup toolchain install failed.");
        std::process::exit(1);
    }

    if !rustc_path.exists() {
        eprintln!(
            "Error: Toolchain installed but rustc not found at {}",
            rustc_path.display()
        );
        std::process::exit(1);
    }

    eprintln!("Using toolchain: {toolchain_name} ({rustc_path:?})");
    rustc_path
}

/// Detect the host triple by parsing `rustc -vV` output.
fn detect_host_triple() -> String {
    let output = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .unwrap_or_else(|e| {
            eprintln!("Error: Failed to run `rustc -vV` to detect host triple: {e}");
            eprintln!("Hint: Make sure rustc is installed and in your PATH.");
            std::process::exit(1);
        });

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(triple) = line.strip_prefix("host: ") {
            return triple.trim().to_string();
        }
    }

    eprintln!("Error: Could not detect host triple from `rustc -vV` output.");
    std::process::exit(1);
}
