use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use llm_extractor::annotate::{AnnotatedFragment, AnnotatedLlmOutput};
use llm_extractor::strip_ws::find_all_strip_whitespace;

use crate::code_structure::{get_nodekind_strings, CodeStructures, NodeKind};
use crate::range_utils;

pub type DependencyIndex = usize;
pub type DependencySet = HashMap<String, DependencyIndex>;
pub type AllDependencies = Vec<String>;

#[derive(Clone, Debug)]
pub struct Snippet {
    pub filename: PathBuf,
    pub kind: Option<String>,
    pub fragment: Vec<u8>,
    pub dependencies: Vec<DependencyIndex>,
    pub placeholders: Vec<(String, Option<NodeKind>)>,
}

pub struct AllSnippets {
    pub snip_from_source: HashMap<String, Vec<Snippet>>,
    pub snip_from_fragments: HashMap<String, Vec<Snippet>>,
}

/// Load annotated LLM outputs from JSON files.
pub fn load_annotated_outputs(files: &[PathBuf]) -> HashMap<PathBuf, AnnotatedLlmOutput> {
    let mut map = HashMap::new();
    let mut read_fail = 0usize;
    let mut parse_fail = 0usize;
    for file in files {
        let data = match std::fs::read_to_string(file) {
            Ok(d) => d,
            Err(_) => { read_fail += 1; continue; }
        };
        let output: AnnotatedLlmOutput = match serde_json::from_str(&data) {
            Ok(o) => o,
            Err(_) => { parse_fail += 1; continue; }
        };
        map.insert(file.clone(), output);
    }
    if read_fail > 0 || parse_fail > 0 {
        eprintln!(
            "load_annotated_outputs: {} loaded, {} read failures, {} parse failures (of {} total)",
            map.len(), read_fail, parse_fail, files.len(),
        );
    }
    map
}

/// Remove "main" fragments and "fn main" dependencies.
pub fn filter_fragments(
    fragments_set: Vec<(PathBuf, Vec<AnnotatedFragment>)>,
) -> Vec<(PathBuf, Vec<AnnotatedFragment>)> {
    let mut filtered = Vec::with_capacity(fragments_set.len());
    for (filename, mut fragments) in fragments_set {
        fragments.retain(|frag| !frag.fragment.contains("main"));
        fragments.iter_mut().for_each(|frag| {
            frag.dependencies.retain(|dep| !dep.contains("fn main"));
        });
        filtered.push((filename, fragments));
    }
    filtered
}

/// Collect all snippets from seed files and fragment files.
///
/// This is the main collection loop, ported from the original genie main function.
pub fn collect_all_snippets(
    seed_files: &[PathBuf],
    seed_structures: &CodeStructures,
    fragments_hashmap: &HashMap<PathBuf, Vec<AnnotatedFragment>>,
    seeds_dir: &PathBuf,
    ingredients_dir: &PathBuf,
) -> (AllSnippets, AllDependencies) {
    let mut dependency_set: DependencySet = HashMap::new();
    let mut all_dependencies: AllDependencies = Vec::new();
    let mut snippet_kind_counter_src: HashMap<String, usize> = HashMap::new();
    let mut snippet_kind_counter_frag: HashMap<String, usize> = HashMap::new();

    fn counter_up(map: &mut HashMap<String, usize>, key: &str) {
        *map.entry(key.to_string()).or_insert(0) += 1;
    }

    fn convert_path_from_rs_to_json(
        p: &PathBuf,
        base_from: &PathBuf,
        base_to: &PathBuf,
    ) -> Option<PathBuf> {
        let relative_path = p.strip_prefix(base_from).ok()?;
        Some(base_to.join(relative_path).with_extension("json"))
    }

    let mut snippets_from_source: Vec<Snippet> = Vec::with_capacity(seed_files.len() * 20);
    let mut snippets_from_fragments: Vec<Snippet> = Vec::with_capacity(seed_files.len() * 8);
    let mut synced_ingredient_files = 0usize;

    for file in seed_files.iter() {
        let related_fragment_filename =
            match convert_path_from_rs_to_json(file, seeds_dir, ingredients_dir) {
                Some(p) => p,
                None => continue,
            };

        let related_fragments: Vec<AnnotatedFragment> = fragments_hashmap
            .get(&related_fragment_filename)
            .unwrap_or(&Vec::new())
            .clone();

        if !related_fragments.is_empty() {
            synced_ingredient_files += 1;
        }

        // Get fragment spans in the seed source code
        struct FragmentSpan {
            fragment: usize,
            span: (usize, usize),
        }

        let seed_source_code: Vec<u8> = std::fs::read(file).unwrap();
        let seed_source_code_str = String::from_utf8_lossy(&seed_source_code);

        let mut fragment_spans: Vec<FragmentSpan> =
            Vec::with_capacity(related_fragments.len() * 2);
        for (i, fragment) in related_fragments.iter().enumerate() {
            let ranges =
                find_all_strip_whitespace(&seed_source_code_str, &fragment.fragment, true, 10);
            for range in ranges {
                fragment_spans.push(FragmentSpan {
                    fragment: i,
                    span: (range.0, range.1),
                });
            }
        }

        // For every node in the file's code structure, build snippets from source
        let mut plain_snippets: HashSet<(String, Vec<u8>)> = HashSet::new();
        let file_code_structure = match seed_structures.get(file) {
            Some(cs) => cs,
            None => continue,
        };

        for (span, nodekind) in file_code_structure.iter() {
            let mut assigned_dependencies: Vec<DependencyIndex> = Vec::new();
            let mut assigned_placeholders: Vec<(String, Option<String>)> = Vec::new();

            for fragment_span in fragment_spans.iter() {
                let fragment_range = fragment_span.span.0..fragment_span.span.1;
                if range_utils::has_intersection_range(span, &fragment_range) {
                    let fragment: &AnnotatedFragment = &related_fragments[fragment_span.fragment];

                    for fragment_dependency in fragment.dependencies.iter() {
                        let dep_index: DependencyIndex =
                            if let Some(&index) = dependency_set.get(fragment_dependency) {
                                index
                            } else {
                                let new_index = all_dependencies.len();
                                all_dependencies.push(fragment_dependency.clone());
                                dependency_set.insert(fragment_dependency.clone(), new_index);
                                new_index
                            };
                        assigned_dependencies.push(dep_index);
                    }

                    for fragment_placeholder in fragment.placeholders.iter() {
                        assigned_placeholders.push((
                            fragment_placeholder.placeholder.clone(),
                            fragment_placeholder.node_kind.clone(),
                        ));
                    }
                }
            }

            assigned_dependencies.sort();
            assigned_dependencies.dedup();
            assigned_placeholders.sort();
            assigned_placeholders.dedup();

            // Check duplicate plain snippets (no deps/placeholders)
            if assigned_dependencies.is_empty() && assigned_placeholders.is_empty() {
                let snippet_code: Vec<u8> = seed_source_code[span.clone()].to_vec();
                if plain_snippets.contains(&(nodekind.clone(), snippet_code.clone())) {
                    continue;
                } else {
                    plain_snippets.insert((nodekind.clone(), snippet_code));
                }
            }

            let snippet = Snippet {
                filename: file.clone(),
                kind: Some(nodekind.clone()),
                fragment: seed_source_code[span.clone()].to_vec(),
                dependencies: assigned_dependencies,
                placeholders: assigned_placeholders,
            };
            for k in get_nodekind_strings(snippet.kind.as_deref()).iter() {
                counter_up(&mut snippet_kind_counter_src, k);
            }
            snippets_from_source.push(snippet);
        }

        // For every fragment, add it to snippets_from_fragments
        for fragment in related_fragments.iter() {
            let kind: Option<String> = fragment.node_kind.clone();
            let dependencies: Vec<DependencyIndex> = fragment
                .dependencies
                .iter()
                .map(|dep| {
                    if let Some(&index) = dependency_set.get(dep) {
                        index
                    } else {
                        let new_index = all_dependencies.len();
                        all_dependencies.push(dep.clone());
                        dependency_set.insert(dep.clone(), new_index);
                        new_index
                    }
                })
                .collect();
            let placeholders: Vec<(String, Option<String>)> = fragment
                .placeholders
                .iter()
                .map(|ph| (ph.placeholder.clone(), ph.node_kind.clone()))
                .collect();
            let snippet = Snippet {
                filename: related_fragment_filename.clone(),
                kind: kind.clone(),
                fragment: fragment.fragment.as_bytes().to_vec(),
                dependencies,
                placeholders,
            };
            for k in get_nodekind_strings(kind.as_deref()).iter() {
                counter_up(&mut snippet_kind_counter_frag, k);
            }
            snippets_from_fragments.push(snippet);
        }
    }

    // Build AllSnippets maps
    let mut snip_from_source: HashMap<String, Vec<Snippet>> = HashMap::new();
    let mut snip_from_fragments: HashMap<String, Vec<Snippet>> = HashMap::new();

    for (kind, count) in snippet_kind_counter_src.iter() {
        snip_from_source.insert(kind.clone(), Vec::with_capacity(*count));
    }
    for (kind, count) in snippet_kind_counter_frag.iter() {
        snip_from_fragments.insert(kind.clone(), Vec::with_capacity(*count));
    }

    for snippet in snippets_from_source {
        let snippet_kind_strings = get_nodekind_strings(snippet.kind.as_deref());
        for kind_str in snippet_kind_strings.iter() {
            snip_from_source
                .get_mut(kind_str)
                .unwrap()
                .push(snippet.clone());
        }
    }
    for snippet in snippets_from_fragments {
        let snippet_kind_strings = get_nodekind_strings(snippet.kind.as_deref());
        for kind_str in snippet_kind_strings.iter() {
            snip_from_fragments
                .get_mut(kind_str)
                .unwrap()
                .push(snippet.clone());
        }
    }

    eprintln!(
        "ingredients: {}/{} seed files have matching ingredient files",
        synced_ingredient_files,
        seed_files.len(),
    );

    let all_snippets = AllSnippets {
        snip_from_source,
        snip_from_fragments,
    };

    (all_snippets, all_dependencies)
}
