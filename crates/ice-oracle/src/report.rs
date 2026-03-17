use std::collections::HashMap;

use crate::result::{stderr_hints_oom, OracleReport, TerminationKind};

/// Serialize the report as JSON.
pub fn to_json(report: &OracleReport) -> String {
    serde_json::to_string_pretty(report).expect("failed to serialize report")
}

/// Format the report as human-readable text matching the original oracle output.
pub fn to_text(report: &OracleReport, verbose: bool, debug: bool) -> String {
    to_text_opts(report, verbose, debug, true)
}

/// Format the report as human-readable text.
///
/// `show_abnormal_list` controls whether individual abnormal termination entries
/// are listed in the debug summary. Statistics are always shown when `debug` is true.
pub fn to_text_opts(report: &OracleReport, verbose: bool, debug: bool, show_abnormal_list: bool) -> String {
    let mut out = String::new();

    out.push_str("Rustc Blackbox Oracle Report\n\n");
    out.push_str(&format!("Compiler: {}\n", report.rustc_path));
    out.push_str(&format!(
        "Configured Timeout (secs): {}, Memory Limit (MB): {}\n",
        report.resource_limits.timeout_secs, report.resource_limits.memory_limit_mb,
    ));
    out.push_str(&format!(
        "\nVersion Info:\n{}\n",
        report.rustc_version
    ));
    out.push_str(&format!(
        "Number of input Rust files: {}\n",
        report.total_files
    ));
    out.push_str(&format!("Total runs: {}\n", report.total_runs));
    out.push_str("----------------\n");

    for outcome in &report.all_outcomes {
        if outcome.is_ice {
            out.push_str("ICE");
            if debug {
                out.push_str(&format!(" [{}]", outcome.termination.label()));
            }
            out.push('\n');
            out.push_str(&format!("  {}\n", outcome.file.display()));
            out.push_str(&format!("  {}\n", outcome.timestamp_utc));
            out.push_str(&format!("  {}\n", outcome.command_display));
            for ice in &outcome.ices {
                out.push_str(&format!("  {}\n", ice.location));
                out.push_str(&format!("  {}\n", ice.reason));
            }
            if debug && stderr_hints_oom(&outcome.stderr) {
                out.push_str("  (stderr suggests OOM)\n");
            }
            out.push('\n');
        } else if verbose {
            out.push_str("NO_ICE");
            if debug {
                out.push_str(&format!(" [{}]", outcome.termination.label()));
            }
            out.push('\n');
            out.push_str(&format!("  {}\n", outcome.file.display()));
            out.push_str(&format!("  {}\n", outcome.timestamp_utc));
            out.push_str(&format!("  {}\n", outcome.command_display));
            if debug && stderr_hints_oom(&outcome.stderr) {
                out.push_str("  (stderr suggests OOM)\n");
            }
            out.push('\n');
        }
    }

    if !report.unique_ices.is_empty() {
        out.push_str("----------------\n");
        out.push_str(&format!("Unique ICEs: {}\n", report.unique_ices.len()));
        for ice in &report.unique_ices {
            out.push_str(&format!("  {} — {}\n", ice.location, ice.reason));
        }
    }

    if debug {
        out.push_str(&format_debug_summary(report, show_abnormal_list));
    }

    out
}

/// Build the debug summary section showing termination-kind statistics.
fn format_debug_summary(report: &OracleReport, show_abnormal_list: bool) -> String {
    let mut out = String::new();

    out.push_str("\n================\n");
    out.push_str("Debug: Resource Limit Summary\n");
    out.push_str("================\n\n");

    // Count by termination kind
    let mut normal = 0usize;
    let mut wall_timeout = 0usize;
    let mut memory_exceeded = 0usize;
    let mut spawn_failed = 0usize;
    let mut unknown = 0usize;
    let mut signals: HashMap<i32, usize> = HashMap::new();
    let mut oom_hint_count = 0usize;

    for o in &report.all_outcomes {
        match &o.termination {
            TerminationKind::Normal => normal += 1,
            TerminationKind::WallTimeout => wall_timeout += 1,
            TerminationKind::MemoryExceeded => memory_exceeded += 1,
            TerminationKind::Signal(sig) => *signals.entry(*sig).or_default() += 1,
            TerminationKind::SpawnFailed => spawn_failed += 1,
            TerminationKind::Unknown => unknown += 1,
        }
        if stderr_hints_oom(&o.stderr) {
            oom_hint_count += 1;
        }
    }

    let signal_total: usize = signals.values().sum();

    out.push_str(&format!("  Total runs:         {}\n", report.all_outcomes.len()));
    out.push_str(&format!("  Completed normally: {normal}\n"));
    out.push_str(&format!("  Wall-clock timeout: {wall_timeout}\n"));
    out.push_str(&format!("  Memory exceeded:    {memory_exceeded}\n"));
    out.push_str(&format!("  Killed by signal:   {signal_total}\n"));
    if !signals.is_empty() {
        let mut sigs: Vec<_> = signals.into_iter().collect();
        sigs.sort_by_key(|(sig, _)| *sig);
        for (sig, count) in &sigs {
            let tk = TerminationKind::Signal(*sig);
            out.push_str(&format!("    {}: {count}\n", tk.label()));
        }
    }
    if spawn_failed > 0 {
        out.push_str(&format!("  Spawn failed:       {spawn_failed}\n"));
    }
    if unknown > 0 {
        out.push_str(&format!("  Unknown:            {unknown}\n"));
    }
    if oom_hint_count > 0 {
        out.push_str(&format!("  Stderr hints OOM:   {oom_hint_count}\n"));
    }

    // List non-normal runs with details
    let abnormal: Vec<_> = report
        .all_outcomes
        .iter()
        .filter(|o| o.termination != TerminationKind::Normal)
        .collect();

    if show_abnormal_list && !abnormal.is_empty() {
        out.push_str("\n  Abnormal terminations:\n");
        for o in &abnormal {
            let ice_tag = if o.is_ice { " ICE" } else { "" };
            let oom_tag = if stderr_hints_oom(&o.stderr) {
                " (stderr suggests OOM)"
            } else {
                ""
            };
            let hint = o.termination.hint();
            let hint_str = if hint.is_empty() {
                String::new()
            } else {
                format!(" — {hint}")
            };
            let rss_str = match o.peak_rss_mb {
                Some(mb) => format!(" peak_rss={mb}MB"),
                None => String::new(),
            };
            out.push_str(&format!(
                "    {} [{}] [{}]{}{}{}{}\n",
                o.file.display(),
                o.variant_label,
                o.termination.label(),
                hint_str,
                rss_str,
                ice_tag,
                oom_tag,
            ));
        }
    }

    out
}
