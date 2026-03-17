#!/usr/bin/env bash
#
# One-click batch extraction pipeline.
#
#   Step 1: Prepare prompts from .rs source files
#   Step 2: Collect LLM responses via batch_processor.py
#   Step 3: Annotate collected JSONs with inferred node kinds
#
# Usage:
#   ./extract-batch.sh <source-dir> <output-dir> [--workers N] [--max-rpm N]
#
# Output layout:
#   <output-dir>/prompts/         — prompt-wrapped .txt files
#   <output-dir>/llm_responses/   — raw_responses/ and generated_texts/ from LLM
#   <output-dir>/results/         — annotated JSON files

set -euo pipefail

# ── defaults ──
WORKERS=32
MAX_RPM=256

# ── parse args ──
if [ $# -lt 2 ]; then
    echo "Usage: $0 <source-dir> <output-dir> [--workers N] [--max-rpm N]"
    exit 1
fi

SOURCE_DIR="$1"
OUTPUT_DIR="$2"
shift 2

while [ $# -gt 0 ]; do
    case "$1" in
        --workers)  WORKERS="$2";  shift 2 ;;
        --max-rpm)  MAX_RPM="$2";  shift 2 ;;
        *)          echo "Unknown option: $1"; exit 1 ;;
    esac
done

# ── resolve paths ──
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CRATE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
WORKSPACE_ROOT="$(cd "$CRATE_DIR/../.." && pwd)"

# Make SOURCE_DIR and OUTPUT_DIR absolute
mkdir -p "$OUTPUT_DIR"
SOURCE_DIR="$(cd "$SOURCE_DIR" && pwd)"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"

PROMPT_DIR="$OUTPUT_DIR/prompts"
LLM_RESPONSE_DIR="$OUTPUT_DIR/llm_responses"
RESULTS_DIR="$OUTPUT_DIR/results"

BATCH_PROCESSOR="$WORKSPACE_ROOT/vendor/gpt-oss-20b/batch_processor.py"

echo "=================================================="
echo "  Batch Extraction Pipeline"
echo "=================================================="
echo "  Source:       $SOURCE_DIR"
echo "  Output:       $OUTPUT_DIR"
echo "  Workers:      $WORKERS"
echo "  Max RPM:      $MAX_RPM"
echo "=================================================="

mkdir -p "$PROMPT_DIR" "$LLM_RESPONSE_DIR" "$RESULTS_DIR"

# ── Step 1: Prepare prompts ──
echo ""
echo "[Step 1/3] Preparing prompts..."
cargo run -p llm-extractor --release -- --prepare "$SOURCE_DIR" "$PROMPT_DIR"

# ── Step 2: Collect LLM responses ──
echo ""
echo "[Step 2/3] Collecting LLM responses..."
# Run from vendor directory where pyproject.toml and .venv are located
(cd "$WORKSPACE_ROOT/vendor/gpt-oss-20b" && uv run python batch_processor.py "$PROMPT_DIR" "$LLM_RESPONSE_DIR" "**/*.txt" "$WORKERS" "$MAX_RPM")

# ── Step 3: Annotate ──
echo ""
echo "[Step 3/3] Annotating with node kinds..."
cargo run -p llm-extractor --release -- --batch "$SOURCE_DIR" "$LLM_RESPONSE_DIR/generated_texts" "$RESULTS_DIR"

echo ""
echo "=================================================="
echo "  Done"
echo "=================================================="
echo "  Prompts:      $PROMPT_DIR"
echo "  LLM raw:      $LLM_RESPONSE_DIR"
echo "  Results:      $RESULTS_DIR"
echo "=================================================="
