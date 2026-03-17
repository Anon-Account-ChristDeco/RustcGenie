use std::collections::HashSet;

use component_extractor::{find_kind_family, KindFamily, KindFamilyFindResult};

/// Check whether two node kind names share any family.
pub fn is_compatible(a: &str, b: &str) -> bool {
    let a_fams = find_kind_family(a);
    let b_fams = find_kind_family(b);
    match (&a_fams, &b_fams) {
        (KindFamilyFindResult::Unknown, _) | (_, KindFamilyFindResult::Unknown) => a == b,
        (KindFamilyFindResult::Unique(af), KindFamilyFindResult::Unique(bf)) => af == bf,
        (KindFamilyFindResult::Unique(af), KindFamilyFindResult::Ambiguous(bfs)) => {
            bfs.contains(af)
        }
        (KindFamilyFindResult::Ambiguous(afs), KindFamilyFindResult::Unique(bf)) => {
            afs.contains(bf)
        }
        (KindFamilyFindResult::Ambiguous(afs), KindFamilyFindResult::Ambiguous(bfs)) => {
            afs.iter().any(|af| bfs.contains(af))
        }
    }
}

/// Compatible kinds for a single node kind (by name).
pub enum CompatibleKinds {
    Families(Vec<KindFamily>),
    Orphan(String),
}

impl CompatibleKinds {
    pub fn new(name: &str) -> Self {
        match find_kind_family(name) {
            KindFamilyFindResult::Unique(f) => CompatibleKinds::Families(vec![f]),
            KindFamilyFindResult::Ambiguous(fs) => CompatibleKinds::Families(fs),
            KindFamilyFindResult::Unknown => CompatibleKinds::Orphan(name.to_string()),
        }
    }

    pub fn contains(&self, kind: &str) -> bool {
        match (self, CompatibleKinds::new(kind)) {
            (CompatibleKinds::Families(a_fams), CompatibleKinds::Families(b_fams)) => {
                a_fams.iter().any(|af| b_fams.contains(af))
            }
            (CompatibleKinds::Orphan(a), CompatibleKinds::Orphan(b)) => *a == b,
            _ => false,
        }
    }
}

/// Union of compatible kinds across multiple node kinds.
pub struct CompatibleKindsPower {
    pub families: HashSet<KindFamily>,
    pub orphans: HashSet<String>,
}

impl CompatibleKindsPower {
    pub fn new<'a>(nodekinds: impl Iterator<Item = &'a str>) -> Self {
        let mut families = HashSet::new();
        let mut orphans = HashSet::new();

        for name in nodekinds {
            match CompatibleKinds::new(name) {
                CompatibleKinds::Families(fams) => families.extend(fams),
                CompatibleKinds::Orphan(ork) => {
                    orphans.insert(ork);
                }
            }
        }

        CompatibleKindsPower { families, orphans }
    }

    pub fn contains(&self, kind: &str) -> bool {
        match CompatibleKinds::new(kind) {
            CompatibleKinds::Families(fams) => fams.iter().any(|f| self.families.contains(f)),
            CompatibleKinds::Orphan(ork) => self.orphans.contains(&ork),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatible_expr_kinds() {
        assert!(is_compatible("array_expression", "call_expression"));
    }

    #[test]
    fn incompatible_kinds() {
        assert!(!is_compatible("array_type", "array_expression"));
    }

    #[test]
    fn compatible_kinds_power_contains() {
        let power = CompatibleKindsPower::new(["array_expression"].into_iter());
        assert!(power.contains("call_expression"));
    }
}
