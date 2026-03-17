#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "==> Initializing submodules..."
git -C "$REPO_ROOT" submodule update --init --recursive

echo "==> Pinning vendor/icemaker to commit 01fe4df..."
git -C "$SCRIPT_DIR/icemaker" checkout 01fe4df

echo "==> Applying icemaker patch..."
if git -C "$SCRIPT_DIR/icemaker" apply --check "$REPO_ROOT/patches/icemaker-fix1219.patch" 2>/dev/null; then
    git -C "$SCRIPT_DIR/icemaker" apply "$REPO_ROOT/patches/icemaker-fix1219.patch"
    echo "    Patch applied."
else
    echo "    Patch already applied or not applicable, skipping."
fi

echo "==> Building vendor/icemaker..."
cargo build --manifest-path "$SCRIPT_DIR/icemaker/Cargo.toml"
echo "    Done."

echo "==> Setup complete."
