use tree_sitter::{Node, Parser, Tree};

pub fn get_parser() -> Parser {
    let mut parser = Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    parser.set_language(&language).unwrap();
    parser
}

pub fn parse_code(code: &[u8]) -> Tree {
    let mut parser = get_parser();
    parser.parse(code, None).unwrap()
}

/// Collect all nodes in the tree via BFS.
pub fn collect_all_nodes(tree: &Tree) -> Vec<Node<'_>> {
    let mut all = Vec::with_capacity(16);
    let root = tree.root_node();
    let mut cursor = tree.walk();
    let mut nodes: Vec<_> = root.children(&mut cursor).collect();
    while !nodes.is_empty() {
        let mut next = Vec::new();
        for node in nodes {
            all.push(node);
            let mut child_cursor = tree.walk();
            for child in node.children(&mut child_cursor) {
                next.push(child);
            }
        }
        nodes = next;
    }
    all
}

/// Convert a tree-sitter-rust node kind ID to its string name.
pub fn kind_id_to_kind(kind_id: u16) -> &'static str {
    <tree_sitter_language::LanguageFn as Into<tree_sitter::Language>>::into(
        tree_sitter_rust::LANGUAGE,
    )
    .node_kind_for_id(kind_id)
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_rust() {
        let code = b"fn main() { let x = 1; }";
        let tree = parse_code(code);
        let root = tree.root_node();
        assert_eq!(root.kind(), "source_file");
        assert!(root.child_count() > 0);
    }

    #[test]
    fn collect_nodes_finds_children() {
        let code = b"fn main() { let x = 1 + 2; }";
        let tree = parse_code(code);
        let nodes = collect_all_nodes(&tree);
        let kinds: Vec<&str> = nodes.iter().map(|n| n.kind()).collect();
        assert!(kinds.contains(&"function_item"));
        assert!(kinds.contains(&"binary_expression"));
        assert!(kinds.contains(&"integer_literal"));
    }

    #[test]
    fn kind_id_round_trip() {
        let code = b"fn main() {}";
        let tree = parse_code(code);
        let root = tree.root_node();
        let func = root.child(0).unwrap();
        let kind_id = func.kind_id();
        let kind_name = kind_id_to_kind(kind_id);
        assert_eq!(kind_name, "function_item");
    }
}
