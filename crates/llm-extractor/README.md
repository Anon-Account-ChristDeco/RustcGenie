# llm-extractor

LLM-based semantic fragment extraction for Rust source files.

## What it does

1. **Builds a prompt** from a Rust source file using front/end templates
2. **Calls an LLM** (single-file mode) or delegates to `batch_processor.py` (batch mode)
3. **Parses the LLM JSON response** — tolerant of ` ```json ``` ` wrapping and malformed output
4. **Infers `node_kind`** for each fragment and placeholder by locating verbatim strings in the seed file's tree-sitter AST
5. **Produces annotated output** — each fragment carries `fragment`, `criteria`, `dependencies`, `placeholders`, and inferred `node_kind` (or `null`)

## CLI

### Single file

```
llm-extractor <rust-file> [--dry-run]
```

- `--dry-run` — print the constructed prompt without calling the LLM

### Batch mode (one-click)

```
./crates/llm-extractor/scripts/extract-batch.sh <source-dir> <output-dir> [--workers 32] [--max-rpm 256]
```

Runs all three steps below in sequence.

> **Prerequisites:** The batch script requires Google Cloud CLI and Python dependencies. See `vendor/gpt-oss-20b/README.md` for setup instructions. Edit `vendor/gpt-oss-20b/config.py` to set your project ID, then run `cd vendor/gpt-oss-20b && uv sync` once to install dependencies.

Output layout:

```
<output-dir>/
  prompts/          — prompt-wrapped .txt files
  llm_responses/    — raw_responses/ and generated_texts/ from LLM
  results/          — annotated JSON files
```

### Batch mode (three-step workflow)

**Step 1: Prepare prompts** from `.rs` source files:

```
llm-extractor --prepare <source-dir> <prompt-dir>
```

Walks `source-dir` for `.rs` files, wraps each with the prompt template, writes to `prompt-dir/<stem>.txt`.

**Step 2: Collect LLM responses** using the Python batch processor:

```
uv run python vendor/gpt-oss-20b/batch_processor.py <prompt-dir> <llm-output-dir> "**/*.txt" [num_workers] [max_rpm]
```

This produces `llm-output-dir/generated_texts/<stem>.json` and `llm-output-dir/raw_responses/<stem>.txt`.

> **Note:** The Python client requires Google Cloud CLI installed and initialized (`gcloud auth application-default login`). Edit `vendor/gpt-oss-20b/config.py` to set your project ID. `uv run python` is recommended over bare `python`. See `vendor/gpt-oss-20b/README.md` for full setup instructions.

**Step 3: Annotate** the collected JSONs with inferred node kinds:

```
llm-extractor --batch <source-dir> <llm-output-dir>/generated_texts <annotated-output-dir>
```

Reads each `.json` from the `generated_texts` directory, matches to `source-dir/<stem>.rs`, infers node kinds, writes refined JSON to `annotated-output-dir`. This directory should be separate from `llm-output-dir` to avoid mixing raw and annotated results.

## Output format

```json
{
  "intro-structures": ["use std::fmt;", ...],
  "fragments": [
    {
      "fragment": "fmt::Display",
      "criteria": "2",
      "dependencies": ["use std::fmt;"],
      "node_kind": "scoped_identifier",
      "placeholders": [
        {
          "placeholder": "fmt",
          "node_kind": "identifier"
        }
      ]
    }
  ]
}
```

Fragments or placeholders whose `node_kind` cannot be inferred will have `"node_kind": null`.

## Modules

| Module | Purpose |
|---|---|
| `prompt` | Embeds prompt templates, builds the full prompt |
| `llm_client` | Shells out to the Python LLM client (single-file mode) |
| `llm_output` | Raw LLM response types (`LlmOutput`, `RawFragment`) |
| `json_parse` | Generous JSON parsing (handles code fences) |
| `strip_ws` | Whitespace-tolerant substring search (KMP) |
| `infer` | Node kind inference from tree-sitter AST |
| `annotate` | Annotation pipeline: parse → infer → output |
| `batch` | Batch prepare and batch annotate |

## Dependencies

- `component-extractor` — for `parse_code` and `kind_id_to_kind`
- `tree-sitter` / `tree-sitter-rust` — AST parsing
- `serde` / `serde_json` — JSON serialization
- `walkdir` — directory traversal for batch mode
- Python `gpt-oss-20b-google-cloud-call` client (`vendor/gpt-oss-20b/`)
