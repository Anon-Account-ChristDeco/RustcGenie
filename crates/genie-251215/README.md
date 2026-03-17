# genie-251215

Structure-aware Rust code mutator that generates test cases for finding rustc ICEs (Internal Compiler Errors).

## Usage

```
genie-251215 --seeds-dir <path> --ingredients-dir <path> --output-dir <path>
```

- `--seeds-dir` — directory of `.rs` seed files to mutate
- `--ingredients-dir` — directory of `.json` annotated LLM output files (from `llm-extractor`)
- `--output-dir` — directory where generated mutant `.rs` files are written (must be empty or non-existent)
- `--ablation-profile <name>` — named ablation profile (see below)
- `--no-placeholder-adaptation` — disable placeholder adaptation (legacy flag)
- `--no-dependency-injection` — disable dependency injection (legacy flag)

The mutator runs two rounds of 10 mutations per seed (20 mutants per seed total), using a mix of structured replacement (96%), primitive type/value substitution, and attribute injection.

### Ablation Study

Use `--ablation-profile <name>` to run with a named ablation configuration:

| Profile | Snippet source | Placeholder adaptation | Dep injection | Misc mutations |
|---------|---------------|----------------------|---------------|----------------|
| `source-only` | source only | on | on (1.0) | on |
| `source-only-no-other` | source only | off | off (0.0) | on |
| `source-only-no-other-no-misc` | source only | off | off (0.0) | off |
| `no-dep-plc-misc` | both | off | off (0.0) | off |
| `half-dep-prob` | both | on | 50% (0.5) | on |

"Misc mutations" refers to the `mutate_primitive_type_snippet`, `mutate_primitive_value_snippet`, and `mutate_add_attribute` mutation paths (the 4% else-branch in the mutation loop).

Example:

```
genie-251215 -s seeds -i ingredients -o out --ablation-profile source-only
genie-251215 -s seeds -i ingredients -o out --ablation-profile source-only-no-other
genie-251215 -s seeds -i ingredients -o out --ablation-profile source-only-no-other-no-misc
genie-251215 -s seeds -i ingredients -o out --ablation-profile half-dep-prob
```

Legacy individual flags are still supported for backwards compatibility:

| Configuration | Command |
|--------------|---------|
| Full (default) | `genie-251215 -s seeds -i ingredients -o out` |
| No placeholder adaptation | `genie-251215 -s seeds -i ingredients -o out --no-placeholder-adaptation` |
| No dependency injection | `genie-251215 -s seeds -i ingredients -o out --no-dependency-injection` |
| Baseline (neither) | `genie-251215 -s seeds -i ingredients -o out --no-placeholder-adaptation --no-dependency-injection` |

## Library

The `genie-251215` crate also exposes its internals as a library:

- `code_structure` — tree-sitter node kind ID/name mapping and code structure extraction
- `compatibility` — node kind family compatibility checking
- `mutator` — core mutation engine
- `range_utils` — span/range intersection utilities
- `replacement` — structured string replacement
- `seed_filter` — heuristic seed file filtering
- `snippet` — snippet collection from seed files and LLM-extracted fragments
