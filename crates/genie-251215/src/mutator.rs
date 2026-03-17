use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use tree_sitter::Tree;

use crate::code_structure::{CodeStructure, CodeStructures, NodeKind, Span, get_nodekind_strings};
use crate::range_utils;
use crate::replacement;
use crate::snippet::{AllDependencies, AllSnippets, Snippet};

use component_extractor::{DECL_STMT_KIND_FAMILY, KindFamily};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SnippetSource {
    Both,       // 50/50 source vs fragments (current default)
    SourceOnly, // only snip_from_source
}

pub struct MutatorConfig {
    pub mutation_per_seed: usize,
    /// Enable placeholder adaptation (filling placeholders with compatible code from seed).
    /// Set to false for ablation study.
    pub enable_placeholder_adaptation: bool,
    /// Probability of injecting dependencies (1.0 = always, 0.0 = never, 0.5 = half).
    pub dep_injection_prob: f64,
    /// Which snippet sources to draw from.
    pub snippet_source: SnippetSource,
    /// Disable misc mutations (primitive type/value substitution and attribute injection).
    /// When true the else-branch of mutate_n is never entered (ablation study).
    pub disable_misc_mutations: bool,
}

pub struct Mutator {
    mutant_dir: PathBuf,
    pub seed_trees: HashMap<PathBuf, (Vec<u8>, Tree)>,
    pub seed_structures: CodeStructures,
    pub all_snippets: AllSnippets,
    pub all_dependencies: AllDependencies,
    pub attribute_snippets: Vec<String>,
}

fn choose_from_slice<'a, T>(rng: &mut fastrand::Rng, slice: &'a [T]) -> &'a T {
    &slice[rng.usize(..slice.len())]
}

fn gen_bool(rng: &mut fastrand::Rng, p: f64) -> bool {
    rng.f64() < p
}

impl Mutator {
    pub fn new(
        mutant_dir: PathBuf,
        seed_trees: HashMap<PathBuf, (Vec<u8>, Tree)>,
        seed_structures: CodeStructures,
        all_snippets: AllSnippets,
        all_dependencies: AllDependencies,
    ) -> Self {
        let _ = std::fs::create_dir_all(&mutant_dir);

        // Collect attribute snippets
        let mut attribute_snippets: HashSet<String> = HashSet::new();

        for (_, snippets) in all_snippets.snip_from_source.iter() {
            for snip in snippets.iter() {
                if let Some(kind) = &snip.kind {
                    if kind.ends_with("attribute_item") {
                        let snip_str = String::from_utf8_lossy(&snip.fragment).to_string();
                        attribute_snippets.insert(snip_str);
                    }
                }
            }
        }
        for (_, snippets) in all_snippets.snip_from_fragments.iter() {
            for snip in snippets.iter() {
                if let Some(kind) = &snip.kind {
                    if kind.ends_with("attribute_item") {
                        let snip_str = String::from_utf8_lossy(&snip.fragment).to_string();
                        attribute_snippets.insert(snip_str);
                    }
                }
            }
        }

        let attribute_snippets: Vec<String> = attribute_snippets.into_iter().collect();

        Mutator {
            mutant_dir,
            seed_trees,
            seed_structures,
            all_snippets,
            all_dependencies,
            attribute_snippets,
        }
    }

    pub fn mutate_n(
        &self,
        seeds: &[PathBuf],
        config: &MutatorConfig,
        rng: &mut fastrand::Rng,
    ) -> Vec<Vec<PathBuf>> {
        let mut ret: Vec<Vec<PathBuf>> = Vec::with_capacity(seeds.len());

        for file in seeds {
            let seed_source_code = match self.seed_trees.get(file) {
                Some((code, _)) => code,
                None => continue,
            };

            let mut v: Vec<Vec<u8>> = Vec::with_capacity(config.mutation_per_seed);
            for _ in 0..config.mutation_per_seed {
                let seed_structure = match self.seed_structures.get(file) {
                    Some(s) => s,
                    None => continue,
                };
                let r: Option<(Vec<u8>, Vec<PathBuf>)> = if config.disable_misc_mutations
                    || gen_bool(rng, 0.96)
                {
                    mutate(
                        seed_source_code,
                        seed_structure,
                        &self.all_snippets,
                        &self.all_dependencies,
                        config,
                        rng,
                    )
                } else {
                    match rng.usize(0..10) {
                        0..=5 => {
                            mutate_primitive_type_snippet(seed_source_code, seed_structure, rng)
                        }
                        6 => mutate_primitive_value_snippet(seed_source_code, seed_structure, rng),
                        7..=9 => mutate_add_attribute(
                            seed_source_code,
                            seed_structure,
                            &self.attribute_snippets,
                            rng,
                        ),
                        _ => unreachable!(),
                    }
                };

                let Some((txt, dep_paths)) = r else {
                    continue;
                };
                let txt_str = String::from_utf8_lossy(&txt);

                // Reject FP-related mutant results
                if IN_CODE_FP_KEYWORDS
                    .iter()
                    .any(|fp_keyword| txt_str.contains(fp_keyword))
                {
                    continue;
                }

                // Append seed file name and dependency paths as comments
                let combined_contents: Vec<u8> = {
                    let file_display = file.display();
                    let dep_paths_display: Vec<String> =
                        dep_paths.iter().map(|p| p.display().to_string()).collect();
                    let mut filename_comment: Vec<u8> = Vec::new();
                    filename_comment.extend_from_slice(b"\n\n// ");
                    filename_comment.extend_from_slice(file_display.to_string().as_bytes());
                    for dep_path_display in dep_paths_display.iter() {
                        filename_comment.extend_from_slice(b"\n// ");
                        filename_comment.extend_from_slice(dep_path_display.as_bytes());
                    }
                    filename_comment.extend_from_slice(b"\n");

                    let mut combined = Vec::with_capacity(txt.len() + filename_comment.len());
                    combined.extend_from_slice(&txt);
                    combined.extend_from_slice(&filename_comment);
                    combined
                };

                v.push(combined_contents);
            }

            let pbs = write_hashed_files(&v, &self.mutant_dir);
            ret.push(pbs);
        }

        ret
    }
}

pub fn mutate(
    seed_source_code: &[u8],
    seed_structure: &CodeStructure,
    all_snippets: &AllSnippets,
    all_dependencies: &AllDependencies,
    config: &MutatorConfig,
    rng: &mut fastrand::Rng,
) -> Option<(Vec<u8>, Vec<PathBuf>)> {
    // Select how many times to replace
    let replace_times = match rng.usize(1..=7) {
        1..=3 => 1,
        4..=6 => 2,
        7 => 3,
        _ => unreachable!(),
    };

    let chosen_source_string_structure: Vec<(Span, NodeKind)> =
        multiple_span_selection(seed_structure, replace_times, rng);

    // Collect (span_index, snippet) pairs to keep spans and snippets in sync
    // even when some spans are skipped via `continue`.
    let (matched_indices, chosen_snippets): (Vec<usize>, Vec<Snippet>) = {
        let mut pairs: Vec<(usize, Snippet)> =
            Vec::with_capacity(chosen_source_string_structure.len());
        for (i, (_, nodekind)) in chosen_source_string_structure.iter().enumerate() {
            // Choose from source or fragment snippets
            let domain_snippets: &HashMap<String, Vec<Snippet>> = match config.snippet_source {
                SnippetSource::SourceOnly => &all_snippets.snip_from_source,
                SnippetSource::Both => {
                    if gen_bool(rng, 0.5) {
                        &all_snippets.snip_from_source
                    } else {
                        &all_snippets.snip_from_fragments
                    }
                }
            };

            let compatible_nodekinds: Vec<String> = if gen_bool(rng, 0.1) {
                // Chaotic: choose from all nodekinds
                domain_snippets.keys().cloned().collect()
            } else {
                get_nodekind_strings(Some(nodekind.as_str()))
            };

            if compatible_nodekinds.is_empty() {
                continue;
            }

            let target_nodekind = choose_from_slice(rng, &compatible_nodekinds);

            let candidate_snippets: &Vec<Snippet> = match domain_snippets.get(target_nodekind) {
                Some(snips) if !snips.is_empty() => snips,
                _ => match all_snippets.snip_from_source.get(target_nodekind) {
                    Some(snips) if !snips.is_empty() => snips,
                    _ => continue,
                },
            };

            let chosen_snippet: &Snippet = choose_from_slice(rng, candidate_snippets);
            pairs.push((i, chosen_snippet.clone()));
        }
        pairs.into_iter().unzip()
    };

    // Filter chosen_source_string_structure to only the spans that got snippets
    let chosen_source_string_structure: Vec<(Span, NodeKind)> = matched_indices
        .iter()
        .map(|&i| chosen_source_string_structure[i].clone())
        .collect();

    if chosen_snippets.is_empty() {
        return None;
    }

    // Fill snippet placeholders (conditional on config)
    let chosen_snippet_ph_filled: Vec<Vec<u8>> = if config.enable_placeholder_adaptation {
        let mut filled_snippets: Vec<Vec<u8>> = Vec::with_capacity(chosen_snippets.len());
        for snippet in chosen_snippets.iter() {
            let mut filled_fragment: Vec<u8> = snippet.fragment.clone();

            for (ph, ph_nodekind_opt) in snippet.placeholders.iter() {
                if gen_bool(rng, 0.5) {
                    continue; // do not replace
                }

                let compatible_nodekinds: HashSet<String> =
                    if let Some(ph_nodekind) = ph_nodekind_opt {
                        get_nodekind_strings(Some(ph_nodekind.as_str()))
                            .into_iter()
                            .collect()
                    } else {
                        if gen_bool(rng, 0.9) {
                            continue; // do not replace
                        }
                        get_nodekind_strings(None).into_iter().collect()
                    };

                // Collect candidate replacement fragments from seed structure
                let mut candidate_str_for_ph: Vec<&str> = Vec::new();
                for (span, nodekind) in seed_structure.iter() {
                    // Convert seed nodekind to family names for proper comparison
                    let seed_families = get_nodekind_strings(Some(nodekind.as_str()));
                    let is_compatible = seed_families
                        .iter()
                        .any(|fam| compatible_nodekinds.contains(fam));
                    if is_compatible {
                        let s = &seed_source_code[span.clone()];
                        if let Ok(s_str) = std::str::from_utf8(s) {
                            candidate_str_for_ph.push(s_str);
                        }
                    }
                }

                if candidate_str_for_ph.is_empty() {
                    continue;
                }
                let str_for_ph: &str = *choose_from_slice(rng, &candidate_str_for_ph);

                // Use current filled_fragment as the working base so that
                // multiple placeholder substitutions accumulate correctly instead
                // of each one overwriting from the original fragment.
                let current_fragment_str: String = match std::str::from_utf8(&filled_fragment) {
                    Ok(s) => s.to_owned(),
                    Err(_) => continue,
                };

                let candidate_spans: Vec<Span> = {
                    let ranges = llm_extractor::strip_ws::find_all_strip_whitespace(
                        &current_fragment_str,
                        ph,
                        true,
                        3,
                    );
                    // Filter to word-boundary matches only, avoiding replacement of
                    // substrings that appear inside larger identifiers
                    ranges
                        .iter()
                        .filter(|(start, end)| {
                            let before_ok = *start == 0
                                || !current_fragment_str[..*start]
                                    .chars()
                                    .last()
                                    .map(|c| c.is_alphanumeric() || c == '_')
                                    .unwrap_or(false);
                            let after_ok = *end >= current_fragment_str.len()
                                || !current_fragment_str[*end..]
                                    .chars()
                                    .next()
                                    .map(|c| c.is_alphanumeric() || c == '_')
                                    .unwrap_or(false);
                            before_ok && after_ok
                        })
                        .map(|r| r.0..r.1)
                        .collect()
                };

                filled_fragment = replacement::just_replace_string_multiple_times(
                    &current_fragment_str,
                    &candidate_spans
                        .iter()
                        .zip(vec![str_for_ph; candidate_spans.len()])
                        .map(|(span, s)| (span, s))
                        .collect::<Vec<_>>(),
                )
                .into_bytes();
            }

            // Deletion mutation
            if gen_bool(rng, 0.1) {
                filled_fragment = Vec::new();
            }

            filled_snippets.push(filled_fragment);
        }
        filled_snippets
    } else {
        // Ablation: skip placeholder adaptation, just use raw fragments
        chosen_snippets
            .iter()
            .map(|snippet| {
                let mut fragment = snippet.fragment.clone();
                // Still apply deletion mutation for consistency
                if gen_bool(rng, 0.1) {
                    fragment = Vec::new();
                }
                fragment
            })
            .collect()
    };

    // Perform replacement — use lossy conversion to avoid panics on invalid UTF-8
    let chosen_snippet_strings: Vec<String> = chosen_snippet_ph_filled
        .iter()
        .map(|v| String::from_utf8_lossy(v).into_owned())
        .collect();

    let spans_and_target_strings: Vec<(&Span, &str)> = chosen_source_string_structure
        .iter()
        .zip(chosen_snippet_strings.iter())
        .map(|((span, _), filled_snippet)| (span, filled_snippet.as_str()))
        .collect();

    let mutated_string = replacement::just_replace_string_multiple_times(
        std::str::from_utf8(seed_source_code).unwrap(),
        &spans_and_target_strings,
    );

    if mutated_string.is_empty() {
        return None;
    }

    // Add dependency strings at the front (conditional on config)
    let mutated_string: String = if gen_bool(rng, config.dep_injection_prob) {
        let mut dependencies_to_add: Vec<String> = Vec::new();
        for snippet in chosen_snippets.iter() {
            for &dep_index in snippet.dependencies.iter() {
                let dep_string = all_dependencies.get(dep_index).unwrap();
                dependencies_to_add.push(dep_string.clone());
            }
        }

        if dependencies_to_add.is_empty() {
            mutated_string
        } else {
            dependencies_to_add.sort();
            dependencies_to_add.dedup();

            let mut result = String::new();
            for dep in dependencies_to_add.iter() {
                result.push_str(dep);
                if !dep.ends_with('\n') {
                    result.push('\n');
                }
            }
            result.push_str(&mutated_string);
            result
        }
    } else {
        mutated_string
    };

    let snippet_pathbufs: Vec<PathBuf> = chosen_snippets
        .iter()
        .map(|snippet| snippet.filename.clone())
        .collect();

    Some((mutated_string.into_bytes(), snippet_pathbufs))
}

static PRIMITIVE_TYPES: &[&str] = &[
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32",
    "f64", "bool", "()", "char", "str", "(_)",
];

static PRIMITIVE_VALUES: &[&str] = &[
    "0",
    "1",
    "-1",
    "1.0",
    "'a'",
    "\"A\"",
    "()",
    "_",
    "(_)",
    "true",
    "false",
    "0xffff_ffff_ffff_ffff",
];

pub fn mutate_primitive_type_snippet(
    seed_source_code: &[u8],
    seed_structure: &CodeStructure,
    rng: &mut fastrand::Rng,
) -> Option<(Vec<u8>, Vec<PathBuf>)> {
    let filtered_seed_structure: Vec<(Span, NodeKind)> = seed_structure
        .iter()
        .filter(|(_, kind)| kind == "primitive_type")
        .cloned()
        .collect();

    if filtered_seed_structure.is_empty() {
        return None;
    }

    let (selected_span, _) = choose_from_slice(rng, &filtered_seed_structure).clone();

    if selected_span.start >= selected_span.end || selected_span.end > seed_source_code.len() {
        return None;
    }

    let chosen_primitive: &str = *choose_from_slice(rng, PRIMITIVE_TYPES);

    let mutated_string = replacement::just_replace_string(
        std::str::from_utf8(seed_source_code).unwrap(),
        &selected_span,
        chosen_primitive,
    )?;

    Some((
        mutated_string.into_bytes(),
        vec![PathBuf::from("TYPE_SNIPPET")],
    ))
}

pub fn mutate_primitive_value_snippet(
    seed_source_code: &[u8],
    seed_structure: &CodeStructure,
    rng: &mut fastrand::Rng,
) -> Option<(Vec<u8>, Vec<PathBuf>)> {
    let expr_family = KindFamily::Expr.get_kind_family_def();
    let filtered_seed_structure: Vec<(Span, NodeKind)> = seed_structure
        .iter()
        .filter(|(_, kind)| expr_family.contains(&kind.as_str()) && kind != "identifier")
        .cloned()
        .collect();

    if filtered_seed_structure.is_empty() {
        return None;
    }

    let (selected_span, _) = choose_from_slice(rng, &filtered_seed_structure).clone();

    if selected_span.start >= selected_span.end || selected_span.end > seed_source_code.len() {
        return None;
    }

    let chosen_primitive: &str = *choose_from_slice(rng, PRIMITIVE_VALUES);

    let mutated_string = replacement::just_replace_string(
        std::str::from_utf8(seed_source_code).unwrap(),
        &selected_span,
        chosen_primitive,
    )?;

    Some((
        mutated_string.into_bytes(),
        vec![PathBuf::from("VALUE_SNIPPET")],
    ))
}

pub fn mutate_add_attribute(
    seed_source_code: &[u8],
    seed_structure: &CodeStructure,
    attribute_snippets: &[String],
    rng: &mut fastrand::Rng,
) -> Option<(Vec<u8>, Vec<PathBuf>)> {
    let filtered_seed_structure: Vec<(Span, NodeKind)> = seed_structure
        .iter()
        .filter(|(_, kind)| DECL_STMT_KIND_FAMILY.contains(&kind.as_str()))
        .cloned()
        .collect();

    if filtered_seed_structure.is_empty() {
        return None;
    }

    let (selected_span, _) = choose_from_slice(rng, &filtered_seed_structure).clone();

    if selected_span.start >= selected_span.end || selected_span.end > seed_source_code.len() {
        return None;
    }

    if attribute_snippets.is_empty() {
        return None;
    }
    let chosen_attribute: &str = choose_from_slice(rng, attribute_snippets).as_str();

    let start_span = selected_span.start;
    if start_span > seed_source_code.len() {
        return None;
    }

    let mutated_string = format!(
        "{}{}\n{}",
        std::str::from_utf8(&seed_source_code[0..start_span]).unwrap(),
        chosen_attribute,
        std::str::from_utf8(&seed_source_code[start_span..]).unwrap(),
    );

    Some((
        mutated_string.into_bytes(),
        vec![PathBuf::from("ADD_ATTRIBUTE")],
    ))
}

pub const IN_CODE_FP_KEYWORDS: &[&str] = &[
    "panicked at",
    "RUST_BACKTRACE=",
    "(core dumped)",
    "mir!",
    "#![no_core]",
    "#[rustc_symbol_name]",
    "break rust",
    "#[rustc_variance]",
    "qemu: uncaught target signal",
    "core_intrinsics",
    "platform_intrinsics",
    "::SIGSEGV",
    "SIGSEGV::",
    "span_delayed_bug_from_inside_query",
    "rustc_layout_scalar_valid_range_end",
    "rustc_attrs",
    "staged_api",
    "lang_items",
    "#[rustc_intrinsic]",
];

pub fn multiple_span_selection(
    source_string_structure: &[(Span, NodeKind)],
    times: usize,
    rng: &mut fastrand::Rng,
) -> Vec<(Span, NodeKind)> {
    let mut available_spans = source_string_structure.to_vec();
    let mut selected_spans: Vec<(Span, NodeKind)> = Vec::new();

    for _ in 0..times {
        if available_spans.is_empty() {
            break;
        }
        let chosen = choose_from_slice(rng, &available_spans).clone();
        selected_spans.push(chosen.clone());
        available_spans.retain(|(s, _)| !range_utils::has_intersection_range(s, &chosen.0));
    }

    selected_spans
}

/// Write mutant contents to files with hashed filenames.
pub fn write_hashed_files(contents: &[Vec<u8>], output_dir: &PathBuf) -> Vec<PathBuf> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut paths = Vec::with_capacity(contents.len());
    for content in contents {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        let hash = hasher.finish();
        let filename = format!("{hash:016x}.rs");
        let path = output_dir.join(filename);
        if let Err(e) = std::fs::write(&path, content) {
            eprintln!("Warning: failed to write mutant {}: {}", path.display(), e);
            continue;
        }
        paths.push(path);
    }
    paths
}

#[cfg(test)]
mod tests {
    use crate::replacement::just_replace_string_multiple_times;
    use llm_extractor::strip_ws::find_all_strip_whitespace;

    /// Simulate the placeholder-filling inner loop for a single snippet with
    /// multiple placeholders, verifying that substitutions accumulate correctly
    /// (each iteration builds on the previous result, not the original fragment).
    fn apply_placeholders(
        fragment: &str,
        placeholders: &[(&str, &str)], // (placeholder_text, replacement_value)
    ) -> String {
        let mut filled_fragment: Vec<u8> = fragment.as_bytes().to_vec();

        for (ph, replacement) in placeholders {
            let current_fragment_str: String =
                std::str::from_utf8(&filled_fragment).unwrap().to_owned();

            let ranges = find_all_strip_whitespace(&current_fragment_str, ph, true, 3);
            let candidate_spans: Vec<std::ops::Range<usize>> = ranges
                .iter()
                .filter(|(start, end)| {
                    let before_ok = *start == 0
                        || !current_fragment_str[..*start]
                            .chars()
                            .last()
                            .map(|c| c.is_alphanumeric() || c == '_')
                            .unwrap_or(false);
                    let after_ok = *end >= current_fragment_str.len()
                        || !current_fragment_str[*end..]
                            .chars()
                            .next()
                            .map(|c| c.is_alphanumeric() || c == '_')
                            .unwrap_or(false);
                    before_ok && after_ok
                })
                .map(|r| r.0..r.1)
                .collect();

            if candidate_spans.is_empty() {
                continue;
            }

            filled_fragment = just_replace_string_multiple_times(
                &current_fragment_str,
                &candidate_spans
                    .iter()
                    .zip(vec![*replacement; candidate_spans.len()])
                    .map(|(span, s)| (span, s))
                    .collect::<Vec<_>>(),
            )
            .into_bytes();
        }

        String::from_utf8(filled_fragment).unwrap()
    }

    #[test]
    fn multi_placeholder_both_substituted() {
        // Fragment: "let (mut x, mut y) = foo;"
        // With placeholders x → "val1" and y → "val2", BOTH should appear in output.
        let result =
            apply_placeholders("let (mut x, mut y) = foo;", &[("x", "val1"), ("y", "val2")]);
        assert!(
            result.contains("val1"),
            "first placeholder substitution missing: {result}"
        );
        assert!(
            result.contains("val2"),
            "second placeholder substitution missing: {result}"
        );
        // The original identifiers should be gone
        assert!(
            !result.contains("mut x,"),
            "original 'x' placeholder still present: {result}"
        );
        assert!(
            !result.contains("mut y)"),
            "original 'y' placeholder still present: {result}"
        );
    }

    #[test]
    fn multi_placeholder_word_boundary_respected() {
        // 'x' should NOT be replaced inside 'Box' (word boundary check)
        let result = apply_placeholders("let x: Box<i32> = Box::new(x);", &[("x", "val")]);
        // standalone 'x' occurrences should be replaced; 'x' inside 'Box' should not
        assert!(
            !result.contains("Boval"),
            "word boundary check failed - replaced inside identifier: {result}"
        );
    }
}
