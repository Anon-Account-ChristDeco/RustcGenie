use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tree_sitter::Tree;
use walkdir::WalkDir;

use crate::parse::parse_code;

/// A single extracted code fragment with its node kind.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FragmentRecord {
    pub fragment: String,
    pub node_kind: String,
}

/// Snippets grouped by node kind. Each entry maps a node kind string to the
/// set of unique byte slices extracted from the parsed trees.
#[derive(Debug, Clone)]
pub struct Snippets<'a>(pub HashMap<&'static str, Vec<&'a [u8]>>);

impl<'a> Snippets<'a> {
    /// Build snippets from a list of `(source_bytes, tree)` pairs.
    pub fn new(trees: Vec<(&'a [u8], &'a Tree)>) -> Self {
        let mut snippets: HashMap<&str, HashSet<&[u8]>> = HashMap::with_capacity(trees.len());
        for (text, tree) in trees {
            let mut nodes = vec![tree.root_node()];
            while !nodes.is_empty() {
                let mut children = Vec::with_capacity(nodes.len());
                for node in nodes {
                    snippets
                        .entry(node.kind())
                        .or_insert_with(|| HashSet::with_capacity(1))
                        .insert(&text[node.byte_range()]);
                    let mut i = 0;
                    while let Some(child) = node.child(i) {
                        children.push(child);
                        i += 1;
                    }
                }
                nodes = children;
            }
        }
        Snippets(
            snippets
                .into_iter()
                .map(|(k, s)| (k, s.into_iter().collect()))
                .collect(),
        )
    }

    /// Number of possible single-node substitutions across all kinds.
    pub fn possible(&self) -> usize {
        self.0.values().map(|s| s.len().saturating_sub(1)).sum()
    }

    /// Flatten into a list of [`FragmentRecord`]s suitable for JSON serialization.
    pub fn to_fragment_records(&self) -> Vec<FragmentRecord> {
        let mut records = Vec::new();
        for (&kind, fragments) in &self.0 {
            for fragment in fragments {
                records.push(FragmentRecord {
                    fragment: String::from_utf8_lossy(fragment).into_owned(),
                    node_kind: kind.to_string(),
                });
            }
        }
        records
    }
}

/// Like [`Snippets`], but each snippet carries the path of the file it came from
/// and owns its bytes.
#[derive(Debug)]
pub struct SnippetsWFile(pub HashMap<&'static str, Vec<(PathBuf, Vec<u8>)>>);

impl SnippetsWFile {
    /// Build snippets from a map of `path -> (source_bytes, tree)`.
    pub fn new(trees: HashMap<PathBuf, (Vec<u8>, Tree)>) -> Self {
        let mut snippets: HashMap<&str, HashSet<(PathBuf, Vec<u8>)>> =
            HashMap::with_capacity(trees.len());
        for (filepath, (text, tree)) in trees {
            let mut worklist: VecDeque<tree_sitter::Node> = VecDeque::new();
            worklist.push_back(tree.root_node());
            while let Some(node) = worklist.pop_front() {
                let node_text = text[node.byte_range()].to_vec();
                snippets
                    .entry(node.kind())
                    .or_insert_with(|| HashSet::with_capacity(1))
                    .insert((filepath.clone(), node_text));
                for i in 0..node.child_count() {
                    worklist.push_back(node.child(i).unwrap());
                }
            }
        }
        SnippetsWFile(
            snippets
                .into_iter()
                .map(|(k, s)| (k, s.into_iter().collect()))
                .collect(),
        )
    }

    /// Number of possible single-node substitutions across all kinds.
    pub fn possible(&self) -> usize {
        self.0.values().map(|s| s.len().saturating_sub(1)).sum()
    }

    /// Flatten into a list of [`FragmentRecord`]s suitable for JSON serialization.
    pub fn to_fragment_records(&self) -> Vec<FragmentRecord> {
        let mut records = Vec::new();
        for (&kind, entries) in &self.0 {
            for (_path, text) in entries {
                records.push(FragmentRecord {
                    fragment: String::from_utf8_lossy(text).into_owned(),
                    node_kind: kind.to_string(),
                });
            }
        }
        records
    }
}

/// Walk `dir` recursively, parse every `.rs` file, and return the collected
/// snippets with file provenance.
pub fn extract_from_dir(dir: &Path) -> SnippetsWFile {
    let mut trees: HashMap<PathBuf, (Vec<u8>, Tree)> = HashMap::new();
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
    {
        let path = entry.into_path();
        if let Ok(source) = std::fs::read(&path) {
            let tree = parse_code(&source);
            trees.insert(path, (source, tree));
        }
    }
    SnippetsWFile::new(trees)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_code;

    #[test]
    fn snippets_basic() {
        let code = b"fn main() { let x = 1; }";
        let tree = parse_code(code);
        let snippets = Snippets::new(vec![(code.as_slice(), &tree)]);
        assert!(snippets.0.contains_key("function_item"));
        assert!(snippets.0.contains_key("integer_literal"));
    }

    #[test]
    fn snippets_w_file_basic() {
        let code = b"fn foo() -> i32 { 42 }";
        let tree = parse_code(code);
        let mut trees = HashMap::new();
        trees.insert(PathBuf::from("test.rs"), (code.to_vec(), tree));
        let snippets = SnippetsWFile::new(trees);
        assert!(snippets.0.contains_key("function_item"));
        // Every entry should carry the filename
        for entries in snippets.0.values() {
            for (path, _) in entries {
                assert_eq!(path, &PathBuf::from("test.rs"));
            }
        }
    }

    #[test]
    fn fragment_records_roundtrip() {
        let code = b"fn main() { let x = 1; }";
        let tree = parse_code(code);
        let snippets = Snippets::new(vec![(code.as_slice(), &tree)]);
        let records = snippets.to_fragment_records();
        assert!(!records.is_empty());
        assert!(records.iter().any(|r| r.node_kind == "integer_literal"));
        // Verify JSON round-trip
        let json = serde_json::to_string(&records).unwrap();
        let decoded: Vec<FragmentRecord> = serde_json::from_str(&json).unwrap();
        assert_eq!(records, decoded);
    }

    #[test]
    fn extract_from_dir_works() {
        let tmp = std::env::temp_dir().join("component_extractor_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("hello.rs"), b"fn hello() {}").unwrap();
        let snippets = extract_from_dir(&tmp);
        assert!(snippets.0.contains_key("function_item"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
