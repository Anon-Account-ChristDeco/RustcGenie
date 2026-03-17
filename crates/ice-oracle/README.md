# ice-oracle

Detects rustc Internal Compiler Errors (ICEs) by compiling Rust source files and parsing stderr for known ICE patterns.

## Library usage

```rust
use std::path::{Path, PathBuf};
use ice_oracle::config::OracleConfig;

let config = OracleConfig::new(PathBuf::from("/path/to/rustc"));
let outcomes = ice_oracle::check_file(&config, Path::new("test.rs"));
for outcome in &outcomes {
    if outcome.is_ice {
        println!("ICE found in {:?}", outcome.file);
    }
}
```

## CLI

```
ice-oracle (--compiler <path> | --nightly-date <YYYY-MM-DD>)
           [--install-toolchain]
           [--file <path> | --dir <path>] [--all-files]
           [--format text|json] [--log <path>]
           [--threads 8] [--timeout 10] [--memory 1024]
           [--verbose] [--debug]
```

Pass `--all-files` to include all files when walking `--dir`, not just `.rs` files.

Either `--compiler` (explicit rustc path) or `--nightly-date` (e.g. `2026-01-20`) must be given. With `--nightly-date`, the rustc binary is resolved from `~/.rustup/toolchains/nightly-{date}-{triple}/bin/rustc`. Pass `--install-toolchain` to allow automatic installation via `rustup toolchain install` if the toolchain is missing.

## Utility binaries

- `parse_ice_message` — reads rustc stderr from stdin and prints extracted ICE info.
- `print_options` — prints the commands that would be generated for a given file.

## Debug mode

Pass `--debug` to append a resource-limit diagnostic section to the text report. It shows:

- Counts of each termination kind (normal, wall-clock timeout, signal-killed, spawn failure).
- Per-signal breakdown (e.g. SIGXCPU for CPU limit, SIGKILL for possible OOM).
- OOM hints detected in stderr.
- A list of every abnormally-terminated run with the file, variant, termination kind, and hints.

Each per-run line in the main report also gets a `[WALL_TIMEOUT]`, `[SIGXCPU (24)]`, etc. tag.

In JSON format (`--format json`), the `termination` field is always present on every outcome regardless of `--debug`.

There is a manual test that verifies debug mode correctly distinguishes `MEMORY_EXCEEDED` from `WALL_TIMEOUT` using [rust-lang/rust#150061](https://github.com/rust-lang/rust/issues/150061) as a fixture (exponential memory growth during const-evaluation). It requires a nightly toolchain:

```bash
cargo test -p ice-oracle --test debug_limits -- --ignored

# or run the script directly
cd crates/ice-oracle && bash tests/test_debug_limits.sh

# with a different nightly
NIGHTLY_DATE=2026-02-01 cargo test -p ice-oracle --test debug_limits -- --ignored
```

## Cross-platform resource limiting

- **Linux:** memory (`RLIMIT_AS`) and CPU (`RLIMIT_CPU`) limits are enforced by the kernel via `setrlimit` in a `pre_exec` hook.
- **macOS:** `RLIMIT_DATA` is not enforced on Apple Silicon, so memory is monitored by polling RSS via `proc_pidinfo` and killing the process when it exceeds the limit. CPU limits use `RLIMIT_CPU`.
- **All platforms:** wall-clock timeout uses the `wait-timeout` crate.
