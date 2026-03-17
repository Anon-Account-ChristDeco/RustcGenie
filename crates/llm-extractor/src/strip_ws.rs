/// Find `needle` inside `haystack` with **all whitespace stripped** from both sides,
/// but return indices referring to the original `haystack` bytes.
///
/// Returns `Some((start_byte, end_exclusive))` so you can slice `&haystack[start_byte..end_exclusive]`.
/// If the normalized needle is empty or not found, returns `None`.
pub fn find_strip_whitespace(
    haystack: &str,
    needle: &str,
    remove_comma: bool,
) -> Option<(usize, usize)> {
    let (h_chars, h_starts, h_ends) = build_stripped(haystack, remove_comma);
    let (n_chars, _n_starts, _n_ends) = build_stripped(needle, remove_comma);

    if n_chars.is_empty() {
        return None;
    }
    if n_chars.len() > h_chars.len() {
        return None;
    }

    if let Some(idx) = kmp_search(&h_chars, &n_chars) {
        let start_byte = h_starts[idx];
        let last = idx + n_chars.len() - 1;
        let end_byte_excl = h_ends[last];
        return Some((start_byte, end_byte_excl));
    }

    None
}

pub fn find_strip_whitespace_starts_from(
    haystack: &str,
    needle: &str,
    remove_comma: bool,
    start_pos: usize,
) -> Option<(usize, usize)> {
    let (h_chars, h_starts, h_ends) = build_stripped(haystack, remove_comma);
    let (n_chars, _n_starts, _n_ends) = build_stripped(needle, remove_comma);

    if n_chars.is_empty() {
        return None;
    }
    if n_chars.len() > h_chars.len() {
        return None;
    }

    // Find the starting index in the stripped haystack that corresponds to start_pos in the original haystack
    let mut start_idx = start_pos;
    for (i, &start_byte) in h_starts.iter().enumerate() {
        if start_byte >= start_pos {
            start_idx = i;
            break;
        }
    }

    if start_idx >= h_chars.len() {
        return None;
    }

    if let Some(idx) = kmp_search(&h_chars[start_idx..], &n_chars) {
        let actual_idx = start_idx + idx;
        let start_byte = h_starts[actual_idx];
        let last = actual_idx + n_chars.len() - 1;
        let end_byte_excl = h_ends[last];
        return Some((start_byte, end_byte_excl));
    }

    None
}

pub fn find_all_strip_whitespace(
    haystack: &str,
    needle: &str,
    remove_comma: bool,
    upper_bound: usize,
) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    let mut search_start = 0;

    let mut count = 0;

    while let Some((start_byte, end_byte_excl)) =
        find_strip_whitespace_starts_from(haystack, needle, remove_comma, search_start)
    {
        count += 1;
        if upper_bound > 0 && count >= upper_bound {
            break;
        }

        results.push((start_byte, end_byte_excl));
        search_start = end_byte_excl;
    }

    results
}

pub fn find_all_strip_whitespace_starts_from(
    haystack: &str,
    needle: &str,
    remove_comma: bool,
    start_pos: usize,
    upper_bound: usize,
) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    let mut search_start = start_pos;

    let mut count = 0;

    while let Some((start_byte, end_byte_excl)) =
        find_strip_whitespace_starts_from(haystack, needle, remove_comma, search_start)
    {
        count += 1;
        if upper_bound > 0 && count >= upper_bound {
            break;
        }

        results.push((start_byte, end_byte_excl));
        search_start = end_byte_excl;
    }

    results
}

/// Build stripped char vector and mapping arrays:
/// - chars: `Vec<char>` with all whitespace (and optionally commas) removed
/// - starts: for each char, original byte start index
/// - ends: for each char, original byte end-exclusive index
fn build_stripped(s: &str, remove_comma: bool) -> (Vec<char>, Vec<usize>, Vec<usize>) {
    let mut chars = Vec::new();
    let mut starts = Vec::new();
    let mut ends = Vec::new();

    for (byte_idx, ch) in s.char_indices() {
        if ch.is_whitespace() || (remove_comma && ch == ',') {
            continue;
        }
        let end = byte_idx + ch.len_utf8();
        chars.push(ch);
        starts.push(byte_idx);
        ends.push(end);
    }

    (chars, starts, ends)
}

/// KMP prefix function for `Vec<char>`
fn kmp_prefix(pat: &[char]) -> Vec<usize> {
    let m = pat.len();
    let mut pi = vec![0; m];
    let mut k = 0usize;
    for q in 1..m {
        while k > 0 && pat[k] != pat[q] {
            k = pi[k - 1];
        }
        if pat[k] == pat[q] {
            k += 1;
        }
        pi[q] = k;
    }
    pi
}

/// KMP search: returns first match index in `text` as normalized-char index
fn kmp_search(text: &[char], pat: &[char]) -> Option<usize> {
    let n = text.len();
    let m = pat.len();
    if m == 0 {
        return Some(0);
    }
    let pi = kmp_prefix(pat);
    let mut q = 0usize;
    for i in 0..n {
        while q > 0 && pat[q] != text[i] {
            q = pi[q - 1];
        }
        if pat[q] == text[i] {
            q += 1;
        }
        if q == m {
            return Some(i + 1 - m);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_strip_match() {
        let hay = "fn foo() {\n    let x = vec![\n        1,\n        2,\n        3,\n    ];\n    println!(\"{}\", x[0]);\n}";
        let needle = "let x = vec![1,2,3,];";
        let found = find_strip_whitespace(hay, needle, false).expect("should find");
        assert_eq!(
            &hay[found.0..found.1],
            "let x = vec![\n        1,\n        2,\n        3,\n    ];"
        );
    }

    #[test]
    fn no_match_returns_none() {
        let hay = "let a = 10;";
        let needle = "let b = 20;";
        assert!(find_strip_whitespace(hay, needle, false).is_none());
    }

    #[test]
    fn unicode_and_whitespace() {
        let hay = "α β\tγ\nδ";
        let needle = "αβγδ";
        let found =
            find_strip_whitespace(hay, needle, false).expect("should find unicode sequence");
        assert_eq!(&hay[found.0..found.1], "α β\tγ\nδ");
    }

    #[test]
    fn empty_normalized_needle() {
        let hay = "abc";
        let needle = "   \n\t ";
        assert!(find_strip_whitespace(hay, needle, false).is_none());
    }

    #[test]
    fn find_with_comma_removal() {
        let hay = "foo(a, b, c)";
        let needle = "foo(a b c)";
        let found = find_strip_whitespace(hay, needle, true).expect("should find with comma removal");
        assert_eq!(&hay[found.0..found.1], "foo(a, b, c)");
    }

    #[test]
    fn find_all_occurrences() {
        let hay = "let x = 1; let x = 2; let x = 3;";
        let needle = "let x";
        let results = find_all_strip_whitespace(hay, needle, false, 10);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn find_starts_from() {
        let hay = "aaa bbb aaa";
        let needle = "aaa";
        let first = find_strip_whitespace(hay, needle, false).expect("should find first");
        let second = find_strip_whitespace_starts_from(hay, needle, false, first.1)
            .expect("should find second");
        assert!(second.0 > first.0);
    }
}
