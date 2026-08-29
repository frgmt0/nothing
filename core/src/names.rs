use std::sync::Arc;

use crate::exp::Id;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct NameTable {
    layer: im::HashMap<Id, String>,
    base: Option<Arc<NameTable>>,
}

impl NameTable {
    pub fn new() -> NameTable {
        NameTable::default()
    }

    pub fn overlay(base: &NameTable) -> NameTable {
        NameTable {
            layer: im::HashMap::new(),
            base: Some(Arc::new(base.clone())),
        }
    }

    pub fn base(&self) -> Option<&NameTable> {
        self.base.as_deref()
    }

    pub fn depth(&self) -> usize {
        1 + self.base.as_deref().map_or(0, NameTable::depth)
    }

    pub fn set(&mut self, id: Id, name: impl Into<String>) {
        self.layer.insert(id, name.into());
    }

    pub fn rename(&mut self, id: Id, name: impl Into<String>) {
        self.set(id, name);
    }

    pub fn with(&self, id: Id, name: impl Into<String>) -> NameTable {
        let mut next = self.clone();
        next.set(id, name);
        next
    }

    pub fn get(&self, id: Id) -> Option<&str> {
        match self.layer.get(&id) {
            Some(name) => Some(name.as_str()),
            None => self.base.as_deref().and_then(|base| base.get(id)),
        }
    }

    pub fn get_own(&self, id: Id) -> Option<&str> {
        self.layer.get(&id).map(String::as_str)
    }

    pub fn contains(&self, id: Id) -> bool {
        self.get(id).is_some()
    }

    pub fn display(&self, id: Id) -> String {
        match self.get(id) {
            Some(name) => name.to_string(),
            None => format!("_{}", id.short()),
        }
    }

    pub fn ids(&self) -> Vec<Id> {
        let mut out: Vec<Id> = match self.base.as_deref() {
            Some(base) => base.ids(),
            None => Vec::new(),
        };
        for id in self.layer.keys() {
            if !out.contains(id) {
                out.push(*id);
            }
        }
        out
    }

    pub fn entries(&self) -> Vec<(Id, String)> {
        self.ids()
            .into_iter()
            .map(|id| (id, self.display(id)))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.ids().len()
    }

    pub fn is_empty(&self) -> bool {
        self.layer.is_empty() && self.base.as_deref().is_none_or(NameTable::is_empty)
    }

    pub fn flatten(&self) -> NameTable {
        let mut flat = NameTable::new();
        for (id, name) in self.entries() {
            flat.set(id, name);
        }
        flat
    }

    pub fn holds_name(&self, name: &str) -> bool {
        self.ids().into_iter().any(|id| self.get(id) == Some(name))
    }

    pub fn named(&self, name: &str) -> Vec<Id> {
        self.ids()
            .into_iter()
            .filter(|id| self.get(*id) == Some(name))
            .collect()
    }
}

pub fn fresh_definition_name(names: &NameTable) -> String {
    (0u64..)
        .map(|n| {
            if n == 0 {
                "def".to_string()
            } else {
                format!("def{n}")
            }
        })
        .find(|candidate| !names.holds_name(candidate))
        .expect("the candidate stream is unbounded")
}

pub fn fresh_binder_name(names: &NameTable) -> String {
    (0u64..)
        .map(|n| format!("x{n}"))
        .find(|candidate| !names.holds_name(candidate))
        .expect("the candidate stream is unbounded")
}

pub fn fresh_field_name(names: &NameTable) -> String {
    (0u64..)
        .map(|n| format!("f{n}"))
        .find(|candidate| !names.holds_name(candidate))
        .expect("the candidate stream is unbounded")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> (Id, Id, Id) {
        (
            Id::from_u128(0xA1),
            Id::from_u128(0xB2),
            Id::from_u128(0xC3),
        )
    }

    #[test]
    fn an_empty_table_names_nothing_but_still_displays_an_id() {
        let (a, _, _) = ids();
        let names = NameTable::new();
        assert_eq!(names.get(a), None);
        assert!(names.is_empty());
        assert_eq!(names.display(a), format!("_{}", a.short()));
    }

    #[test]
    fn a_name_round_trips() {
        let (a, b, _) = ids();
        let mut names = NameTable::new();
        names.set(a, "xs");
        names.set(b, "total");
        assert_eq!(names.get(a), Some("xs"));
        assert_eq!(names.display(b), "total");
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn renaming_is_a_write_that_cannot_fail() {
        let (a, _, _) = ids();
        let mut names = NameTable::new();
        names.rename(a, "xs");
        names.rename(a, "items");
        names.rename(a, "items");
        assert_eq!(names.get(a), Some("items"));
        assert_eq!(names.len(), 1, "renaming never adds an entry");
    }

    #[test]
    fn two_ids_may_share_one_display_name() {
        let (a, b, _) = ids();
        let mut names = NameTable::new();
        names.set(a, "x");
        names.set(b, "x");
        assert_eq!(names.display(a), names.display(b));
        assert_ne!(a, b);
        let mut shared = names.named("x");
        shared.sort();
        let mut both = vec![a, b];
        both.sort();
        assert_eq!(shared, both);
    }

    #[test]
    fn an_overlay_shadows_its_base_without_touching_it() {
        let (a, b, _) = ids();
        let mut base = NameTable::new();
        base.set(a, "xs");
        base.set(b, "n");

        let mut mine = NameTable::overlay(&base);
        mine.set(a, "items");

        assert_eq!(mine.get(a), Some("items"));
        assert_eq!(mine.get(b), Some("n"), "unshadowed names fall through");
        assert_eq!(base.get(a), Some("xs"), "the base is untouched");
        assert_eq!(mine.get_own(b), None, "b is not in the overlay's own layer");
        assert_eq!(mine.depth(), 2);
        assert_eq!(mine.base().and_then(|b| b.get(a)), Some("xs"));
    }

    #[test]
    fn flattening_an_overlay_preserves_every_visible_name() {
        let (a, b, c) = ids();
        let mut base = NameTable::new();
        base.set(a, "xs");
        base.set(b, "n");
        let mut mine = NameTable::overlay(&base);
        mine.set(a, "items");
        mine.set(c, "acc");

        let flat = mine.flatten();
        assert_eq!(flat.depth(), 1);
        for id in [a, b, c] {
            assert_eq!(flat.display(id), mine.display(id));
        }
        assert_eq!(flat.len(), 3);
    }

    #[test]
    fn a_fresh_binder_name_avoids_every_name_in_the_table() {
        let (a, b, _) = ids();
        let mut names = NameTable::new();
        assert_eq!(fresh_binder_name(&names), "x0");
        names.set(a, "x0");
        assert_eq!(fresh_binder_name(&names), "x1");
        names.set(b, "x1");
        assert_eq!(fresh_binder_name(&names), "x2");

        let overlay = NameTable::overlay(&names);
        assert_eq!(
            fresh_binder_name(&overlay),
            "x2",
            "an overlay sees the names its base already spent"
        );
    }

    #[test]
    fn a_fresh_field_name_has_its_own_stream_and_avoids_every_name() {
        let (a, _, _) = ids();
        let mut names = NameTable::new();
        assert_eq!(fresh_field_name(&names), "f0");
        names.set(a, "f0");
        assert_eq!(fresh_field_name(&names), "f1");
        assert_eq!(
            fresh_binder_name(&names),
            "x0",
            "a field name never eats a binder name"
        );
    }
}
