# ice-perfeval-blackbox

Performance evaluation driver that compares ICE-finding rates of mutation approaches under a time budget.

Supports two mutators:
- **icemaker** — vendored tool using `--codegen-splice-omni`
- **genie-251215** — workspace member with ablation study flags

## Features

- Unique ICE tracking via crash-location deduplication across iterations
- Non-ICE file cleanup to save disk space (default, disable with `--keep-non-ice`)
- Crash-safe resume — persists `seen_locations.json` after every iteration
- Per-iteration oracle reports saved to disk

## Prerequisites

When using the `icemaker` subcommand, the vendored icemaker binary must be built first:

```sh
./vendor/setup.sh
```

This initializes the submodule, applies patches, and builds the icemaker binary.

## Usage

```
ice-perfeval-blackbox icemaker \
    --icemaker-bin <PATH> --rustc-path <PATH> --seeds-dir <PATH> \
    --output-dir <PATH> --time-budget <SECS> \
    [--timeout 10] [--memory 1024] [--threads 8] [--keep-non-ice]

ice-perfeval-blackbox genie \
    --genie-bin <PATH> --rustc-path <PATH> --seeds-dir <PATH> \
    --ingredients-dir <PATH> --output-dir <PATH> --time-budget <SECS> \
    [--ablation-profile <PROFILE>] \
    [--no-placeholder-adaptation] [--no-dependency-injection] \
    [--timeout 10] [--memory 1024] [--threads 8] [--keep-non-ice]
```

Supported ablation profiles (passed through to `genie-251215 --ablation-profile`):

| Profile | Snippet source | Placeholder adaptation | Dep injection | Misc mutations |
|---------|---------------|----------------------|---------------|----------------|
| `source-only` | source only | on | on (1.0) | on |
| `source-only-no-other` | source only | off | off (0.0) | on |
| `source-only-no-other-no-misc` | source only | off | off (0.0) | off |
| `no-dep-plc-misc` | both | off | off (0.0) | off |
| `half-dep-prob` | both | on | 50% (0.5) | on |

## Output structure

```
output_dir/
├── run.log                 # Config header + per-iteration log with timing breakdown
├── seen_locations.json     # Checkpoint of all seen ICE locations (crash recovery)
├── all_ices/               # Every ICE-triggering file (copied, iter-prefixed)
├── unique_ices/            # Files with ≥1 new ICE location (copied, iter-prefixed)
├── oracle_reports/         # Per-iteration oracle text reports
└── mutants/
    └── iter_N/             # Generated mutants (non-ICE files deleted by default)
```

## Resume

If `output_dir` contains `seen_locations.json`, the tool resumes from the last completed iteration. If the directory exists and is non-empty without a checkpoint, it errors out.
