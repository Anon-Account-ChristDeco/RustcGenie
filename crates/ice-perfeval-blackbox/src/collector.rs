use std::collections::HashSet;
use std::path::{Path, PathBuf};

use ice_oracle::result::OracleReport;

/// Persistent state for tracking unique ICE locations across iterations.
pub struct Collector {
    pub seen_locations: HashSet<String>,
    output_dir: PathBuf,
}

impl Collector {
    /// Create a new collector rooted at `output_dir`.
    pub fn new(output_dir: &Path) -> Self {
        Self {
            seen_locations: HashSet::new(),
            output_dir: output_dir.to_path_buf(),
        }
    }

    /// Load previously seen locations from `seen_locations.json`.
    pub fn load(output_dir: &Path) -> Result<Self, String> {
        let path = output_dir.join("seen_locations.json");
        let data = std::fs::read_to_string(&path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        let seen: HashSet<String> =
            serde_json::from_str(&data).map_err(|e| format!("failed to parse {}: {e}", path.display()))?;
        Ok(Self {
            seen_locations: seen,
            output_dir: output_dir.to_path_buf(),
        })
    }

    /// Process the oracle report for one iteration:
    /// - Copy ICE files to `all_ices/`
    /// - Copy files with new ICE locations to `unique_ices/` (iter-prefixed)
    /// - Return stats: total ICE files and new location strings
    pub fn process_report(
        &mut self,
        report: &OracleReport,
        iteration: u32,
    ) -> Result<IterStats, String> {
        let all_ices_dir = self.output_dir.join("all_ices");
        let unique_ices_dir = self.output_dir.join("unique_ices");
        std::fs::create_dir_all(&all_ices_dir)
            .map_err(|e| format!("failed to create all_ices dir: {e}"))?;
        std::fs::create_dir_all(&unique_ices_dir)
            .map_err(|e| format!("failed to create unique_ices dir: {e}"))?;

        let mut total_ice_files = 0u32;
        let mut new_locations: Vec<String> = Vec::new();

        // Group ICE outcomes by source file.
        // A file may have multiple outcomes (one per variant), each possibly with ICEs.
        let mut seen_files: HashSet<PathBuf> = HashSet::new();

        for outcome in &report.ice_outcomes {
            if !seen_files.insert(outcome.file.clone()) {
                // Already processed this file (different variant).
                continue;
            }

            total_ice_files += 1;

            // Collect all unique ICE locations from ALL outcomes for this file.
            let mut seen_locs: HashSet<&str> = HashSet::new();
            let file_locations: Vec<&str> = report
                .ice_outcomes
                .iter()
                .filter(|o| o.file == outcome.file)
                .flat_map(|o| o.ices.iter().map(|ice| ice.location.as_str()))
                .filter(|loc| seen_locs.insert(loc))
                .collect();

            // Copy to all_ices/.
            let file_name = outcome
                .file
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            let all_dest = all_ices_dir.join(format!("iter{iteration}_{file_name}"));
            if outcome.file.exists() {
                let _ = std::fs::copy(&outcome.file, &all_dest);
            }

            // Check which locations are new.
            let new_locs: Vec<&str> = file_locations
                .iter()
                .filter(|loc| !self.seen_locations.contains(**loc))
                .copied()
                .collect();

            if !new_locs.is_empty() {
                let unique_dest = unique_ices_dir.join(format!("iter{iteration}_{file_name}"));
                if outcome.file.exists() {
                    let _ = std::fs::copy(&outcome.file, &unique_dest);
                }
                // Record new locations.
                for loc in &new_locs {
                    new_locations.push((*loc).to_string());
                }
                // Add ALL locations from this file to seen set.
                for loc in &file_locations {
                    self.seen_locations.insert((*loc).to_string());
                }
            }
        }

        Ok(IterStats {
            total_ice_files,
            new_locations,
        })
    }

    /// Delete non-ICE `.rs` files from the mutant directory.
    pub fn cleanup_non_ice_files(report: &OracleReport, mutant_dir: &Path) {
        let ice_files: HashSet<&Path> = report
            .ice_outcomes
            .iter()
            .map(|o| o.file.as_path())
            .collect();

        for outcome in &report.all_outcomes {
            if !ice_files.contains(outcome.file.as_path()) && outcome.file.exists() {
                let _ = std::fs::remove_file(&outcome.file);
            }
        }

        // Also remove any .rs files in the mutant dir that weren't in the report at all
        // (shouldn't happen normally, but be thorough).
        if let Ok(entries) = std::fs::read_dir(mutant_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "rs")
                    && !ice_files.contains(path.as_path())
                {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }

    /// Persist `seen_locations` to disk for crash recovery.
    pub fn save(&self) -> Result<(), String> {
        let path = self.output_dir.join("seen_locations.json");
        let json = serde_json::to_string_pretty(&self.seen_locations)
            .map_err(|e| format!("failed to serialize seen_locations: {e}"))?;
        std::fs::write(&path, json)
            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
        Ok(())
    }

    pub fn total_unique_locations(&self) -> usize {
        self.seen_locations.len()
    }
}

pub struct IterStats {
    pub total_ice_files: u32,
    pub new_locations: Vec<String>,
}
