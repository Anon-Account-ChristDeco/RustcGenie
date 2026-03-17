use std::collections::HashMap;
use std::sync::LazyLock;

pub static LITERAL_KIND_FAMILY: &[&str] = &[
    "boolean_literal",
    "char_literal",
    "float_literal",
    "integer_literal",
    "raw_string_literal",
    "string_literal",
];

pub static EXPR_KIND_FAMILY: &[&str] = &[
    // literals
    "boolean_literal",
    "char_literal",
    "float_literal",
    "integer_literal",
    "raw_string_literal",
    "string_literal",
    // other expressions
    "array_expression",
    "assignment_expression",
    "async_block",
    "await_expression",
    "binary_expression",
    "block",
    "break_expression",
    "call_expression",
    "closure_expression",
    "compound_assignment_expr",
    "const_block",
    "continue_expression",
    "field_expression",
    "for_expression",
    "gen_block",
    "generic_function",
    "identifier",
    "if_expression",
    "index_expression",
    "loop_expression",
    "macro_invocation",
    "match_expression",
    "metavariable",
    "parenthesized_expression",
    "range_expression",
    "reference_expression",
    "return_expression",
    "scoped_identifier",
    "self",
    "struct_expression",
    "try_block",
    "try_expression",
    "tuple_expression",
    "type_cast_expression",
    "unary_expression",
    "unit_expression",
    "unsafe_block",
    "while_expression",
    "yield_expression",
];

pub static PATTERN_KIND_FAMILY: &[&str] = &[
    // literals
    "boolean_literal",
    "char_literal",
    "float_literal",
    "integer_literal",
    "raw_string_literal",
    "string_literal",
    // other patterns
    "_",
    "captured_pattern",
    "const_block",
    "generic_pattern",
    "identifier",
    "macro_invocation",
    "mut_pattern",
    "or_pattern",
    "range_pattern",
    "ref_pattern",
    "reference_pattern",
    "remaining_field_pattern",
    "scoped_identifier",
    "slice_pattern",
    "struct_pattern",
    "tuple_pattern",
    "tuple_struct_pattern",
];

pub static TYPE_KIND_FAMILY: &[&str] = &[
    "abstract_type",
    "array_type",
    "bounded_type",
    "dynamic_type",
    "function_type",
    "generic_type",
    "macro_invocation",
    "metavariable",
    "never_type",
    "pointer_type",
    "primitive_type",
    "reference_type",
    "removed_trait_bound",
    "scoped_type_identifier",
    "tuple_type",
    "type_identifier",
    "unit_type",
];

pub static TOKEN_TREE_KIND_FAMILY: &[&str] = &[
    // literals
    "boolean_literal",
    "char_literal",
    "float_literal",
    "integer_literal",
    "raw_string_literal",
    "string_literal",
    // others
    "crate",
    "identifier",
    "metavariable",
    "mutable_specifier",
    "primitive_type",
    "self",
    "super",
    "token_repetition",
    "token_tree",
];

pub static TOKEN_TREE_PATTERN_KIND_FAMILY: &[&str] = &[
    // literals
    "boolean_literal",
    "char_literal",
    "float_literal",
    "integer_literal",
    "raw_string_literal",
    "string_literal",
    // others
    "crate",
    "identifier",
    "metavariable",
    "mutable_specifier",
    "primitive_type",
    "self",
    "token_binding_pattern",
    "token_repetition_pattern",
    "token_tree_pattern",
];

pub static DECL_STMT_KIND_FAMILY: &[&str] = &[
    "associated_type",
    "attribute_item",
    "const_item",
    "empty_statement",
    "enum_item",
    "extern_crate_declaration",
    "foreign_mod_item",
    "function_item",
    "function_signature_item",
    "impl_item",
    "inner_attribute_item",
    "let_declaration",
    "macro_definition",
    "macro_invocation",
    "mod_item",
    "static_item",
    "struct_item",
    "trait_item",
    "type_item",
    "union_item",
    "use_declaration",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KindFamily {
    Literal,
    Expr,
    Pattern,
    Type,
    TokenTree,
    TokenTreePattern,
    DeclStmt,
}

impl KindFamily {
    pub fn get_kind_family_def(self) -> &'static [&'static str] {
        match self {
            KindFamily::Literal => LITERAL_KIND_FAMILY,
            KindFamily::Expr => EXPR_KIND_FAMILY,
            KindFamily::Pattern => PATTERN_KIND_FAMILY,
            KindFamily::Type => TYPE_KIND_FAMILY,
            KindFamily::TokenTree => TOKEN_TREE_KIND_FAMILY,
            KindFamily::TokenTreePattern => TOKEN_TREE_PATTERN_KIND_FAMILY,
            KindFamily::DeclStmt => DECL_STMT_KIND_FAMILY,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KindFamilyFindResult {
    Unknown,
    Ambiguous(Vec<KindFamily>),
    Unique(KindFamily),
}

impl KindFamilyFindResult {
    pub fn get_kinds(&self) -> Vec<&'static str> {
        match self {
            KindFamilyFindResult::Unknown => Vec::new(),
            KindFamilyFindResult::Ambiguous(kinds) => kinds
                .iter()
                .flat_map(|k| k.get_kind_family_def().iter().copied())
                .collect(),
            KindFamilyFindResult::Unique(kind) => kind.get_kind_family_def().to_vec(),
        }
    }

    pub fn get_kinds_w_original_kind(&self, original_kind: &'static str) -> Vec<&'static str> {
        let mut kinds = self.get_kinds();
        if !kinds.is_empty() {
            kinds.push(original_kind);
        }
        kinds
    }
}

static KIND_FAMILY_FINDER: LazyLock<HashMap<&'static str, Vec<KindFamily>>> = LazyLock::new(|| {
    let mut map: HashMap<&str, Vec<KindFamily>> = HashMap::new();

    for &kind in LITERAL_KIND_FAMILY {
        map.entry(kind).or_default().push(KindFamily::Literal);
    }
    for &kind in EXPR_KIND_FAMILY {
        map.entry(kind).or_default().push(KindFamily::Expr);
    }
    for &kind in PATTERN_KIND_FAMILY {
        map.entry(kind).or_default().push(KindFamily::Pattern);
    }
    for &kind in TYPE_KIND_FAMILY {
        map.entry(kind).or_default().push(KindFamily::Type);
    }
    for &kind in TOKEN_TREE_KIND_FAMILY {
        map.entry(kind).or_default().push(KindFamily::TokenTree);
    }
    for &kind in TOKEN_TREE_PATTERN_KIND_FAMILY {
        map.entry(kind)
            .or_default()
            .push(KindFamily::TokenTreePattern);
    }
    for &kind in DECL_STMT_KIND_FAMILY {
        map.entry(kind).or_default().push(KindFamily::DeclStmt);
    }

    map
});

fn kind_families_to_result(families: &[KindFamily]) -> KindFamilyFindResult {
    match families.len() {
        0 => KindFamilyFindResult::Unknown,
        1 => KindFamilyFindResult::Unique(families[0]),
        _ => KindFamilyFindResult::Ambiguous(families.to_vec()),
    }
}

pub fn find_kind_family(kind: &str) -> KindFamilyFindResult {
    if let Some(families) = KIND_FAMILY_FINDER.get(kind) {
        return kind_families_to_result(families);
    }
    KindFamilyFindResult::Unknown
}

/// Find the kind family for a node, using parent context to disambiguate when possible.
pub fn find_kind_family_w_parent(kind: &str, parent_kind: &str) -> KindFamilyFindResult {
    match (
        KIND_FAMILY_FINDER.get(parent_kind),
        KIND_FAMILY_FINDER.get(kind),
    ) {
        (Some(p_families), Some(families)) => {
            if families.len() == 1 {
                return KindFamilyFindResult::Unique(families[0]);
            }
            let intersection: Vec<KindFamily> = families
                .iter()
                .filter(|f| p_families.contains(f))
                .cloned()
                .collect();
            kind_families_to_result(&intersection)
        }
        (None, Some(families)) => kind_families_to_result(families),
        (Some(p_families), None) => kind_families_to_result(p_families),
        (None, None) => KindFamilyFindResult::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_is_ambiguous() {
        let result = find_kind_family("integer_literal");
        match result {
            KindFamilyFindResult::Ambiguous(families) => {
                assert!(families.contains(&KindFamily::Literal));
                assert!(families.contains(&KindFamily::Expr));
                assert!(families.contains(&KindFamily::Pattern));
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn unique_type() {
        let result = find_kind_family("array_type");
        assert_eq!(result, KindFamilyFindResult::Unique(KindFamily::Type));
    }

    #[test]
    fn unknown_kind() {
        let result = find_kind_family("nonexistent_kind");
        assert_eq!(result, KindFamilyFindResult::Unknown);
    }

    #[test]
    fn parent_disambiguates() {
        // "identifier" appears in Expr, Pattern, TokenTree, TokenTreePattern
        // If parent is in Type only, the intersection should narrow it down
        let result = find_kind_family_w_parent("identifier", "array_type");
        // array_type is only in Type, but identifier is not in Type,
        // so intersection is empty => Unknown
        assert_eq!(result, KindFamilyFindResult::Unknown);
    }

    #[test]
    fn get_kinds_returns_family_members() {
        let result = find_kind_family("array_type");
        let kinds = result.get_kinds();
        assert!(kinds.contains(&"array_type"));
        assert!(kinds.contains(&"reference_type"));
    }
}
