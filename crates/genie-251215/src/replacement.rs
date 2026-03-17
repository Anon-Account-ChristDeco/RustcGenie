use std::collections::HashMap;

use crate::code_structure::Span;
use crate::compatibility::{CompatibleKinds, CompatibleKindsPower};
use crate::range_utils;

type NodeKind = String;

fn choose_from_slice<'a, T>(rng: &mut fastrand::Rng, slice: &'a [T]) -> &'a T {
    &slice[rng.usize(..slice.len())]
}

fn gen_bool(rng: &mut fastrand::Rng, p: f64) -> bool {
    rng.f64() < p
}

pub fn just_replace_string(source_string: &str, span: &Span, target_string: &str) -> Option<String> {
    if span.start > span.end
        || span.end > source_string.len()
        || !source_string.is_char_boundary(span.start)
        || !source_string.is_char_boundary(span.end)
    {
        return None;
    }
    Some(format!(
        "{}{}{}",
        &source_string[0..span.start],
        target_string,
        &source_string[span.end..]
    ))
}

pub fn just_replace_string_multiple_times(
    source_string: &str,
    spans_and_target_strings: &[(&Span, &str)],
) -> String {
    // Filter out overlapping spans instead of asserting
    let mut spans_and_target_strings: Vec<(&Span, &str)> = spans_and_target_strings.to_vec();
    spans_and_target_strings.sort_by_key(|(span, _)| span.start);

    let mut filtered: Vec<(&Span, &str)> = Vec::with_capacity(spans_and_target_strings.len());
    for (span, target) in spans_and_target_strings.iter() {
        let overlaps = filtered
            .iter()
            .any(|(s, _)| range_utils::has_intersection_range(s, span));
        if !overlaps {
            filtered.push((span, target));
        }
    }
    let spans_and_target_strings = filtered;

    let mut result_string = source_string.to_string();
    let mut offset: isize = 0;

    for (span, target_string) in spans_and_target_strings.iter() {
        let mut adjusted_start: usize = std::cmp::min(
            std::cmp::max(0, (span.start as isize) + offset),
            result_string.len() as isize,
        )
        .try_into()
        .unwrap();
        let mut adjusted_end: usize = std::cmp::min(
            std::cmp::max(0, (span.end as isize) + offset),
            result_string.len() as isize,
        )
        .try_into()
        .unwrap();

        // Snap to nearest char boundaries
        while adjusted_start < result_string.len()
            && !result_string.is_char_boundary(adjusted_start)
        {
            adjusted_start = adjusted_start.saturating_sub(1);
        }
        while adjusted_end < result_string.len() && !result_string.is_char_boundary(adjusted_end) {
            adjusted_end = adjusted_end.saturating_add(1);
        }
        adjusted_end = std::cmp::min(adjusted_end, result_string.len());

        let end_string = if adjusted_end > result_string.len() {
            "".to_string()
        } else {
            result_string[adjusted_end..].to_string()
        };
        result_string = format!(
            "{}{}{}",
            &result_string[0..adjusted_start],
            target_string,
            &end_string
        );

        offset += target_string.len() as isize - (span.end as isize - span.start as isize);
    }

    result_string
}

/// Structured string replacement: picks a random location from the source structure,
/// finds compatible replacement candidates, and performs the replacement.
/// Returns (mutated_string, (replaced_span, source_kind), (target_kind, target_string, dependencies)).
pub fn structured_string_replacement<'a>(
    source_string: &str,
    source_string_structure: &[(Span, NodeKind)],
    target_candidates: &HashMap<NodeKind, Vec<(String, Vec<&'a str>)>>,
    rng: &mut fastrand::Rng,
) -> Option<(String, (Span, NodeKind), (NodeKind, String, Vec<&'a str>))> {
    if source_string_structure.is_empty() {
        return None;
    }

    let (span, node_kind) = choose_from_slice(rng, source_string_structure);

    let compatible_nodekinds = CompatibleKinds::new(node_kind);

    let filtered_target_candidates: Vec<(&NodeKind, &Vec<(String, Vec<&'a str>)>)> =
        target_candidates
            .iter()
            .filter(|(k, v)| compatible_nodekinds.contains(k) && !v.is_empty())
            .collect();

    if filtered_target_candidates.is_empty() {
        return None;
    }

    let (chosen_target_nodekind, last_candidates) =
        choose_from_slice(rng, &filtered_target_candidates);
    let chosen_target_string: &(String, Vec<&'a str>) = choose_from_slice(rng, last_candidates);

    let (mutated_string, curly_brace_added, semicolon_also_added): (String, bool, bool) = {
        if !source_string.is_char_boundary(span.start)
            || !source_string.is_char_boundary(span.start + 1)
            || !source_string.is_char_boundary(span.end.saturating_sub(1))
            || !source_string.is_char_boundary(span.end)
        {
            return None;
        }

        if span.end - span.start >= 2
            && &source_string[span.start..(span.start + 1)] == "{"
            && &source_string[(span.end - 1)..span.end] == "}"
            && gen_bool(rng, 0.5)
        {
            if gen_bool(rng, 0.5) {
                (
                    format!(
                        "{}{{{};}}{}",
                        &source_string[0..span.start],
                        chosen_target_string.0,
                        &source_string[span.end..]
                    ),
                    true,
                    true,
                )
            } else {
                (
                    format!(
                        "{}{{{}}}{}",
                        &source_string[0..span.start],
                        chosen_target_string.0,
                        &source_string[span.end..]
                    ),
                    true,
                    false,
                )
            }
        } else {
            (
                format!(
                    "{}{}{}",
                    &source_string[0..span.start],
                    chosen_target_string.0,
                    &source_string[span.end..]
                ),
                false,
                false,
            )
        }
    };

    assert!(!(!curly_brace_added && semicolon_also_added));

    match (curly_brace_added, semicolon_also_added) {
        (true, true) => {
            let target_string = {
                let mut s = chosen_target_string.0.clone();
                s.push(';');
                s
            };
            Some((
                mutated_string,
                ((span.start + 1)..(span.end - 1), node_kind.clone()),
                (
                    (*chosen_target_nodekind).clone(),
                    target_string,
                    chosen_target_string.1.clone(),
                ),
            ))
        }
        (true, false) => Some((
            mutated_string,
            ((span.start + 1)..(span.end - 1), node_kind.clone()),
            (
                (*chosen_target_nodekind).clone(),
                chosen_target_string.0.clone(),
                chosen_target_string.1.clone(),
            ),
        )),
        (false, false) => Some((
            mutated_string,
            (span.clone(), node_kind.clone()),
            (
                (*chosen_target_nodekind).clone(),
                chosen_target_string.0.clone(),
                chosen_target_string.1.clone(),
            ),
        )),
        (false, true) => unreachable!(),
    }
}

/// Filter a code structure to only include node kinds that appear in the target candidates
/// (or are compatible with them).
pub fn filter_structure_by_target_candidates_compatible_nodekinds(
    source_string_structure: &[(Span, NodeKind)],
    target_candidates: &HashMap<NodeKind, Vec<(String, Vec<&str>)>>,
) -> Vec<(Span, NodeKind)> {
    let appeared_nodekinds = target_candidates
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, _)| k.as_str());

    let power = CompatibleKindsPower::new(appeared_nodekinds);

    source_string_structure
        .iter()
        .filter(|(_, kind)| power.contains(kind))
        .cloned()
        .collect()
}

/// Replace multiple times, updating spans after each replacement and excluding
/// already-replaced spans from subsequent candidates.
pub fn structured_string_replacement_multiple_times<'a>(
    source_string: &str,
    source_string_structure: &[(Span, NodeKind)],
    target_candidates: &HashMap<NodeKind, Vec<(String, Vec<&'a str>)>>,
    times: usize,
    rng: &mut fastrand::Rng,
) -> (
    String,
    Vec<((Span, NodeKind), (NodeKind, String, Vec<&'a str>))>,
) {
    let mut source_string_structure = source_string_structure.to_vec();
    let mut result: String = source_string.to_string();
    let mut mutations: Vec<((Span, NodeKind), (NodeKind, String, Vec<&'a str>))> = Vec::new();

    for _ in 0..times {
        if let Some((
            result_string,
            (span, source_node_kind),
            (target_node_kind, target_string, target_dependencies),
        )) = structured_string_replacement(
            &result,
            &source_string_structure,
            target_candidates,
            rng,
        ) {
            // Exclude already replaced spans
            source_string_structure
                .retain(|(s, _)| !range_utils::has_intersection_range(s, &span));

            // Update span offsets
            let edited_index: isize =
                (span.start as isize - span.end as isize) + target_string.len() as isize;
            for (s, _) in source_string_structure.iter_mut() {
                if s.start >= span.start {
                    if edited_index >= 0 {
                        let d = edited_index as usize;
                        s.start = s.start.saturating_add(d);
                        s.end = s.end.saturating_add(d);
                    } else {
                        let d = (-edited_index) as usize;
                        s.start = s.start.saturating_sub(d);
                        s.end = s.end.saturating_sub(d);
                    }
                }
            }

            result = result_string;
            mutations.push((
                (span.clone(), source_node_kind),
                (target_node_kind, target_string, target_dependencies),
            ));
        }
    }
    (result, mutations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn just_replace_string_basic() {
        let s = "hello world";
        let result = just_replace_string(s, &(6..11), "rust");
        assert_eq!(result, Some("hello rust".to_string()));
    }

    #[test]
    fn just_replace_multiple() {
        let s = "aaa bbb ccc";
        let result =
            just_replace_string_multiple_times(s, &[(&(0..3), "XXX"), (&(8..11), "YYY")]);
        assert_eq!(result, "XXX bbb YYY");
    }
}
