use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Invoke icemaker with `--codegen-splice-omni` in the seeds directory.
///
/// Icemaker creates a new `icemaker_omni_*` directory inside `seeds_dir`.
/// We detect it by diffing directory entries before/after invocation,
/// then move the new directory to `dest`.
pub fn run_icemaker(
    icemaker_bin: &Path,
    seeds_dir: &Path,
    threads: u32,
    dest: &Path,
) -> Result<PathBuf, String> {
    let before: HashSet<_> = list_dirs(seeds_dir);

    let status = Command::new(icemaker_bin)
        .arg("--codegen-splice-omni")
        .arg("--threads")
        .arg(threads.to_string())
        .current_dir(seeds_dir)
        .status()
        .map_err(|e| format!("failed to spawn icemaker: {e}"))?;

    if !status.success() {
        return Err(format!("icemaker exited with status {status}"));
    }

    let after: HashSet<_> = list_dirs(seeds_dir);
    let new_dirs: Vec<_> = after.difference(&before).collect();

    let new_dir = match new_dirs.len() {
        0 => return Err("icemaker did not create any new directory".to_string()),
        1 => new_dirs[0].clone(),
        _ => {
            // Pick the one matching the icemaker_omni_ prefix if possible.
            new_dirs
                .iter()
                .find(|d| {
                    d.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("icemaker_omni_"))
                })
                .cloned()
                .cloned()
                .unwrap_or_else(|| new_dirs[0].clone())
        }
    };

    std::fs::rename(&new_dir, dest)
        .map_err(|e| format!("failed to move {} -> {}: {e}", new_dir.display(), dest.display()))?;

    Ok(dest.to_path_buf())
}

/// Invoke genie-251215 to generate mutants.
pub fn run_genie(
    genie_bin: &Path,
    seeds_dir: &Path,
    ingredients_dir: &Path,
    dest: &Path,
    ablation_profile: Option<&str>,
    no_placeholder_adaptation: bool,
    no_dependency_injection: bool,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dest)
        .map_err(|e| format!("failed to create output dir {}: {e}", dest.display()))?;

    let mut cmd = Command::new(genie_bin);
    cmd.arg("--seeds-dir")
        .arg(seeds_dir)
        .arg("--ingredients-dir")
        .arg(ingredients_dir)
        .arg("--output-dir")
        .arg(dest);

    if let Some(profile) = ablation_profile {
        cmd.arg("--ablation-profile").arg(profile);
    }
    if no_placeholder_adaptation {
        cmd.arg("--no-placeholder-adaptation");
    }
    if no_dependency_injection {
        cmd.arg("--no-dependency-injection");
    }

    let status = cmd
        .status()
        .map_err(|e| format!("failed to spawn genie: {e}"))?;

    if !status.success() {
        return Err(format!("genie exited with status {status}"));
    }

    Ok(dest.to_path_buf())
}

/// List immediate subdirectories of `dir`.
fn list_dirs(dir: &Path) -> HashSet<PathBuf> {
    let mut set = HashSet::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                set.insert(path);
            }
        }
    }
    set
}
