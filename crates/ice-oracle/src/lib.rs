pub mod command;
pub mod config;
pub mod dedup;
pub mod executor;
pub mod parser;
pub mod report;
pub mod result;

use std::path::Path;

use config::OracleConfig;
use dedup::deduplicate;
use result::{OracleReport, RunOutcome};

/// Run the full oracle pipeline: compile all files with all variants, collect and deduplicate ICEs.
pub fn run_oracle(config: &OracleConfig, files: &[&Path]) -> OracleReport {
    let all_outcomes = executor::run_batch(config, files);
    build_report(config, files.len(), all_outcomes)
}

/// Check a single file against all variants. Returns outcomes for that file.
pub fn check_file(config: &OracleConfig, file: &Path) -> Vec<RunOutcome> {
    let mut outcomes = Vec::new();
    for vi in 0..config.variants.len() {
        outcomes.push(executor::run_single(config, vi, file));
    }
    outcomes
}

/// Normalize an ICE location: trim any prefix before "compiler/" so it starts with "compiler/".
/// Returns `None` if the location does not contain "compiler".
fn normalize_location(location: &str) -> Option<String> {
    if let Some(pos) = location.find("compiler") {
        Some(location[pos..].to_string())
    } else {
        None
    }
}

/// Normalize and optionally filter ICE locations in a `RunOutcome`.
/// Updates `ices` and `is_ice` in place.
fn normalize_outcome(outcome: &mut RunOutcome, allow_non_compiler: bool) {
    for ice in &mut outcome.ices {
        if let Some(normalized) = normalize_location(&ice.location) {
            ice.location = normalized;
        }
        // If no "compiler" found, location stays as-is (will be filtered below if needed)
    }

    if !allow_non_compiler {
        outcome.ices.retain(|ice| ice.location.starts_with("compiler"));
        outcome.is_ice = !outcome.ices.is_empty();
    }
}

/// Build an `OracleReport` from collected outcomes.
fn build_report(
    config: &OracleConfig,
    total_files: usize,
    mut all_outcomes: Vec<RunOutcome>,
) -> OracleReport {
    let rustc_version = get_rustc_version(&config.rustc_path);

    // Normalize and filter ICE locations.
    for outcome in &mut all_outcomes {
        normalize_outcome(outcome, config.allow_non_compiler_locations);
    }

    let ice_outcomes: Vec<RunOutcome> = all_outcomes
        .iter()
        .filter(|o| o.is_ice)
        .cloned()
        .collect();

    let all_ices: Vec<_> = ice_outcomes.iter().flat_map(|o| o.ices.clone()).collect();
    let unique_ices = deduplicate(&all_ices);

    OracleReport {
        rustc_path: config.rustc_path.display().to_string(),
        rustc_version,
        resource_limits: config.resource_limits.clone(),
        total_files,
        total_runs: all_outcomes.len(),
        ice_outcomes,
        unique_ices,
        all_outcomes,
    }
}

fn get_rustc_version(rustc_path: &Path) -> String {
    std::process::Command::new(rustc_path)
        .arg("-Vv")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_else(|| "<unknown>".to_string())
}
