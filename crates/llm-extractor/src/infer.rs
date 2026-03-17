use tree_sitter::{Language, Node, Parser, Tree};

use crate::strip_ws::{
    find_all_strip_whitespace, find_all_strip_whitespace_starts_from, find_strip_whitespace,
    find_strip_whitespace_starts_from,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct NodeKind(pub u16);

impl NodeKind {
    pub fn new(node: &Node) -> Self {
        NodeKind(node.kind_id())
    }

    pub fn get_name(&self, language: &Language) -> &'static str {
        language.node_kind_for_id(self.0).unwrap()
    }

    pub fn is_error_kind(&self, language: &Language) -> bool {
        self.get_name(language) == "ERROR"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeRep {
    pub span: (usize, usize),
    pub node_kind: NodeKind,
}

impl NodeRep {
    pub fn new(node: &Node) -> Self {
        NodeRep {
            span: (node.start_byte(), node.end_byte()),
            node_kind: NodeKind::new(node),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OptionNodeRep {
    NoneValue,
    NonError(NodeKind),
    Error,
}

impl OptionNodeRep {
    fn new(node: &Option<NodeRep>, language: &Language) -> Self {
        match node {
            None => OptionNodeRep::NoneValue,
            Some(n) => {
                if n.node_kind.is_error_kind(language) {
                    OptionNodeRep::Error
                } else {
                    OptionNodeRep::NonError(n.node_kind)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeInclusion {
    pub span: Option<(usize, usize)>,
    pub parent: Option<NodeRep>,
    pub exact: Option<NodeRep>,
    pub first_child_tree: Option<NodeRep>,
}

impl NodeInclusion {
    pub fn new(parsed_code: &ParsedCode, target_string: &str) -> Self {
        let indice_opt = find_strip_whitespace(parsed_code.code, target_string, true);
        if indice_opt.is_none()
            || target_string.is_empty()
            || parsed_code.code.len() < target_string.len()
        {
            return NodeInclusion {
                span: None,
                parent: None,
                exact: None,
                first_child_tree: None,
            };
        }
        let (first_idx, last_idx) = indice_opt.unwrap();
        NodeInclusion::new_with_indices(parsed_code, first_idx, last_idx)
    }

    pub fn new_starts_from(
        parsed_code: &ParsedCode,
        target_string: &str,
        start_pos: usize,
    ) -> Self {
        let indice_opt =
            find_strip_whitespace_starts_from(parsed_code.code, target_string, true, start_pos);
        if indice_opt.is_none()
            || target_string.is_empty()
            || parsed_code.code.len() < target_string.len()
        {
            return NodeInclusion {
                span: None,
                parent: None,
                exact: None,
                first_child_tree: None,
            };
        }
        let (first_idx, last_idx) = indice_opt.unwrap();
        NodeInclusion::new_with_indices(parsed_code, first_idx, last_idx)
    }

    pub fn new_inclusions(parsed_code: &ParsedCode, target_string: &str) -> Vec<NodeInclusion> {
        let indice_opts = find_all_strip_whitespace(parsed_code.code, target_string, true, 20);
        let mut inclusions: Vec<NodeInclusion> = Vec::new();
        for (first_idx, last_idx) in indice_opts {
            let inclusion = NodeInclusion::new_with_indices(parsed_code, first_idx, last_idx);
            inclusions.push(inclusion);
        }
        inclusions
    }

    pub fn new_inclusions_starts_from(
        parsed_code: &ParsedCode,
        target_string: &str,
        start_pos: usize,
    ) -> Vec<NodeInclusion> {
        let indice_opts = find_all_strip_whitespace_starts_from(
            parsed_code.code,
            target_string,
            true,
            start_pos,
            20,
        );
        let mut inclusions: Vec<NodeInclusion> = Vec::new();
        for (first_idx, last_idx) in indice_opts {
            let inclusion = NodeInclusion::new_with_indices(parsed_code, first_idx, last_idx);
            inclusions.push(inclusion);
        }
        inclusions
    }

    pub fn new_with_indices(parsed_code: &ParsedCode, first_idx: usize, last_idx: usize) -> Self {
        fn find_child_tree_in_node(node: Node, first_idx: usize, last_idx: usize) -> Option<Node> {
            for child in node.children(&mut node.walk()) {
                if child.is_named() || child.children(&mut child.walk()).len() > 0 {
                    if first_idx <= child.start_byte() && child.start_byte() < last_idx {
                        return Some(child);
                    }
                }
            }
            None
        }

        let mut exact_node: Option<Node> = None;
        let mut parent_node: Option<Node> = None;
        let first_child_tree: Option<Node>;

        let mut cur_node = parsed_code.tree.root_node();

        loop {
            // Exact match check
            if cur_node.start_byte() == first_idx && cur_node.end_byte() == last_idx {
                exact_node = Some(cur_node);
                parent_node = cur_node.parent();
                first_child_tree = find_child_tree_in_node(cur_node, first_idx, last_idx);
                break;
            }

            // traverse children - find if any child includes the target string
            let mut cursor = cur_node.walk();
            let mut continue_flag = false;
            for node in cursor.node().children(&mut cursor) {
                if node.start_byte() <= first_idx && last_idx <= node.end_byte() {
                    parent_node = Some(cur_node);
                    cur_node = node;
                    continue_flag = true;
                    break;
                }
            }
            if continue_flag {
                continue;
            }

            // No more children includes the target string
            first_child_tree = find_child_tree_in_node(cur_node, first_idx, last_idx);
            break;
        }

        NodeInclusion {
            span: Some((first_idx, last_idx)),
            parent: parent_node.map(|n| NodeRep::new(&n)),
            exact: exact_node.map(|n| NodeRep::new(&n)),
            first_child_tree: first_child_tree.map(|n| NodeRep::new(&n)),
        }
    }
}

pub struct ParsedCode<'code> {
    pub code: &'code str,
    pub tree: Tree,
}

impl<'code> ParsedCode<'code> {
    pub fn new(code: &'code str) -> Option<Self> {
        let parser = &mut Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .ok()?;

        Some(ParsedCode {
            code,
            tree: parser.parse(code, None)?,
        })
    }

    pub fn infer_nodekind(&self, target_string: &str) -> Option<NodeKind> {
        if target_string.is_empty() {
            return None;
        }

        self.infer_nodekind_w_node_inclusion(
            target_string,
            &NodeInclusion::new(self, target_string),
        )
    }

    pub fn infer_nodekind_w_node_inclusion(
        &self,
        target_string: &str,
        node_inclusion: &NodeInclusion,
    ) -> Option<NodeKind> {
        let NodeInclusion {
            span,
            parent: parent_node,
            exact: exact_node,
            first_child_tree: first_child_tree_node,
        } = *node_inclusion;

        if span.is_none() {
            eprintln!(
                "Warning: target string not found in source code: '{}'",
                target_string
            );
            return None;
        }
        let start_idx = span.unwrap().0;
        let end_idx = span.unwrap().1;

        let language = &tree_sitter_rust::LANGUAGE.into();
        let exact_onrep = OptionNodeRep::new(&exact_node, language);
        let parent_onrep = OptionNodeRep::new(&parent_node, language);
        let firstchild_onrep = OptionNodeRep::new(&first_child_tree_node, language);

        // Decision tree (27 cases):
        // 1. Exact match is `source_file` → None
        // 2. Exact match is non-Error → return its kind
        // 3. No exact / Error → try string replacement inference
        // 4. Fallback: first child (non-Error) → its kind; else parent (non-Error, not source_file) → its kind
        // 5. Otherwise → None

        if let OptionNodeRep::NonError(node_kind) = exact_onrep {
            if node_kind.get_name(language) == "source_file" {
                // Case 1: exact match is source_file
                // If the source_file has exactly one named child, return
                // that child's kind (the fragment is the file's sole item).
                if let OptionNodeRep::NonError(child_kind) = firstchild_onrep {
                    Some(child_kind)
                } else {
                    None
                }
            } else {
                // Case 2: exact match is non-Error → return its kind
                Some(node_kind)
            }
        } else {
            // Cases 3+: no exact match or Error
            if let Some(inferred_type) =
                self.try_string_replacement_inference(start_idx, end_idx)
            {
                // Case 3: string replacement inference succeeded
                Some(inferred_type)
            } else {
                match (&parent_onrep, &firstchild_onrep) {
                    // Case 4-1: first child is non-Error
                    (_, OptionNodeRep::NonError(node_kind)) => Some(*node_kind),
                    // Case 4-2-1: parent is non-Error and not source_file
                    (OptionNodeRep::NonError(node_kind), _) => {
                        if node_kind.get_name(language) != "source_file" {
                            Some(*node_kind)
                        } else {
                            None
                        }
                    }
                    // Case 5: discard
                    _ => None,
                }
            }
        }
    }

    pub fn try_string_replacement_inference(
        &self,
        start_idx: usize,
        end_idx: usize,
    ) -> Option<NodeKind> {
        // Try with "()" replacement — infers _expression, _type, _pattern supertypes
        let replaced_code = format!(
            "{} () {}",
            &self.code[0..start_idx],
            &self.code[end_idx..self.code.len()]
        );

        let new_start_idx = start_idx + 1;
        let new_end_idx = new_start_idx + 2;
        if let Some(parsed_code) = ParsedCode::new(&replaced_code) {
            let NodeInclusion {
                exact: exact_node, ..
            } = NodeInclusion::new_with_indices(&parsed_code, new_start_idx, new_end_idx);

            if let Some(exact_node) = exact_node {
                let language = &tree_sitter_rust::LANGUAGE.into();
                match exact_node.node_kind.get_name(language) {
                    "unit_expression" | "unit_type" | "match_pattern" | "tuple_pattern" => {
                        return Some(exact_node.node_kind);
                    }
                    _ => {}
                }
            }
        }

        // Try with "trait A {}" replacement — infers _declaration_statement
        let replaced_code = format!(
            "{} trait A {{}} {}",
            &self.code[0..start_idx],
            &self.code[end_idx..self.code.len()]
        );

        let new_start_idx = start_idx + 1;
        let new_end_idx = new_start_idx + 10;
        if let Some(parsed_code) = ParsedCode::new(&replaced_code) {
            let NodeInclusion {
                exact: exact_node, ..
            } = NodeInclusion::new_with_indices(&parsed_code, new_start_idx, new_end_idx);

            if let Some(exact_node) = exact_node {
                let language = &tree_sitter_rust::LANGUAGE.into();
                match exact_node.node_kind.get_name(language) {
                    "trait_item" => return Some(exact_node.node_kind),
                    _ => {}
                }
            }
        }

        None
    }
}

/// Remove all line comments (`//`) and block comments (`/* */`) from Rust source code.
pub fn comment_removal(code: &str) -> String {
    let mut result = String::new();
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut chars = code.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
                result.push(ch);
            }
            chars.next();
        } else if in_block_comment {
            if ch == '*' {
                chars.next();
                if let Some(&next_ch) = chars.peek() {
                    if next_ch == '/' {
                        in_block_comment = false;
                        chars.next();
                    }
                }
            } else {
                chars.next();
            }
        } else if ch == '/' {
            chars.next();
            if let Some(&next_ch) = chars.peek() {
                if next_ch == '/' {
                    in_line_comment = true;
                    chars.next();
                } else if next_ch == '*' {
                    in_block_comment = true;
                    chars.next();
                } else {
                    result.push(ch);
                }
            } else {
                result.push(ch);
            }
        } else {
            result.push(ch);
            chars.next();
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_code() {
        let code = "fn main() { let x = 1; }";
        let parsed = ParsedCode::new(code);
        assert!(parsed.is_some());
    }

    #[test]
    fn infer_function_item() {
        let code = "fn foo() {} fn bar() {}";
        let parsed = ParsedCode::new(code).unwrap();
        let kind = parsed.infer_nodekind("fn foo() {}");
        assert!(kind.is_some());
        let language: Language = tree_sitter_rust::LANGUAGE.into();
        assert_eq!(kind.unwrap().get_name(&language), "function_item");
    }

    #[test]
    fn infer_identifier() {
        let code = "fn foo() { let x = 1; }";
        let parsed = ParsedCode::new(code).unwrap();
        let kind = parsed.infer_nodekind("foo");
        assert!(kind.is_some());
        let language: Language = tree_sitter_rust::LANGUAGE.into();
        assert_eq!(kind.unwrap().get_name(&language), "identifier");
    }

    #[test]
    fn infer_source_file_single_child() {
        let code = "fn main() {}";
        let parsed = ParsedCode::new(code).unwrap();
        // The entire source matches source_file, but it has one child → return that child's kind
        let kind = parsed.infer_nodekind("fn main() {}");
        assert!(kind.is_some());
        let language: Language = tree_sitter_rust::LANGUAGE.into();
        assert_eq!(kind.unwrap().get_name(&language), "function_item");
    }

    #[test]
    fn infer_source_file_multiple_children_returns_first_child() {
        // When the fragment spans the entire multi-item file,
        // first_child_tree gives the first item's kind
        let code = "fn foo() {}\nfn bar() {}";
        let parsed = ParsedCode::new(code).unwrap();
        let kind = parsed.infer_nodekind("fn foo() {}\nfn bar() {}");
        assert!(kind.is_some());
        let language: Language = tree_sitter_rust::LANGUAGE.into();
        assert_eq!(kind.unwrap().get_name(&language), "function_item");
    }

    #[test]
    fn infer_empty_string_returns_none() {
        let code = "fn main() {}";
        let parsed = ParsedCode::new(code).unwrap();
        let kind = parsed.infer_nodekind("");
        assert!(kind.is_none());
    }

    #[test]
    fn infer_not_found_returns_none() {
        let code = "fn main() {}";
        let parsed = ParsedCode::new(code).unwrap();
        let kind = parsed.infer_nodekind("nonexistent_symbol");
        assert!(kind.is_none());
    }

    #[test]
    fn infer_type_identifier() {
        let code = "struct Foo; impl Foo {}";
        let parsed = ParsedCode::new(code).unwrap();
        let kind = parsed.infer_nodekind("Foo");
        assert!(kind.is_some());
        let language: Language = tree_sitter_rust::LANGUAGE.into();
        let name = kind.unwrap().get_name(&language);
        // Could be type_identifier or identifier depending on which occurrence is found first
        assert!(name == "type_identifier" || name == "identifier");
    }

    #[test]
    fn infer_use_declaration() {
        let code = "use std::fmt;\nfn main() {}";
        let parsed = ParsedCode::new(code).unwrap();
        let kind = parsed.infer_nodekind("use std::fmt;");
        assert!(kind.is_some());
        let language: Language = tree_sitter_rust::LANGUAGE.into();
        assert_eq!(kind.unwrap().get_name(&language), "use_declaration");
    }

    #[test]
    fn comment_removal_line_comments() {
        let code = "fn main() {\n    // comment\n    let x = 1;\n}";
        let result = comment_removal(code);
        assert!(!result.contains("// comment"));
        assert!(result.contains("let x = 1;"));
    }

    #[test]
    fn comment_removal_block_comments() {
        let code = "fn main() { /* block */ let x = 1; }";
        let result = comment_removal(code);
        assert!(!result.contains("/* block */"));
        assert!(result.contains("let x = 1;"));
    }

    #[test]
    fn node_inclusion_not_found() {
        let code = "fn main() {}";
        let parsed = ParsedCode::new(code).unwrap();
        let inclusion = NodeInclusion::new(&parsed, "nonexistent");
        assert!(inclusion.span.is_none());
    }

    #[test]
    fn node_inclusion_found() {
        let code = "fn main() { let x = 1; }";
        let parsed = ParsedCode::new(code).unwrap();
        let inclusion = NodeInclusion::new(&parsed, "let x = 1;");
        assert!(inclusion.span.is_some());
    }

    #[test]
    fn string_replacement_inference_expression() {
        // Test that string replacement inference works for expression contexts
        let code = "fn main() { let x = foo + bar; }";
        let parsed = ParsedCode::new(code).unwrap();
        // "foo + bar" spans an expression context
        let kind = parsed.infer_nodekind("foo + bar");
        // Should be inferred (binary_expression or via replacement)
        assert!(kind.is_some());
    }

    #[test]
    fn multiple_inclusions() {
        let code = "let x = 1; let y = 1;";
        let parsed = ParsedCode::new(code).unwrap();
        let inclusions = NodeInclusion::new_inclusions(&parsed, "1");
        assert_eq!(inclusions.len(), 2);
    }

    #[test]
    fn infer_fn_in_comment_removed_code() {
        // After comment removal, the source has an extra blank line inside the fn body.
        // The fragment (without comment) should still be findable and inferable.
        let source_with_comment =
            "pub fn by_value(_x: i32) -> usize {\n    //~^ WARN something\n    0\n}";
        let removed = comment_removal(source_with_comment);
        eprintln!("comment-removed: {:?}", removed);

        let parsed = ParsedCode::new(&removed).unwrap();
        let fragment = "pub fn by_value(_x: i32) -> usize {\n    0\n}";
        let inclusion = NodeInclusion::new(&parsed, fragment);
        eprintln!("inclusion: {:?}", inclusion);
        assert!(inclusion.span.is_some(), "fragment should be found in comment-removed code");

        let kind = parsed.infer_nodekind_w_node_inclusion(fragment, &inclusion);
        eprintln!("kind: {:?}", kind);
        assert!(kind.is_some(), "node_kind should be inferred");
        let language: Language = tree_sitter_rust::LANGUAGE.into();
        assert_eq!(kind.unwrap().get_name(&language), "function_item");
    }
}
