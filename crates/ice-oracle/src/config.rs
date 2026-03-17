use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Resource limits applied to each rustc invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub timeout_secs: u32,
    pub memory_limit_mb: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            timeout_secs: 10,
            memory_limit_mb: 1024,
        }
    }
}

/// A set of rustc flags to try, with a human-readable label.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileVariant {
    pub rustc_flags: Vec<String>,
    pub label: String,
}

/// Top-level configuration for the oracle.
#[derive(Debug, Clone)]
pub struct OracleConfig {
    pub rustc_path: PathBuf,
    pub resource_limits: ResourceLimits,
    pub variants: Vec<CompileVariant>,
    pub parallelism: usize,
    /// Extra arguments appended to every rustc invocation (after all other flags).
    pub extra_args: Vec<String>,
    /// If false (default), ICE locations that do not contain "compiler" are discarded.
    pub allow_non_compiler_locations: bool,
}

impl OracleConfig {
    pub fn new(rustc_path: PathBuf) -> Self {
        Self {
            rustc_path,
            resource_limits: ResourceLimits::default(),
            variants: crate::command::default_variants(),
            parallelism: 8,
            extra_args: Vec::new(),
            allow_non_compiler_locations: false,
        }
    }
}
