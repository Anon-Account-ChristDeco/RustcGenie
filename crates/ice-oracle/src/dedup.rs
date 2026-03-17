use std::collections::HashSet;

use crate::result::IceInfo;

/// Deduplicate by location only, preserving first-seen order.
///
/// Two ICEs are considered duplicates when their `location` matches,
/// regardless of `reason`.
pub fn deduplicate(ices: &[IceInfo]) -> Vec<IceInfo> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for ice in ices {
        if seen.insert(ice.location.clone()) {
            result.push(ice.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_preserves_order() {
        let a = IceInfo {
            location: "a.rs:1".into(),
            reason: "bug".into(),
        };
        let b = IceInfo {
            location: "b.rs:2".into(),
            reason: "oops".into(),
        };
        let input = vec![a.clone(), b.clone(), a.clone()];
        let result = deduplicate(&input);
        assert_eq!(result, vec![a, b]);
    }

    #[test]
    fn test_dedup_same_location_different_reason() {
        let a = IceInfo {
            location: "a.rs:1".into(),
            reason: "bug".into(),
        };
        let a2 = IceInfo {
            location: "a.rs:1".into(),
            reason: "different reason".into(),
        };
        let result = deduplicate(&[a.clone(), a2]);
        assert_eq!(result, vec![a]);
    }

    #[test]
    fn test_dedup_empty() {
        let result = deduplicate(&[]);
        assert!(result.is_empty());
    }
}
