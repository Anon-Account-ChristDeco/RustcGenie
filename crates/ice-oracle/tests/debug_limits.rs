//! Runs `test_debug_limits.sh` to verify that `--debug` correctly reports
//! MEMORY_EXCEEDED vs WALL_TIMEOUT.
//!
//! Ignored by default because it needs a nightly toolchain installed and
//! takes ~15 s.  Run with:
//!
//!     cargo test -p ice-oracle --test debug_limits -- --ignored
//!
//! Override the nightly date:
//!
//!     NIGHTLY_DATE=2026-02-01 cargo test -p ice-oracle --test debug_limits -- --ignored

use std::process::Command;

#[test]
#[ignore]
fn debug_limits() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let script = format!("{manifest_dir}/tests/test_debug_limits.sh");

    let output = Command::new("bash")
        .arg(&script)
        .output()
        .expect("failed to run test_debug_limits.sh");

    // Print stdout/stderr so `cargo test` shows the details on failure.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.is_empty() {
        println!("{stdout}");
    }
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }

    assert!(output.status.success(), "test_debug_limits.sh failed");
}
