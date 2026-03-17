use std::cmp::{max, min};
use std::ops::Range;

pub fn intersection_range<T: Ord + Clone>(a: &Range<T>, b: &Range<T>) -> Option<Range<T>> {
    let s = max(a.start.clone(), b.start.clone());
    let e = min(a.end.clone(), b.end.clone());
    if s < e { Some(s..e) } else { None }
}

pub fn has_intersection_range<T: Ord + Clone>(a: &Range<T>, b: &Range<T>) -> bool {
    intersection_range(a, b).is_some()
}

pub fn is_contained_range<T: Ord + Clone>(inner: &Range<T>, outer: &Range<T>) -> bool {
    inner.start >= outer.start && inner.end <= outer.end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_overlapping() {
        assert_eq!(intersection_range(&(0..5), &(3..8)), Some(3..5));
    }

    #[test]
    fn intersection_disjoint() {
        assert_eq!(intersection_range(&(0..3), &(5..8)), None);
    }

    #[test]
    fn intersection_touching() {
        assert_eq!(intersection_range(&(0..5), &(5..8)), None);
    }

    #[test]
    fn has_intersection() {
        assert!(has_intersection_range(&(0..5), &(3..8)));
        assert!(!has_intersection_range(&(0..3), &(5..8)));
    }

    #[test]
    fn contained() {
        assert!(is_contained_range(&(2..4), &(0..5)));
        assert!(!is_contained_range(&(0..6), &(1..5)));
    }
}
