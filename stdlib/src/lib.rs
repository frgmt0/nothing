use std::sync::{Arc, OnceLock};

use nothing_core::doc::Doc;
use nothing_core::docs::DocTable;
use nothing_core::exp::Id;
use nothing_core::names::NameTable;
use nothing_core::prelude::Prelude;
use nothing_store::{Document, decode_document};

pub const STDLIB_BYTES: &[u8] = include_bytes!("../std.n");

pub fn document() -> &'static Document {
    static DOCUMENT: OnceLock<Document> = OnceLock::new();
    DOCUMENT.get_or_init(|| {
        decode_document(STDLIB_BYTES).expect("the embedded stdlib is written by this build")
    })
}

pub fn prelude() -> Arc<Prelude> {
    static PRELUDE: OnceLock<Arc<Prelude>> = OnceLock::new();
    PRELUDE
        .get_or_init(|| {
            let stdlib = document();
            Arc::new(Prelude::new(&stdlib.doc, &stdlib.names, &stdlib.docs))
        })
        .clone()
}

pub fn doc() -> &'static Doc {
    &document().doc
}

pub fn names() -> &'static NameTable {
    &document().names
}

pub fn docs() -> &'static DocTable {
    &document().docs
}

pub fn id_of(name: &str) -> Option<Id> {
    let stdlib = document();
    stdlib
        .doc
        .ids()
        .into_iter()
        .find(|id| stdlib.names.get(*id) == Some(name))
}

pub fn names_in_order() -> Vec<String> {
    let stdlib = document();
    stdlib
        .doc
        .ids()
        .into_iter()
        .map(|id| stdlib.names.display(id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::ty::Ty;

    #[test]
    fn the_embedded_stdlib_decodes_and_is_well_typed() {
        let stdlib = document();
        assert!(
            stdlib.doc.len() >= 25 && stdlib.doc.len() <= 40,
            "the stdlib holds {} definitions; B4 asks for 25 to 40",
            stdlib.doc.len()
        );
        assert!(stdlib.doc.is_well_typed());
    }

    #[test]
    fn every_stdlib_definition_is_named_documented_and_annotated() {
        let stdlib = document();
        for def in stdlib.doc.defs() {
            let name = stdlib
                .names
                .get(def.id)
                .unwrap_or_else(|| panic!("{} is unnamed", def.id));
            assert!(
                stdlib.docs.get(def.id).is_some_and(|line| !line.is_empty()),
                "{name} has no doc line"
            );
            assert_ne!(
                def.ann,
                Ty::Hole,
                "{name} has no type annotation, so callers get no help from it"
            );
        }
    }

    #[test]
    fn the_stdlib_has_no_holes_left_in_it() {
        fn holes(exp: &nothing_core::exp::Exp) -> usize {
            use nothing_core::exp::Exp;
            let here = usize::from(matches!(exp, Exp::EmptyHole(_) | Exp::NonEmptyHole(..)));
            here + match exp {
                Exp::Lam(_, _, b)
                | Exp::Proj(_, b)
                | Exp::Field(b, _)
                | Exp::Inj(_, b)
                | Exp::Print(b)
                | Exp::CmdPure(b)
                | Exp::NonEmptyHole(_, b) => holes(b),
                Exp::Ap(a, b)
                | Exp::BinOp(_, a, b)
                | Exp::Let(_, a, b)
                | Exp::Pair(a, b)
                | Exp::CmdBind(a, _, b)
                | Exp::Cons(a, b) => holes(a) + holes(b),
                Exp::If(a, b, c) | Exp::Fold(a, b, c) => holes(a) + holes(b) + holes(c),
                Exp::Record(fields) => fields.iter().map(|(_, e)| holes(e)).sum(),
                Exp::Match(s, arms) => {
                    holes(s) + arms.iter().map(|(_, _, b)| holes(b)).sum::<usize>()
                }
                _ => 0,
            }
        }
        for def in document().doc.defs() {
            assert_eq!(
                holes(&def.body),
                0,
                "{} still has a hole in it",
                document().names.display(def.id)
            );
        }
    }

    #[test]
    fn the_committed_action_log_replays_to_the_committed_document() {
        let stdlib = document();
        assert!(
            !stdlib.log.is_empty(),
            "a stdlib with no action log is not evidence it was built with the product"
        );
        let mut state = nothing_action::act::EditState::empty();
        for entry in stdlib.log.entries() {
            assert!(
                state.apply_mut(entry.action.clone()),
                "the log entry {:?} does not apply on replay",
                entry.action
            );
        }
        assert_eq!(
            state.doc(),
            stdlib.doc,
            "replaying the log does not reproduce the stdlib"
        );
        assert_eq!(state.docs.own(), stdlib.docs, "the doc lines do not replay");
        for def in stdlib.doc.defs() {
            assert_eq!(
                state.names.get(def.id),
                stdlib.names.get(def.id),
                "the name of {} does not replay",
                def.id
            );
        }
    }

    #[test]
    fn re_encoding_the_replayed_document_reproduces_the_committed_bytes() {
        let stdlib = document();
        let round_tripped = nothing_store::encode_document(stdlib);
        assert_eq!(
            round_tripped, STDLIB_BYTES,
            "stdlib/std.n is not what this build's encoder writes"
        );
    }

    #[test]
    fn the_prelude_carries_every_definition_into_the_typing_context() {
        let prelude = prelude();
        assert_eq!(prelude.len(), document().doc.len());
        let min = id_of("min").expect("the stdlib defines min");
        assert_eq!(
            prelude.ctx().lookup(&min),
            Some(Ty::Arrow(
                Box::new(Ty::Num),
                Box::new(Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Num)))
            ))
        );
        assert_eq!(prelude.docs().get(min), Some("the smaller of two numbers"));
    }

    #[test]
    fn no_stdlib_id_is_the_main_id_every_user_document_starts_with() {
        assert!(
            !document().doc.ids().contains(&nothing_core::doc::MAIN_ID),
            "a stdlib definition on the main id would be shadowed by every user document"
        );
    }

    #[test]
    fn a_document_written_against_the_prelude_types_only_with_it() {
        use nothing_core::doc::Def;
        use nothing_core::exp::Exp;
        let caller = Id::from_u128(0xca11);
        let min = id_of("min").expect("the stdlib defines min");
        let doc = Doc::new(vec![Def::new(
            caller,
            Ty::Num,
            Exp::ap(Exp::ap(Exp::var(min), Exp::num(3)), Exp::num(5)),
        )])
        .expect("one definition");

        assert!(!doc.is_well_typed(), "min is not in the document itself");
        assert!(
            doc.is_well_typed_in(prelude().ctx()),
            "and types the moment the prelude is the outer context"
        );

        let extended = prelude().extend(&doc);
        assert!(extended.is_well_typed());
        assert_eq!(extended.len(), document().doc.len() + 1);
    }

    #[test]
    fn a_log_that_carries_a_set_doc_action_is_what_documented_the_stdlib() {
        let documented = document()
            .log
            .entries()
            .iter()
            .filter(|entry| matches!(entry.action, nothing_action::act::Action::SetDoc(..)))
            .count();
        assert_eq!(
            documented,
            document().doc.len(),
            "every doc line must have been written by a SetDoc action in the log"
        );
    }
}
