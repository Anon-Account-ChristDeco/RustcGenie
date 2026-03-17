# component-extractor

Parses Rust source files using [tree-sitter](https://tree-sitter.github.io/) and extracts AST node snippets grouped by node kind. These snippets are the raw building blocks for mutation-based fuzzing.

## Library

### Modules

- **`parse`** — tree-sitter parser setup and node collection
  - `get_parser()` / `parse_code()` — parse Rust source bytes into a tree-sitter AST
  - `collect_all_nodes()` — BFS traversal of all nodes in a tree
  - `kind_id_to_kind()` — resolve a numeric node kind ID to its string name
- **`snippet`** — snippet extraction from parsed ASTs
  - `Snippets` — borrows source bytes, groups unique byte slices by node kind
  - `SnippetsWFile` — owned snippets with originating file path
  - `FragmentRecord` — `{ fragment: String, node_kind: String }`, serializable with serde
  - `extract_from_dir()` — recursively walk a directory, parse all `.rs` files, return `SnippetsWFile`
- **`kind_family`** — semantic classification of tree-sitter-rust node kinds
  - `KindFamily` — enum: `Literal`, `Expr`, `Pattern`, `Type`, `TokenTree`, `TokenTreePattern`
  - `find_kind_family()` / `find_kind_family_w_parent()` — look up which family a node kind belongs to

### Example

```rust
use component_extractor::{parse_code, Snippets};

let code = b"fn main() { let x = 1 + 2; }";
let tree = parse_code(code);
let snippets = Snippets::new(vec![(code.as_slice(), &tree)]);

for (kind, fragments) in &snippets.0 {
    println!("{kind}: {} snippets", fragments.len());
}
```

## Binary

```
component-extractor <path> [--verbose] [--json]
```

- `<path>` — a `.rs` file or directory (walks recursively for `.rs` files)
- `--verbose` / `-v` — show up to 3 snippet previews per node kind
- `--json` — output all fragments as a JSON array of `{ "fragment": "...", "node_kind": "..." }`

By default, prints total snippet count then each node kind sorted by frequency, annotated with its kind family when known.
