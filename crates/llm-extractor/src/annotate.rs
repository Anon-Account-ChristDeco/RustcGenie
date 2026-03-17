use serde::{Deserialize, Serialize};

use crate::infer::{comment_removal, NodeInclusion, ParsedCode};
use crate::llm_output::{LlmOutput, RawFragment};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotatedFragment {
    pub fragment: String,
    pub criteria: String,
    pub dependencies: Vec<String>,
    pub node_kind: Option<String>,
    pub placeholders: Vec<AnnotatedPlaceholder>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotatedPlaceholder {
    pub placeholder: String,
    pub node_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotatedLlmOutput {
    #[serde(rename = "intro-structures")]
    pub intro_structures: Vec<String>,
    pub fragments: Vec<AnnotatedFragment>,
}

fn node_kind_name(kind: &crate::infer::NodeKind) -> String {
    let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
    kind.get_name(&language).to_string()
}

/// Annotate an LLM output with inferred tree-sitter node kinds.
pub fn annotate(source_code: &str, llm_output: &LlmOutput) -> AnnotatedLlmOutput {
    let parsed = ParsedCode::new(source_code);

    let fragments = llm_output
        .fragments
        .iter()
        .map(|raw_frag| {
            if let Some(ref parsed_code) = parsed {
                annotate_fragment(parsed_code, raw_frag, source_code)
            } else {
                // Can't parse → all node_kinds are None
                AnnotatedFragment {
                    fragment: raw_frag.fragment.clone(),
                    criteria: raw_frag.criteria.clone(),
                    dependencies: raw_frag.dependencies.clone(),
                    node_kind: None,
                    placeholders: raw_frag
                        .placeholders
                        .iter()
                        .map(|p| AnnotatedPlaceholder {
                            placeholder: p.clone(),
                            node_kind: None,
                        })
                        .collect(),
                }
            }
        })
        .collect();

    AnnotatedLlmOutput {
        intro_structures: llm_output.intro_structures.clone(),
        fragments,
    }
}

fn annotate_fragment(
    parsed_code: &ParsedCode,
    fragment: &RawFragment,
    source_code: &str,
) -> AnnotatedFragment {
    // Find all occurrences of this fragment in the source
    let fragment_node_inclusions: Vec<NodeInclusion> =
        NodeInclusion::new_inclusions(parsed_code, &fragment.fragment);

    let annotated_candidates: Vec<AnnotatedFragment> = fragment_node_inclusions
        .iter()
        .map(|inclusion| {
            let frag_kind = parsed_code
                .infer_nodekind_w_node_inclusion(&fragment.fragment, inclusion);

            let placeholders = fragment
                .placeholders
                .iter()
                .map(|p| annotate_placeholder(parsed_code, p, inclusion))
                .collect();

            AnnotatedFragment {
                fragment: fragment.fragment.clone(),
                criteria: fragment.criteria.clone(),
                dependencies: fragment.dependencies.clone(),
                node_kind: frag_kind.map(|k| node_kind_name(&k)),
                placeholders,
            }
        })
        .collect();

    // Special case: if both "identifier" and "type_identifier" exist, prefer "type_identifier"
    for candidate in &annotated_candidates {
        if candidate.node_kind.as_deref() == Some("type_identifier") {
            return candidate.clone();
        }
    }

    // Return first candidate if it has a node_kind
    if let Some(first) = annotated_candidates.first() {
        if first.node_kind.is_some() {
            return first.clone();
        }
    }

    // Fallback: try comment-removed code (covers both "not found in original"
    // and "found but inference returned None" cases)
    let comment_removed = comment_removal(source_code);
    if let Some(cr_parsed) = ParsedCode::new(&comment_removed) {
        let inclusion = NodeInclusion::new(&cr_parsed, &fragment.fragment);
        let frag_kind = cr_parsed
            .infer_nodekind_w_node_inclusion(&fragment.fragment, &inclusion);

        let placeholders = fragment
            .placeholders
            .iter()
            .map(|p| annotate_placeholder(&cr_parsed, p, &inclusion))
            .collect();

        return AnnotatedFragment {
            fragment: fragment.fragment.clone(),
            criteria: fragment.criteria.clone(),
            dependencies: fragment.dependencies.clone(),
            node_kind: frag_kind.map(|k| node_kind_name(&k)),
            placeholders,
        };
    }

    // Truly not found anywhere — all None
    if let Some(first) = annotated_candidates.first() {
        return first.clone();
    }
    AnnotatedFragment {
        fragment: fragment.fragment.clone(),
        criteria: fragment.criteria.clone(),
        dependencies: fragment.dependencies.clone(),
        node_kind: None,
        placeholders: fragment
            .placeholders
            .iter()
            .map(|p| AnnotatedPlaceholder {
                placeholder: p.clone(),
                node_kind: None,
            })
            .collect(),
    }
}

fn annotate_placeholder(
    parsed_code: &ParsedCode,
    placeholder: &str,
    fragment_inclusion: &NodeInclusion,
) -> AnnotatedPlaceholder {
    let fragment_span = fragment_inclusion.span;
    if fragment_span.is_none() {
        return AnnotatedPlaceholder {
            placeholder: placeholder.to_string(),
            node_kind: None,
        };
    }
    let (frag_start_idx, _) = fragment_span.unwrap();

    let placeholder_inclusion =
        NodeInclusion::new_starts_from(parsed_code, placeholder, frag_start_idx);

    let kind = parsed_code
        .infer_nodekind_w_node_inclusion(placeholder, &placeholder_inclusion);

    AnnotatedPlaceholder {
        placeholder: placeholder.to_string(),
        node_kind: kind.map(|k| node_kind_name(&k)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm_output::{LlmOutput, RawFragment};

    #[test]
    fn annotate_simple() {
        let source = "use std::fmt;\n\nfn main() {\n    println!(\"hello\");\n}";
        let llm_output = LlmOutput {
            intro_structures: vec!["use std::fmt;".to_string()],
            fragments: vec![
                RawFragment {
                    fragment: "use std::fmt;".to_string(),
                    criteria: "1".to_string(),
                    dependencies: vec![],
                    placeholders: vec!["fmt".to_string()],
                },
                RawFragment {
                    fragment: "main".to_string(),
                    criteria: "1".to_string(),
                    dependencies: vec![],
                    placeholders: vec![],
                },
            ],
        };

        let result = annotate(source, &llm_output);
        assert_eq!(result.intro_structures.len(), 1);
        assert_eq!(result.fragments.len(), 2);

        // use_declaration should be inferred
        assert!(result.fragments[0].node_kind.is_some());
        assert_eq!(
            result.fragments[0].node_kind.as_deref(),
            Some("use_declaration")
        );
    }

    #[test]
    fn annotate_hallucinated_fragment() {
        let source = "fn main() {}";
        let llm_output = LlmOutput {
            intro_structures: vec![],
            fragments: vec![RawFragment {
                fragment: "nonexistent_thing".to_string(),
                criteria: "1".to_string(),
                dependencies: vec![],
                placeholders: vec!["also_nonexistent".to_string()],
            }],
        };

        let result = annotate(source, &llm_output);
        assert_eq!(result.fragments.len(), 1);
        assert!(result.fragments[0].node_kind.is_none());
        assert!(result.fragments[0].placeholders[0].node_kind.is_none());
    }

    #[test]
    fn annotate_with_type_identifier_preference() {
        // "Foo" appears both as type_identifier (in struct definition) and could appear elsewhere
        let source = "struct Foo;\nfn bar() -> Foo { Foo }";
        let llm_output = LlmOutput {
            intro_structures: vec!["struct Foo;".to_string()],
            fragments: vec![RawFragment {
                fragment: "Foo".to_string(),
                criteria: "1".to_string(),
                dependencies: vec!["struct Foo;".to_string()],
                placeholders: vec![],
            }],
        };

        let result = annotate(source, &llm_output);
        assert_eq!(result.fragments.len(), 1);
        // Should prefer type_identifier over identifier
        assert!(result.fragments[0].node_kind.is_some());
        assert_eq!(
            result.fragments[0].node_kind.as_deref(),
            Some("type_identifier")
        );
    }

    #[test]
    fn annotate_placeholder_in_fragment() {
        let source = "fn foo(x: i32) -> i32 { x + 1 }";
        let llm_output = LlmOutput {
            intro_structures: vec!["fn foo(x: i32) -> i32 { x + 1 }".to_string()],
            fragments: vec![RawFragment {
                fragment: "x + 1".to_string(),
                criteria: "4".to_string(),
                dependencies: vec!["fn foo(x: i32) -> i32 { x + 1 }".to_string()],
                placeholders: vec!["x".to_string(), "1".to_string()],
            }],
        };

        let result = annotate(source, &llm_output);
        assert_eq!(result.fragments[0].placeholders.len(), 2);
    }

    #[test]
    fn annotate_fragment_with_comment_stripped_by_llm() {
        // Source has a comment inside a function body; the LLM omits the comment
        let source = "pub fn by_value(_x: i32) -> usize {\n    //~^ WARN something\n    0\n}";
        let llm_output = LlmOutput {
            intro_structures: vec![],
            fragments: vec![RawFragment {
                fragment: "pub fn by_value(_x: i32) -> usize {\n    0\n}".to_string(),
                criteria: "4".to_string(),
                dependencies: vec![],
                placeholders: vec!["_x".to_string(), "usize".to_string()],
            }],
        };

        let result = annotate(source, &llm_output);
        // Should find via comment-removal fallback, not return None
        assert!(
            result.fragments[0].node_kind.is_some(),
            "Expected node_kind to be inferred via comment-removal fallback, got None"
        );
        assert_eq!(
            result.fragments[0].node_kind.as_deref(),
            Some("function_item")
        );
        // Placeholders should also be inferred in the comment-removed code
        assert!(result.fragments[0].placeholders[0].node_kind.is_some());
    }
}
