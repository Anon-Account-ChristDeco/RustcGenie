use std::path::PathBuf;

use ice_oracle::command::{build_command, default_variants, display_command};
use ice_oracle::config::{OracleConfig, ResourceLimits};

/// Prints the commands that would be generated for a given file.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: print_options <rustc-path> <file.rs>");
        std::process::exit(1);
    }

    let config = OracleConfig {
        rustc_path: PathBuf::from(&args[1]),
        resource_limits: ResourceLimits::default(),
        variants: default_variants(),
        parallelism: 1,
        extra_args: Vec::new(),
        allow_non_compiler_locations: false,
    };

    let file = PathBuf::from(&args[2]);

    for variant in &config.variants {
        let cmd = build_command(&config, variant, &file);
        println!("[{}] {}", variant.label, display_command(&cmd));
    }
}
