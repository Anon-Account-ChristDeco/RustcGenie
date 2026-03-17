# RustcGenie

Artifact for the paper: *RustcGenie: Context-Aware Tree-Splicing Fuzzing for Discovering Internal Compiler Errors in Rust*.

RustcGenie is a context-aware tree-splicing fuzzer that discovers rustc ICEs (Internal Compiler Errors) while maintaining practical test-generation throughput. It augments grammar-preserving subtree replacement with *compilation context* — dependency injection and placeholder adaptation — so that spliced fragments compile meaningfully in the target program.

## Prerequisites

- **Rust nightly toolchain** (e.g. `rustup toolchain install nightly-2025-09-02`)
- **Python 3.13+** and [uv](https://github.com/astral-sh/uv) (only for `llm-extractor`)
- **Google Cloud CLI** authenticated (`gcloud auth application-default login`) (only for `llm-extractor`)

## Building

```sh
# Build all workspace crates
cargo build --release

# Build vendored icemaker (required for the icemaker baseline)
./vendor/setup.sh
```

## Data

Two data archives are provided with this artifact:

| Archive | Contents | Used by |
|---------|----------|---------|
| `seeds_250902.zip` | 26,954 Rust seed programs (rustc test suite + glacier2 fixed ICE cases) | `ice-perfeval-blackbox` (both icemaker and genie) |
| `ingr_gtf_annot_250902_str_nodekind.zip` | Pre-extracted semantic fragments (annotated JSON) | `ice-perfeval-blackbox` (genie only) |

Unzip them to a location of your choice:

```sh
unzip seeds_250902.zip
unzip ingr_gtf_annot_250902_str_nodekind.zip
```

## Running RustcGenie

The main evaluation driver is `ice-perfeval-blackbox`. It runs a fuzzing loop under a time budget, tracks unique ICE locations via deduplication, and produces per-iteration reports.

### RustcGenie (genie)

```sh
cargo run --release -p ice-perfeval-blackbox -- genie \
    --genie-bin target/release/genie-251215 \
    --rustc-path $(rustup which rustc --toolchain nightly-2025-09-02) \
    --seeds-dir seeds_250902 \
    --ingredients-dir ingr_gtf_annot_250902_str_nodekind \
    --output-dir output_genie \
    --time-budget 259200
```

### Icemaker baseline

```sh
cargo run --release -p ice-perfeval-blackbox -- icemaker \
    --icemaker-bin vendor/icemaker/target/debug/icemaker \
    --rustc-path $(rustup which rustc --toolchain nightly-2025-09-02) \
    --seeds-dir seeds_250902 \
    --output-dir output_icemaker \
    --time-budget 259200
```

### Ablation study

Disable individual context-aware mutation elements:

```sh
# No placeholder adaptation
cargo run --release -p ice-perfeval-blackbox -- genie \
    --genie-bin target/release/genie-251215 \
    --rustc-path $(rustup which rustc --toolchain nightly-2025-09-02) \
    --seeds-dir seeds_250902 \
    --ingredients-dir ingr_gtf_annot_250902_str_nodekind \
    --output-dir output_no_pa \
    --time-budget 259200 \
    --no-placeholder-adaptation

# No dependency injection
... --output-dir output_no_di --no-dependency-injection

# Neither (baseline tree splicing)
... --output-dir output_no_both --no-placeholder-adaptation --no-dependency-injection
```

### Common options

| Option | Default | Description |
|--------|---------|-------------|
| `--time-budget` | — | Fuzzing time budget in seconds |
| `--timeout` | 10 | Per-compilation wall-clock timeout (seconds) |
| `--memory` | 1024 | Per-compilation memory limit (MiB) |
| `--threads` | 8 | Parallel compilation threads |
| `--keep-non-ice` | off | Keep mutant files that did not trigger ICEs |

See `crates/ice-perfeval-blackbox/README.md` for output structure and resume behavior.

## Generating Semantic Fragments (llm-extractor)

The provided `ingr_gtf_annot_250902_str_nodekind` archive contains pre-extracted fragments used in the paper's evaluation. To regenerate fragments from seeds (or extract from a different seed corpus):

### Setup

```sh
# Configure your GCP project ID
edit vendor/gpt-oss-20b/config.py   # set project_id

# Install Python dependencies
cd vendor/gpt-oss-20b && uv sync && cd ../..
```

### One-click batch extraction

```sh
./crates/llm-extractor/scripts/extract-batch.sh <source-dir> <output-dir> [--workers 32] [--max-rpm 256]
```

Where `<source-dir>` is a directory of `.rs` seed files (e.g. `seeds_250902`). The output directory will contain annotated JSON files that can be passed as `--ingredients-dir` to `ice-perfeval-blackbox`.

Note: LLM outputs are non-deterministic, so regenerated fragments may differ from the provided archive.

See `crates/llm-extractor/README.md` for the three-step manual workflow and output format.

## Workspace Crates

| Crate | Description |
|-------|-------------|
| `component-extractor` | Tree-sitter-based AST snippet extraction for Rust source files |
| `genie-251215` | Context-aware tree-splicing mutator (the core of RustcGenie) |
| `ice-oracle` | ICE detection oracle — compiles Rust files and parses stderr for ICE patterns |
| `ice-perfeval-blackbox` | Evaluation driver with time-budgeted fuzzing and ICE deduplication |
| `llm-extractor` | Offline LLM-based semantic fragment extraction |
