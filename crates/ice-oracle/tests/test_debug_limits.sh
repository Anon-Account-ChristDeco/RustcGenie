#!/usr/bin/env bash
#
# Verify that --debug correctly distinguishes MEMORY_EXCEEDED from WALL_TIMEOUT.
#
# Uses tests/fixtures/150061.rs (rust-lang/rust#150061) which triggers
# exponential memory growth during const-evaluation.
#
# Requires a nightly toolchain (default: nightly-2026-01-20).
# Override: NIGHTLY_DATE=2026-02-01 bash tests/test_debug_limits.sh
#
# Run directly:
#   cd crates/ice-oracle && bash tests/test_debug_limits.sh
#
# Run via cargo:
#   cargo test -p ice-oracle --test debug_limits -- --ignored

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CRATE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
WORKSPACE="$(cd "$CRATE_DIR/../.." && pwd)"
FIXTURE="$SCRIPT_DIR/fixtures/150061.rs"
NIGHTLY_DATE="${NIGHTLY_DATE:-2026-01-20}"

if [ ! -f "$FIXTURE" ]; then
    echo "ERROR: fixture not found: $FIXTURE"
    exit 1
fi

passed=0
failed=0

run_test() {
    local name="$1"
    local pattern="$2"
    shift 2

    echo "--- $name ---"

    # Combine stdout + stderr so we capture the toolchain-info line too.
    output=$(cargo run --manifest-path "$WORKSPACE/Cargo.toml" \
        -p ice-oracle --bin ice-oracle -- \
        -n "$NIGHTLY_DATE" -f "$FIXTURE" --debug "$@" 2>&1) || true

    if echo "$output" | grep -q "$pattern"; then
        echo "PASS: found '$pattern'"
        ((passed++)) || true
    else
        echo "FAIL: expected '$pattern' in output:"
        echo "$output"
        ((failed++)) || true
    fi
    echo
}

echo "=== ice-oracle --debug resource-limit test ==="
echo "Nightly:  $NIGHTLY_DATE"
echo "Fixture:  $FIXTURE"
echo

run_test \
    "Memory exceeded (256 MB limit)" \
    "MEMORY_EXCEEDED" \
    --memory 256

run_test \
    "Wall-clock timeout (3 s, 8192 MB memory)" \
    "WALL_TIMEOUT" \
    --memory 8192 --timeout 3

echo "=== Results: $passed passed, $failed failed ==="

if [ "$failed" -gt 0 ]; then
    exit 1
fi
