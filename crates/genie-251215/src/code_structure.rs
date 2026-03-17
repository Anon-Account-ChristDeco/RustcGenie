use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::LazyLock;

use tree_sitter::Tree;

use component_extractor::find_kind_family;

pub type Span = Range<usize>;
pub type NodeKind = String;
pub type CodeStructure = Vec<(Span, NodeKind)>;
pub type CodeStructures = HashMap<PathBuf, CodeStructure>;

/// Punctuation and keyword node kinds that should be excluded from code structures.
pub static SYNTACTIC_NODEKINDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "(", ")", "{", "}", "[", "]", "\"", "#", "!", "//", "'", ",", ":", ";", "::", "|", "->",
        "=>", ".", "..", "...", "/*", "*/", "fn", "else", "for", "if", "impl", "in", "let",
        "loop", "match", "mod", "pub", "struct", "trait", "unsafe", "type", "use", "where",
        "while",
    ]
    .into_iter()
    .collect()
});

/// Extract the code structure (list of spans and their node kinds) from a tree via BFS.
pub fn new_code_structure(tree: &Tree) -> CodeStructure {
    let mut result = Vec::new();
    let mut nodes_q = vec![tree.root_node()];
    while !nodes_q.is_empty() {
        let mut children = Vec::with_capacity(nodes_q.len());
        for node in nodes_q {
            if node.kind() != "source_file" {
                result.push((node.byte_range(), node.kind().to_string()));
            }
            let mut i = 0;
            while let Some(child) = node.child(i) {
                children.push(child);
                i += 1;
            }
        }
        nodes_q = children;
    }
    result
}

/// Extract code structures for multiple seed files, filtering out blocks, comments,
/// and syntactic node kinds.
pub fn new_code_structures(seed_trees: &HashMap<PathBuf, (Vec<u8>, Tree)>) -> CodeStructures {
    seed_trees
        .iter()
        .map(|(path, (_, tree))| {
            let cs = new_code_structure(tree)
                .into_iter()
                .filter(|(_, kind)| {
                    kind != "block"
                        && kind != "block_comment"
                        && kind != "line_comment"
                        && !SYNTACTIC_NODEKINDS.contains(kind.as_str())
                })
                .collect();
            (path.clone(), cs)
        })
        .collect()
}

/// Returns the family names for a node kind (as formatted `Debug` strings),
/// or the node kind name itself if it belongs to no family, or `""` if `None`.
pub fn get_nodekind_strings(nodekind_opt: Option<&str>) -> Vec<String> {
    let Some(name) = nodekind_opt else {
        return vec!["".to_string()];
    };
    let result = find_kind_family(name);
    match result {
        component_extractor::KindFamilyFindResult::Unknown => {
            vec![name.to_string()]
        }
        component_extractor::KindFamilyFindResult::Unique(fam) => {
            vec![format!("{fam:?}")]
        }
        component_extractor::KindFamilyFindResult::Ambiguous(fams) => {
            fams.iter().map(|f| format!("{f:?}")).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use component_extractor::parse_code;

    #[test]
    fn code_structure_extracts_nodes() {
        let code = b"fn main() { let x = 1; }";
        let tree = parse_code(code);
        let cs = new_code_structure(&tree);
        assert!(!cs.is_empty());
        let kind_names: Vec<&str> = cs.iter().map(|(_, k)| k.as_str()).collect();
        assert!(kind_names.contains(&"function_item"));
        assert!(kind_names.contains(&"integer_literal"));
    }

    #[test]
    fn nodekind_strings_for_expr() {
        let strings = get_nodekind_strings(Some("array_expression"));
        assert_eq!(strings, vec!["Expr"]);
    }

    #[test]
    fn nodekind_strings_for_none() {
        let strings = get_nodekind_strings(None);
        assert_eq!(strings, vec![""]);
    }
}
