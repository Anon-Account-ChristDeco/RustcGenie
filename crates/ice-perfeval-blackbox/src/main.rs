mod collector;
mod mutator;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use clap::{Parser, Subcommand};
use ice_oracle::config::{OracleConfig, ResourceLimits};
use time::OffsetDateTime;
use time::macros::format_description;
use walkdir::WalkDir;

use collector::Collector;

#[derive(Parser)]
#[command(name = "ice-perfeval-blackbox")]
#[command(about = "Performance evaluation driver for ICE-finding mutation approaches")]
struct Cli {
    #[command(subcommand)]
    command: MutatorCommand,
}

#[derive(Subcommand)]
enum MutatorCommand {
    /// Run evaluation using icemaker (--codegen-splice-omni)
    Icemaker {
        /// Path to the icemaker binary [default: vendor/icemaker/target/debug/icemaker relative to workspace root]
        #[arg(long)]
        icemaker_bin: Option<PathBuf>,

        #[command(flatten)]
        common: CommonArgs,
    },

    /// Run evaluation using genie-251215
    Genie {
        /// Path to the genie-251215 binary [default: sibling binary next to this executable]
        #[arg(long)]
        genie_bin: Option<PathBuf>,

        /// Directory with ingredient files for genie
        #[arg(long)]
        ingredients_dir: PathBuf,

        /// Named ablation profile
        #[arg(long)]
        ablation_profile: Option<String>,

        /// Disable placeholder adaptation (ablation) [legacy]
        #[arg(long, default_value_t = false)]
        no_placeholder_adaptation: bool,

        /// Disable dependency injection (ablation) [legacy]
        #[arg(long, default_value_t = false)]
        no_dependency_injection: bool,

        #[command(flatten)]
        common: CommonArgs,
    },
}

#[derive(Parser)]
struct CommonArgs {
    /// Path to the rustc binary to test against [default: `rustup which rustc`]
    #[arg(long)]
    rustc_path: Option<PathBuf>,

    /// Directory containing seed .rs files
    #[arg(long)]
    seeds_dir: PathBuf,

    /// Output directory for results
    #[arg(long)]
    output_dir: PathBuf,

    /// Time budget in seconds
    #[arg(long)]
    time_budget: u64,

    /// Per-file compilation timeout in seconds
    #[arg(long, default_value_t = 10)]
    timeout: u32,

    /// Per-file memory limit in MB
    #[arg(long, default_value_t = 1024)]
    memory: u32,

    /// Parallelism: controls oracle threads (rustc invocations) and is passed to
    /// icemaker's --threads (0 = all cores). Note: icemaker's --codegen-splice-omni
    /// mode currently ignores --threads.
    #[arg(long, default_value_t = 8)]
    threads: u32,

    /// Keep non-ICE files in mutant directories
    #[arg(long, default_value_t = false)]
    keep_non_ice: bool,
}

/// Resolve `--rustc-path` default via `rustup which rustc`.
fn resolve_rustc_path(provided: Option<PathBuf>) -> PathBuf {
    if let Some(p) = provided {
        return canonicalize(&p, "--rustc-path");
    }
    let output = Command::new("rustup")
        .args(["which", "rustc"])
        .output()
        .expect("failed to run `rustup which rustc` — provide --rustc-path explicitly");
    assert!(
        output.status.success(),
        "`rustup which rustc` failed — provide --rustc-path explicitly"
    );
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    eprintln!("using rustc: {}", path.display());
    path
}

/// Resolve a binary path by looking for a sibling next to the current executable.
fn resolve_sibling_bin(provided: Option<PathBuf>, name: &str) -> PathBuf {
    if let Some(p) = provided {
        return canonicalize(&p, &format!("--{}-bin", name.replace("_", "-")));
    }
    let exe = std::env::current_exe().expect("failed to determine current executable path");
    let sibling = exe.parent().expect("executable has no parent dir").join(name);
    assert!(
        sibling.exists(),
        "{} not found at {} — provide --{}-bin explicitly",
        name,
        sibling.display(),
        name.replace("_", "-"),
    );
    eprintln!("using {name}: {}", sibling.display());
    sibling
}

/// Resolve icemaker binary from vendor directory relative to workspace root.
/// Assumes the current executable is at `<workspace>/target/{debug,release}/ice-perfeval-blackbox`.
fn resolve_icemaker_bin(provided: Option<PathBuf>) -> PathBuf {
    if let Some(p) = provided {
        return canonicalize(&p, "--icemaker-bin");
    }
    let exe = std::env::current_exe().expect("failed to determine current executable path");
    // exe: <workspace>/target/<profile>/ice-perfeval-blackbox
    // workspace root: exe/../../..
    let workspace_root = exe
        .parent() // target/<profile>/
        .and_then(|p| p.parent()) // target/
        .and_then(|p| p.parent()) // <workspace>/
        .expect("could not determine workspace root from executable path");
    let profile = exe
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy();
    let icemaker = workspace_root
        .join("vendor/icemaker/target")
        .join(profile.as_ref())
        .join("icemaker");
    assert!(
        icemaker.exists(),
        "icemaker not found at {} — run ./vendor/setup.sh or provide --icemaker-bin explicitly",
        icemaker.display(),
    );
    eprintln!("using icemaker: {}", icemaker.display());
    icemaker
}

/// Canonicalize a path, with a clear error message on failure.
fn canonicalize(path: &Path, label: &str) -> PathBuf {
    std::fs::canonicalize(path)
        .unwrap_or_else(|e| panic!("{label} path does not exist: {}: {e}", path.display()))
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        MutatorCommand::Icemaker {
            icemaker_bin,
            mut common,
        } => {
            let icemaker_bin = resolve_icemaker_bin(icemaker_bin);
            let rustc_path = resolve_rustc_path(common.rustc_path.take());
            common.seeds_dir = canonicalize(&common.seeds_dir, "--seeds-dir");
            common.output_dir = canonicalize_or_create(&common.output_dir);
            let mutator_opts = format!(
                "icemaker_bin: {}",
                icemaker_bin.display(),
            );
            run_loop(
                &common,
                &rustc_path,
                |dest| {
                    mutator::run_icemaker(&icemaker_bin, &common.seeds_dir, common.threads, dest)
                },
                "icemaker",
                &mutator_opts,
            );
        }
        MutatorCommand::Genie {
            genie_bin,
            ingredients_dir,
            ablation_profile,
            no_placeholder_adaptation,
            no_dependency_injection,
            mut common,
        } => {
            let genie_bin = resolve_sibling_bin(genie_bin, "genie-251215");
            let rustc_path = resolve_rustc_path(common.rustc_path.take());
            common.seeds_dir = canonicalize(&common.seeds_dir, "--seeds-dir");
            let ingredients_dir = canonicalize(&ingredients_dir, "--ingredients-dir");
            common.output_dir = canonicalize_or_create(&common.output_dir);
            let mutator_opts = format!(
                "genie_bin: {}\ningredients_dir: {}\nablation_profile: {}\nno_placeholder_adaptation: {}\nno_dependency_injection: {}",
                genie_bin.display(),
                ingredients_dir.display(),
                ablation_profile.as_deref().unwrap_or("(none)"),
                no_placeholder_adaptation,
                no_dependency_injection,
            );
            run_loop(
                &common,
                &rustc_path,
                |dest| {
                    mutator::run_genie(
                        &genie_bin,
                        &common.seeds_dir,
                        &ingredients_dir,
                        dest,
                        ablation_profile.as_deref(),
                        no_placeholder_adaptation,
                        no_dependency_injection,
                    )
                },
                "genie",
                &mutator_opts,
            );
        }
    }
}

/// Ensure output directory exists, then canonicalize.
fn canonicalize_or_create(path: &Path) -> PathBuf {
    std::fs::create_dir_all(path)
        .unwrap_or_else(|e| panic!("failed to create output directory {}: {e}", path.display()));
    std::fs::canonicalize(path)
        .unwrap_or_else(|e| panic!("failed to canonicalize output directory {}: {e}", path.display()))
}

fn run_loop(
    common: &CommonArgs,
    rustc_path: &Path,
    mut invoke_mutator: impl FnMut(&Path) -> Result<PathBuf, String>,
    label: &str,
    mutator_opts: &str,
) {
    let output_dir = &common.output_dir;

    // --- Determine start state (fresh vs resume) ---
    let (mut collector, start_iteration) = init_collector(output_dir);

    // Create subdirectories.
    let reports_dir = output_dir.join("oracle_reports");
    let mutants_dir = output_dir.join("mutants");
    std::fs::create_dir_all(&reports_dir).expect("failed to create oracle_reports dir");
    std::fs::create_dir_all(&mutants_dir).expect("failed to create mutants dir");

    // Build oracle config.
    let mut oracle_config = OracleConfig::new(rustc_path.to_path_buf());
    oracle_config.resource_limits = ResourceLimits {
        timeout_secs: common.timeout,
        memory_limit_mb: common.memory,
    };
    oracle_config.parallelism = common.threads as usize;

    // Write config header to run.log.
    write_config_header(output_dir, rustc_path, label, common, mutator_opts);

    let budget = std::time::Duration::from_secs(common.time_budget);
    let global_start = Instant::now();
    let mut iteration = start_iteration;

    eprintln!(
        "[{label}] starting from iteration {iteration}, time budget: {}s, unique locations so far: {}",
        common.time_budget,
        collector.total_unique_locations(),
    );

    loop {
        if global_start.elapsed() >= budget {
            eprintln!("[{label}] time budget exhausted after iteration {}", iteration.saturating_sub(1));
            break;
        }

        let iter_start = Instant::now();
        eprintln!("[{label}] === iteration {iteration} ===");

        // 1. Invoke mutator.
        let mutant_dir = mutants_dir.join(format!("iter_{iteration}"));
        let mutation_start = Instant::now();
        let mutant_result = invoke_mutator(&mutant_dir);
        let mutation_secs = mutation_start.elapsed().as_secs_f64();

        let mutant_dir = match mutant_result {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[{label}] WARNING: mutator failed in iteration {iteration}: {e}");
                append_log(output_dir, iteration, iter_start.elapsed(), None, label);
                iteration += 1;
                continue;
            }
        };

        // 2. Collect .rs files from mutant dir.
        let rs_files = collect_rs_files(&mutant_dir);
        if rs_files.is_empty() {
            eprintln!("[{label}] iteration {iteration}: no .rs files produced");
            append_log(output_dir, iteration, iter_start.elapsed(), None, label);
            iteration += 1;
            continue;
        }

        let file_refs: Vec<&Path> = rs_files.iter().map(|p| p.as_path()).collect();

        // 3. Run oracle.
        let oracle_start = Instant::now();
        let report = ice_oracle::run_oracle(&oracle_config, &file_refs);
        let oracle_secs = oracle_start.elapsed().as_secs_f64();

        // 4. Write oracle report.
        let report_text = ice_oracle::report::to_text_opts(&report, false, true, common.keep_non_ice);
        let report_path = reports_dir.join(format!("iteration_{iteration}.txt"));
        if let Err(e) = std::fs::write(&report_path, &report_text) {
            eprintln!("[{label}] WARNING: failed to write report: {e}");
        }

        // 5. Process ICE results.
        let stats = match collector.process_report(&report, iteration) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[{label}] WARNING: collector failed in iteration {iteration}: {e}");
                iteration += 1;
                continue;
            }
        };

        // 6. Cleanup non-ICE files.
        if !common.keep_non_ice {
            Collector::cleanup_non_ice_files(&report, &mutant_dir);
        }

        // 7. Persist seen_locations.
        if let Err(e) = collector.save() {
            eprintln!("[{label}] WARNING: failed to persist seen_locations: {e}");
        }

        // 8. Append to run.log.
        let new_locations = stats.new_locations;
        let log_entry = LogEntry {
            total_ice_files: stats.total_ice_files,
            new_unique_locations: new_locations.len(),
            cumulative_unique_locations: collector.total_unique_locations(),
            total_rs_files: rs_files.len(),
            mutation_secs,
            oracle_secs,
            mutant_dir: mutant_dir.display().to_string(),
            new_locations,
        };
        append_log(
            output_dir,
            iteration,
            iter_start.elapsed(),
            Some(&log_entry),
            label,
        );

        eprintln!(
            "[{label}] iteration {iteration}: {} files, {} ICE files, {} new unique locations, {} cumulative locations (mutation={:.1}s oracle={:.1}s total={:.1}s)",
            rs_files.len(),
            stats.total_ice_files,
            log_entry.new_unique_locations,
            collector.total_unique_locations(),
            mutation_secs,
            oracle_secs,
            iter_start.elapsed().as_secs_f64(),
        );

        iteration += 1;
    }

    eprintln!(
        "[{label}] done. Total time: {:.1}s, iterations: {}, unique ICE locations: {}",
        global_start.elapsed().as_secs_f64(),
        iteration - start_iteration,
        collector.total_unique_locations(),
    );
}

/// Determine whether we're resuming or starting fresh. Returns (Collector, start_iteration).
fn init_collector(output_dir: &Path) -> (Collector, u32) {
    let seen_path = output_dir.join("seen_locations.json");

    if output_dir.exists() {
        if seen_path.exists() {
            // Resume.
            let collector = Collector::load(output_dir)
                .expect("failed to load seen_locations.json for resume");
            let start_iter = detect_next_iteration(output_dir);
            eprintln!(
                "resuming from iteration {start_iter} with {} known locations",
                collector.total_unique_locations()
            );
            return (collector, start_iter);
        }

        // Directory exists but no checkpoint — check if empty.
        let is_empty = std::fs::read_dir(output_dir)
            .map(|mut d| d.next().is_none())
            .unwrap_or(false);

        if !is_empty {
            eprintln!(
                "ERROR: output directory {} exists and is non-empty but has no seen_locations.json. \
                 Remove it or provide a different --output-dir.",
                output_dir.display()
            );
            std::process::exit(1);
        }
    }

    // Fresh start.
    std::fs::create_dir_all(output_dir).expect("failed to create output directory");
    (Collector::new(output_dir), 0)
}

/// Detect the next iteration number from existing oracle report filenames.
fn detect_next_iteration(output_dir: &Path) -> u32 {
    let reports_dir = output_dir.join("oracle_reports");
    if !reports_dir.exists() {
        return 0;
    }

    let mut max_iter: Option<u32> = None;
    if let Ok(entries) = std::fs::read_dir(&reports_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            // iteration_N.txt
            if let Some(rest) = name.strip_prefix("iteration_") {
                if let Some(num_str) = rest.strip_suffix(".txt") {
                    if let Ok(n) = num_str.parse::<u32>() {
                        max_iter = Some(max_iter.map_or(n, |m: u32| m.max(n)));
                    }
                }
            }
        }
    }

    max_iter.map_or(0, |n| n + 1)
}

/// Recursively collect all `.rs` files from a directory.
fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir).into_iter().flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "rs") {
            files.push(path.to_path_buf());
        }
    }
    files
}

struct LogEntry {
    total_rs_files: usize,
    total_ice_files: u32,
    new_unique_locations: usize,
    cumulative_unique_locations: usize,
    mutation_secs: f64,
    oracle_secs: f64,
    mutant_dir: String,
    new_locations: Vec<String>,
}

/// Write run configuration header to run.log (once at start).
fn write_config_header(output_dir: &Path, rustc_path: &Path, label: &str, common: &CommonArgs, mutator_opts: &str) {
    let log_path = output_dir.join("run.log");
    let timestamp = format_timestamp();

    let cli_args: Vec<String> = std::env::args().collect();
    let rustc_version = Command::new(rustc_path)
        .arg("-Vv")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|e| format!("failed to run rustc -Vv: {e}"));

    let header = format!(
        "\
=== run started at {timestamp} ===
mutator: {label}
cli: {cli_args}
seeds_dir: {seeds_dir}
output_dir: {output_dir_display}
rustc_path: {rustc_path_display}
timeout: {timeout}s
memory: {memory}MB
threads: {threads}
time_budget: {budget}s
keep_non_ice: {keep_non_ice}
{mutator_opts}

rustc -Vv:
{rustc_version}

",
        cli_args = cli_args.join(" "),
        seeds_dir = common.seeds_dir.display(),
        output_dir_display = output_dir.display(),
        rustc_path_display = rustc_path.display(),
        timeout = common.timeout,
        memory = common.memory,
        threads = common.threads,
        budget = common.time_budget,
        keep_non_ice = common.keep_non_ice,
    );

    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .expect("failed to open run.log");
    f.write_all(header.as_bytes())
        .expect("failed to write config header to run.log");
}

fn append_log(
    output_dir: &Path,
    iteration: u32,
    elapsed: std::time::Duration,
    stats: Option<&LogEntry>,
    label: &str,
) {
    let log_path = output_dir.join("run.log");
    let timestamp = format_timestamp();

    let mut buf = String::new();

    if let Some(s) = stats {
        use std::fmt::Write;
        writeln!(buf, "[{timestamp}] [{label}] iter={iteration} new_unique_locations={}", s.new_unique_locations).unwrap();
        writeln!(buf, "  time={:.1}s mutation={:.1}s oracle={:.1}s", elapsed.as_secs_f64(), s.mutation_secs, s.oracle_secs).unwrap();
        writeln!(buf, "  ices={} cumulative_locations={}", s.total_ice_files, s.cumulative_unique_locations).unwrap();
        writeln!(buf, "  mutants={} mutant_dir={}", s.total_rs_files, s.mutant_dir).unwrap();
        for loc in &s.new_locations {
            writeln!(buf, "  NEW_LOCATION: {loc}").unwrap();
        }
    } else {
        buf = format!(
            "[{timestamp}] [{label}] iter={iteration} SKIPPED (mutator failure)\n",
        );
    }

    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .expect("failed to open run.log");
    f.write_all(buf.as_bytes())
        .expect("failed to write to run.log");
}

fn format_timestamp() -> String {
    let now = OffsetDateTime::now_utc();
    let fmt = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
    now.format(&fmt).unwrap_or_else(|_| "unknown".to_string())
}
