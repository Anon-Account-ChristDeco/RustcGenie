use std::sync::LazyLock;

use regex::Regex;

use crate::result::IceInfo;

static SINGLE_LINE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // thread 'rustc' panicked at <file>: <message>
        // thread 'rustc' (1234) panicked at <file>: <message>
        Regex::new(r"^thread 'rustc'(?: \(([0-9]+)\))? panicked at (.*?): (.*)$").unwrap(),
        // error: internal compiler error: <file>: <message>
        Regex::new(r"^error: internal compiler error: (.*?): (.*)$").unwrap(),
        // note: delayed at <file:line> <message>
        Regex::new(r"^note: delayed at (.*:\d+)(.*)$").unwrap(),
    ]
});

static MULTI_LINE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    // thread 'rustc' panicked at <file>:
    // <message on next line>
    vec![Regex::new(r"^thread 'rustc'(?: \(([0-9]+)\))? panicked at (.*?):$").unwrap()]
});

/// Extract ICE messages from rustc stderr output.
///
/// Returns a deduplicated list of `IceInfo` values found via known ICE patterns.
pub fn extract_ice_messages(stderr: &str) -> Vec<IceInfo> {
    let patterns = &*SINGLE_LINE_PATTERNS;
    let mut results: Vec<IceInfo> = Vec::new();

    // Single-line patterns
    for line in stderr.lines() {
        for re in patterns {
            if let Some(captures) = re.captures(line) {
                // If group 3 exists, this is the first pattern: (digits?), file = group 2, msg = group 3
                // Otherwise, use groups 1 and 2.
                let (file_cap, msg_cap) = if captures.get(3).is_some() {
                    (captures.get(2), captures.get(3))
                } else {
                    (captures.get(1), captures.get(2))
                };

                if let (Some(file), Some(msg)) = (file_cap, msg_cap) {
                    let info = IceInfo {
                        location: file.as_str().to_string(),
                        reason: msg.as_str().to_string(),
                    };
                    if !results.contains(&info) {
                        results.push(info);
                    }
                    break; // stop testing other patterns for this line
                }
            }
        }
    }

    // Multi-line patterns
    let multi_patterns = &*MULTI_LINE_PATTERNS;
    let lines: Vec<&str> = stderr.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        for re in multi_patterns {
            if let Some(captures) = re.captures(lines[i]) {
                if let Some(file) = captures.get(2) {
                    if i + 1 < lines.len() {
                        let m = lines[i + 1].trim();
                        if m.is_empty() {
                            continue;
                        }
                        let info = IceInfo {
                            location: file.as_str().to_string(),
                            reason: m.to_string(),
                        };
                        if !results.contains(&info) {
                            results.push(info);
                        }
                        i += 1; // skip the message line
                    }
                }
            }
        }
        i += 1;
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panic_with_thread_id() {
        let stderr = "thread 'rustc' (1234) panicked at compiler/rustc_middle/src/ty/typeck_results.rs:593:9: Box<dyn Any>";
        let got = extract_ice_messages(stderr);
        assert_eq!(
            got,
            vec![IceInfo {
                location: "compiler/rustc_middle/src/ty/typeck_results.rs:593:9".to_string(),
                reason: "Box<dyn Any>".to_string(),
            }]
        );
    }

    #[test]
    fn test_panic_without_thread_id() {
        let stderr = "thread 'rustc' panicked at compiler/rustc_middle/src/ty/typeck_results.rs:593:9: Box<dyn Any>";
        let got = extract_ice_messages(stderr);
        assert_eq!(
            got,
            vec![IceInfo {
                location: "compiler/rustc_middle/src/ty/typeck_results.rs:593:9".to_string(),
                reason: "Box<dyn Any>".to_string(),
            }]
        );
    }

    #[test]
    fn test_panic_two_lines() {
        let stderr = "\
thread 'rustc' panicked at compiler/rustc_middle/src/ty/typeck_results.rs:593:9:
Box<dyn Any>
";
        let got = extract_ice_messages(stderr);
        assert_eq!(
            got,
            vec![IceInfo {
                location: "compiler/rustc_middle/src/ty/typeck_results.rs:593:9".to_string(),
                reason: "Box<dyn Any>".to_string(),
            }]
        );
    }

    #[test]
    fn test_ice_same_line() {
        let stderr = "error: internal compiler error: compiler/rustc_hir_typeck/src/pat.rs:689:21: FIXME(deref_patterns): adjust mode unimplemented for ConstBlock(...)";
        let got = extract_ice_messages(stderr);
        assert_eq!(
            got,
            vec![IceInfo {
                location: "compiler/rustc_hir_typeck/src/pat.rs:689:21".to_string(),
                reason: "FIXME(deref_patterns): adjust mode unimplemented for ConstBlock(...)"
                    .to_string(),
            }]
        );
    }

    #[test]
    fn test_mix() {
        let stderr = "\
thread 'rustc' panicked at compiler/rustc_middle/src/ty/typeck_results.rs:593:9:
Box<dyn Any>
error: internal compiler error: compiler/rustc_hir_typeck/src/pat.rs:689:21: FIXME(deref_patterns): adjust mode unimplemented for ConstBlock(...)
";
        let got = extract_ice_messages(stderr);
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn test_mix_with_thread_id() {
        let stderr = "\
thread 'rustc' (0981234) panicked at compiler/rustc_middle/src/ty/typeck_results.rs:593:9:
Box<dyn Any>
error: internal compiler error: compiler/rustc_hir_typeck/src/pat.rs:689:21: FIXME(deref_patterns): adjust mode unimplemented for ConstBlock(...)
";
        let got = extract_ice_messages(stderr);
        assert_eq!(got.len(), 2);
    }
}
