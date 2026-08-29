use std::sync::Arc;

use crate::exp::Id;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DocTable {
    layer: im::HashMap<Id, String>,
    base: Option<Arc<DocTable>>,
}

impl DocTable {
    pub fn new() -> DocTable {
        DocTable::default()
    }

    pub fn overlay(base: &DocTable) -> DocTable {
        DocTable {
            layer: im::HashMap::new(),
            base: Some(Arc::new(base.clone())),
        }
    }

    pub fn base(&self) -> Option<&DocTable> {
        self.base.as_deref()
    }

    pub fn depth(&self) -> usize {
        1 + self.base.as_deref().map_or(0, DocTable::depth)
    }

    pub fn set(&mut self, id: Id, doc: impl Into<String>) {
        let doc = doc.into();
        if doc.is_empty() {
            self.layer.remove(&id);
        } else {
            self.layer.insert(id, doc);
        }
    }

    pub fn with(&self, id: Id, doc: impl Into<String>) -> DocTable {
        let mut next = self.clone();
        next.set(id, doc);
        next
    }

    pub fn get(&self, id: Id) -> Option<&str> {
        match self.layer.get(&id) {
            Some(doc) => Some(doc.as_str()),
            None => self.base.as_deref().and_then(|base| base.get(id)),
        }
    }

    pub fn get_own(&self, id: Id) -> Option<&str> {
        self.layer.get(&id).map(String::as_str)
    }

    pub fn contains(&self, id: Id) -> bool {
        self.get(id).is_some()
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
            .filter_map(|id| self.get(id).map(|doc| (id, doc.to_string())))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.layer.is_empty() && self.base.as_deref().is_none_or(DocTable::is_empty)
    }

    pub fn flatten(&self) -> DocTable {
        let mut flat = DocTable::new();
        for (id, doc) in self.entries() {
            flat.set(id, doc);
        }
        flat
    }

    pub fn own(&self) -> DocTable {
        DocTable {
            layer: self.layer.clone(),
            base: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> Id {
        Id::from_u128(n)
    }

    #[test]
    fn an_empty_table_documents_nothing() {
        let docs = DocTable::new();
        assert_eq!(docs.get(id(1)), None);
        assert!(docs.is_empty());
        assert_eq!(docs.len(), 0);
    }

    #[test]
    fn a_doc_line_round_trips_and_is_replaced_not_appended() {
        let mut docs = DocTable::new();
        docs.set(id(1), "the smaller of two numbers");
        assert_eq!(docs.get(id(1)), Some("the smaller of two numbers"));
        docs.set(id(1), "the smaller of its two arguments");
        assert_eq!(docs.get(id(1)), Some("the smaller of its two arguments"));
        assert_eq!(docs.len(), 1);
    }

    #[test]
    fn setting_a_doc_to_nothing_removes_it() {
        let mut docs = DocTable::new();
        docs.set(id(1), "a line");
        docs.set(id(1), "");
        assert_eq!(docs.get(id(1)), None);
        assert!(docs.is_empty());
    }

    #[test]
    fn an_overlay_shadows_its_base_without_touching_it() {
        let mut base = DocTable::new();
        base.set(id(1), "from the stdlib");
        base.set(id(2), "also from the stdlib");

        let mut mine = DocTable::overlay(&base);
        mine.set(id(1), "mine");

        assert_eq!(mine.get(id(1)), Some("mine"));
        assert_eq!(mine.get(id(2)), Some("also from the stdlib"));
        assert_eq!(base.get(id(1)), Some("from the stdlib"));
        assert_eq!(mine.get_own(id(2)), None);
        assert_eq!(mine.depth(), 2);
        assert_eq!(
            mine.base().and_then(|b| b.get(id(1))),
            Some("from the stdlib")
        );
    }

    #[test]
    fn own_keeps_only_the_documents_own_layer() {
        let mut base = DocTable::new();
        base.set(id(1), "from the stdlib");
        let mut mine = DocTable::overlay(&base);
        mine.set(id(2), "mine");

        let own = mine.own();
        assert_eq!(own.depth(), 1);
        assert_eq!(
            own.get(id(1)),
            None,
            "the stdlib's docs are not the document's"
        );
        assert_eq!(own.get(id(2)), Some("mine"));

        assert_eq!(mine.flatten().len(), 2, "flattening keeps both");
    }

    #[test]
    fn documenting_is_a_write_that_cannot_fail() {
        let mut docs = DocTable::new();
        for _ in 0..3 {
            docs.set(id(1), "steady");
        }
        assert_eq!(docs.get(id(1)), Some("steady"));
        assert_eq!(docs.ids(), vec![id(1)]);
        assert!(docs.contains(id(1)));
    }
}
