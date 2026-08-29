use std::sync::{Arc, OnceLock};

use crate::ctx::Ctx;
use crate::doc::{Def, Doc};
use crate::docs::DocTable;
use crate::exp::Id;
use crate::names::NameTable;
use crate::ty::Ty;

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Prelude {
    defs: Vec<Def>,
    names: NameTable,
    docs: DocTable,
    ctx: Ctx,
}

impl Prelude {
    pub fn empty() -> Prelude {
        Prelude::default()
    }

    pub fn shared_empty() -> Arc<Prelude> {
        static EMPTY: OnceLock<Arc<Prelude>> = OnceLock::new();
        EMPTY.get_or_init(|| Arc::new(Prelude::empty())).clone()
    }

    pub fn new(doc: &Doc, names: &NameTable, docs: &DocTable) -> Prelude {
        Prelude::from_defs(doc.defs().to_vec(), names.flatten(), docs.flatten())
    }

    pub fn from_defs(defs: Vec<Def>, names: NameTable, docs: DocTable) -> Prelude {
        let ctx = defs
            .iter()
            .fold(Ctx::empty(), |ctx, def| ctx.extend(def.id, def.ann.clone()));
        Prelude {
            defs,
            names,
            docs,
            ctx,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn defs(&self) -> &[Def] {
        &self.defs
    }

    pub fn ids(&self) -> Vec<Id> {
        self.defs.iter().map(|def| def.id).collect()
    }

    pub fn contains(&self, id: Id) -> bool {
        self.defs.iter().any(|def| def.id == id)
    }

    pub fn get(&self, id: Id) -> Option<&Def> {
        self.defs.iter().find(|def| def.id == id)
    }

    pub fn ann(&self, id: Id) -> Option<Ty> {
        self.get(id).map(|def| def.ann.clone())
    }

    pub fn names(&self) -> &NameTable {
        &self.names
    }

    pub fn docs(&self) -> &DocTable {
        &self.docs
    }

    pub fn ctx(&self) -> &Ctx {
        &self.ctx
    }

    pub fn names_for(&self, own: &NameTable) -> NameTable {
        if self.is_empty() {
            return own.clone();
        }
        let mut out = NameTable::overlay(&self.names);
        for (id, name) in own.own().entries() {
            out.set(id, name);
        }
        out
    }

    pub fn docs_for(&self, own: &DocTable) -> DocTable {
        if self.is_empty() {
            return own.clone();
        }
        let mut out = DocTable::overlay(&self.docs);
        for (id, doc) in own.own().entries() {
            out.set(id, doc);
        }
        out
    }

    pub fn extend(&self, doc: &Doc) -> Doc {
        if self.is_empty() {
            return doc.clone();
        }
        let theirs = doc.ids();
        let mut defs: Vec<Def> = self
            .defs
            .iter()
            .filter(|def| !theirs.contains(&def.id))
            .cloned()
            .collect();
        defs.extend(doc.defs().iter().cloned());
        Doc::new(defs).expect("the prelude drops any id the document redefines")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exp::Exp;

    fn id(n: u128) -> Id {
        Id::from_u128(n)
    }

    fn two_definition_prelude() -> Prelude {
        let doc = Doc::new(vec![
            Def::new(id(1), Ty::Num, Exp::num(1)),
            Def::new(id(2), Ty::Bool, Exp::bool_(true)),
        ])
        .expect("two definitions");
        let mut names = NameTable::new();
        names.set(id(1), "one");
        names.set(id(2), "yes");
        let mut docs = DocTable::new();
        docs.set(id(1), "the number one");
        Prelude::new(&doc, &names, &docs)
    }

    #[test]
    fn an_empty_prelude_changes_nothing() {
        let empty = Prelude::empty();
        assert!(empty.is_empty());
        assert_eq!(empty.ids(), Vec::<Id>::new());
        let doc = Doc::single(Exp::num(3));
        assert_eq!(empty.extend(&doc), doc);
        assert_eq!(empty.ctx().lookup(&id(1)), None);
    }

    #[test]
    fn the_prelude_puts_its_definitions_in_the_typing_context() {
        let prelude = two_definition_prelude();
        assert_eq!(prelude.ctx().lookup(&id(1)), Some(Ty::Num));
        assert_eq!(prelude.ctx().lookup(&id(2)), Some(Ty::Bool));
        assert_eq!(prelude.len(), 2);
        assert!(prelude.contains(id(1)));
        assert_eq!(prelude.ann(id(2)), Some(Ty::Bool));
    }

    #[test]
    fn extending_a_document_puts_the_prelude_first_and_keeps_the_document() {
        let prelude = two_definition_prelude();
        let doc = Doc::new(vec![Def::new(id(9), Ty::Num, Exp::var(id(1)))]).expect("one");
        let extended = prelude.extend(&doc);
        assert_eq!(extended.len(), 3);
        assert_eq!(extended.ids(), vec![id(1), id(2), id(9)]);
        assert!(
            extended.is_well_typed(),
            "a document referring to a prelude definition types once the prelude is in scope"
        );
        assert!(
            !doc.is_well_typed(),
            "and does not type without it, which is the whole point of the mechanism"
        );
    }

    #[test]
    fn names_and_docs_fall_through_to_the_prelude_but_a_document_may_shadow_them() {
        let prelude = two_definition_prelude();
        let mut own = NameTable::new();
        own.set(id(9), "mine");
        let names = prelude.names_for(&own);
        assert_eq!(names.get(id(1)), Some("one"));
        assert_eq!(names.get(id(9)), Some("mine"));
        assert_eq!(names.own().len(), 1, "only `mine` belongs to the document");

        let mut own_docs = DocTable::new();
        own_docs.set(id(9), "mine too");
        let docs = prelude.docs_for(&own_docs);
        assert_eq!(docs.get(id(1)), Some("the number one"));
        assert_eq!(docs.get(id(9)), Some("mine too"));
        assert_eq!(docs.own().len(), 1);
    }

    #[test]
    fn a_document_that_redefines_a_prelude_id_wins() {
        let prelude = two_definition_prelude();
        let doc = Doc::new(vec![Def::new(id(1), Ty::Num, Exp::num(99))]).expect("one");
        let extended = prelude.extend(&doc);
        assert_eq!(extended.len(), 2);
        assert_eq!(extended.ids(), vec![id(2), id(1)]);
        assert_eq!(
            extended.get(id(1)).map(|def| def.body.clone()),
            Some(Exp::num(99))
        );
    }
}
