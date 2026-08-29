use crate::ctx::Ctx;
use crate::exp::{Exp, HoleId, Id};
use crate::names::NameTable;
use crate::render::render;
use crate::ty::Ty;
use crate::typing::{ana, is_well_typed_in};

pub const MAIN_ID: Id = Id::from_u128(0x6d61696e_0000_0000_0000_000000000000);

pub const MAIN_NAME: &str = "main";

#[derive(Clone, PartialEq, Debug)]
pub struct Def {
    pub id: Id,
    pub ann: Ty,
    pub body: Exp,
}

impl Def {
    pub fn new(id: Id, ann: Ty, body: Exp) -> Def {
        Def { id, ann, body }
    }

    pub fn hole(id: Id, hole: HoleId) -> Def {
        Def {
            id,
            ann: Ty::Hole,
            body: Exp::empty_hole(hole),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Doc {
    defs: Vec<Def>,
}

impl Doc {
    pub fn new(defs: Vec<Def>) -> Option<Doc> {
        if defs.is_empty() {
            return None;
        }
        let mut seen: Vec<Id> = Vec::new();
        for def in &defs {
            if seen.contains(&def.id) {
                return None;
            }
            seen.push(def.id);
        }
        Some(Doc { defs })
    }

    pub fn single(exp: Exp) -> Doc {
        Doc {
            defs: vec![Def::new(MAIN_ID, Ty::Hole, exp)],
        }
    }

    pub fn defs(&self) -> &[Def] {
        &self.defs
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn get(&self, id: Id) -> Option<&Def> {
        self.defs.iter().find(|def| def.id == id)
    }

    pub fn index_of(&self, id: Id) -> Option<usize> {
        self.defs.iter().position(|def| def.id == id)
    }

    pub fn ids(&self) -> Vec<Id> {
        self.defs.iter().map(|def| def.id).collect()
    }

    pub fn ctx(&self) -> Ctx {
        self.defs
            .iter()
            .fold(Ctx::empty(), |ctx, def| ctx.extend(def.id, def.ann.clone()))
    }

    pub fn is_well_typed(&self) -> bool {
        let ctx = self.ctx();
        self.defs.iter().all(|def| def_is_well_typed(&ctx, def))
    }

    pub fn main_id(&self, names: &NameTable) -> Option<Id> {
        self.defs
            .iter()
            .map(|def| def.id)
            .find(|id| names.get(*id) == Some(MAIN_NAME))
    }

    pub fn field_ids(&self) -> Vec<Id> {
        let mut out = Vec::new();
        for def in &self.defs {
            for id in field_ids(&def.body) {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        }
        out
    }

    pub fn render(&self, names: &NameTable) -> String {
        self.defs
            .iter()
            .map(|def| render_def(def, names))
            .collect::<Vec<String>>()
            .join("\n")
    }
}

pub fn def_is_well_typed(ctx: &Ctx, def: &Def) -> bool {
    is_well_typed_in(ctx, &def.body) && ana(ctx, &def.body, &def.ann)
}

pub fn render_def(def: &Def, names: &NameTable) -> String {
    format!(
        "{} : {} = {}",
        names.display(def.id),
        crate::render::render_ty(&def.ann, names),
        render(&def.body, names)
    )
}

pub fn references(exp: &Exp, target: Id) -> bool {
    match exp {
        Exp::Var(id) => *id == target,
        Exp::Lam(id, _, body) => *id != target && references(body, target),
        Exp::Let(id, bound, body) => {
            references(bound, target) || (*id != target && references(body, target))
        }
        Exp::Ap(f, a) => references(f, target) || references(a, target),
        Exp::BinOp(_, l, r) => references(l, target) || references(r, target),
        Exp::Pair(l, r) => references(l, target) || references(r, target),
        Exp::Cons(l, r) => references(l, target) || references(r, target),
        Exp::If(c, t, e) => references(c, target) || references(t, target) || references(e, target),
        Exp::Fold(l, i, s) => {
            references(l, target) || references(i, target) || references(s, target)
        }
        Exp::Proj(_, e) => references(e, target),
        Exp::Field(e, _) => references(e, target),
        Exp::Record(fields) => fields.iter().any(|(_, e)| references(e, target)),
        Exp::NonEmptyHole(_, e) => references(e, target),
        Exp::Num(_) | Exp::Bool(_) | Exp::Str(_) | Exp::Nil | Exp::EmptyHole(_) => false,
    }
}

pub fn field_ids(exp: &Exp) -> Vec<Id> {
    fn walk(exp: &Exp, out: &mut Vec<Id>) {
        match exp {
            Exp::Record(fields) => {
                for (id, e) in fields {
                    if !out.contains(id) {
                        out.push(*id);
                    }
                    walk(e, out);
                }
            }
            Exp::Field(e, id) => {
                if !out.contains(id) {
                    out.push(*id);
                }
                walk(e, out);
            }
            Exp::Lam(_, _, e) | Exp::Proj(_, e) | Exp::NonEmptyHole(_, e) => walk(e, out),
            Exp::Ap(a, b)
            | Exp::BinOp(_, a, b)
            | Exp::Pair(a, b)
            | Exp::Cons(a, b)
            | Exp::Let(_, a, b) => {
                walk(a, out);
                walk(b, out);
            }
            Exp::If(a, b, c) | Exp::Fold(a, b, c) => {
                walk(a, out);
                walk(b, out);
                walk(c, out);
            }
            Exp::Var(_)
            | Exp::Num(_)
            | Exp::Bool(_)
            | Exp::Str(_)
            | Exp::Nil
            | Exp::EmptyHole(_) => {}
        }
    }
    let mut out = Vec::new();
    walk(exp, &mut out);
    out
}

pub fn projects(exp: &Exp, field: Id) -> bool {
    match exp {
        Exp::Field(e, id) => *id == field || projects(e, field),
        Exp::Record(fields) => fields.iter().any(|(_, e)| projects(e, field)),
        Exp::Lam(_, _, e) | Exp::Proj(_, e) | Exp::NonEmptyHole(_, e) => projects(e, field),
        Exp::Ap(a, b) | Exp::BinOp(_, a, b) | Exp::Pair(a, b) | Exp::Cons(a, b) => {
            projects(a, field) || projects(b, field)
        }
        Exp::Let(_, a, b) => projects(a, field) || projects(b, field),
        Exp::If(a, b, c) | Exp::Fold(a, b, c) => {
            projects(a, field) || projects(b, field) || projects(c, field)
        }
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::Str(_) | Exp::Nil | Exp::EmptyHole(_) => {
            false
        }
    }
}

pub fn quarantine_projections(exp: &Exp, field: Id, fresh: &mut dyn FnMut() -> HoleId) -> Exp {
    match exp {
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::Str(_) | Exp::Nil | Exp::EmptyHole(_) => {
            exp.clone()
        }
        Exp::Lam(id, ty, body) => {
            Exp::lam(*id, ty.clone(), quarantine_projections(body, field, fresh))
        }
        Exp::Let(id, bound, body) => Exp::let_(
            *id,
            quarantine_projections(bound, field, fresh),
            quarantine_projections(body, field, fresh),
        ),
        Exp::Ap(f, a) => Exp::ap(
            quarantine_projections(f, field, fresh),
            quarantine_projections(a, field, fresh),
        ),
        Exp::BinOp(op, l, r) => Exp::bin_op(
            *op,
            quarantine_projections(l, field, fresh),
            quarantine_projections(r, field, fresh),
        ),
        Exp::Pair(l, r) => Exp::pair(
            quarantine_projections(l, field, fresh),
            quarantine_projections(r, field, fresh),
        ),
        Exp::Cons(l, r) => Exp::cons(
            quarantine_projections(l, field, fresh),
            quarantine_projections(r, field, fresh),
        ),
        Exp::If(c, t, e) => Exp::if_(
            quarantine_projections(c, field, fresh),
            quarantine_projections(t, field, fresh),
            quarantine_projections(e, field, fresh),
        ),
        Exp::Fold(l, i, s) => Exp::fold(
            quarantine_projections(l, field, fresh),
            quarantine_projections(i, field, fresh),
            quarantine_projections(s, field, fresh),
        ),
        Exp::Proj(side, e) => Exp::proj(*side, quarantine_projections(e, field, fresh)),
        Exp::Record(fields) => Exp::record(
            fields
                .iter()
                .map(|(id, e)| (*id, quarantine_projections(e, field, fresh))),
        ),
        Exp::NonEmptyHole(h, e) => Exp::non_empty_hole(*h, quarantine_projections(e, field, fresh)),
        Exp::Field(e, id) => {
            let subject = quarantine_projections(e, field, fresh);
            if *id == field {
                Exp::non_empty_hole(fresh(), subject)
            } else {
                Exp::field(subject, *id)
            }
        }
    }
}

pub fn vacate(exp: &Exp, target: Id, fresh: &mut dyn FnMut() -> HoleId) -> Exp {
    match exp {
        Exp::Var(id) if *id == target => Exp::empty_hole(fresh()),
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::Str(_) | Exp::Nil | Exp::EmptyHole(_) => {
            exp.clone()
        }
        Exp::Lam(id, ty, body) => {
            if *id == target {
                exp.clone()
            } else {
                Exp::lam(*id, ty.clone(), vacate(body, target, fresh))
            }
        }
        Exp::Let(id, bound, body) => {
            let bound = vacate(bound, target, fresh);
            let body = if *id == target {
                (**body).clone()
            } else {
                vacate(body, target, fresh)
            };
            Exp::let_(*id, bound, body)
        }
        Exp::Ap(f, a) => Exp::ap(vacate(f, target, fresh), vacate(a, target, fresh)),
        Exp::BinOp(op, l, r) => {
            Exp::bin_op(*op, vacate(l, target, fresh), vacate(r, target, fresh))
        }
        Exp::Pair(l, r) => Exp::pair(vacate(l, target, fresh), vacate(r, target, fresh)),
        Exp::Cons(l, r) => Exp::cons(vacate(l, target, fresh), vacate(r, target, fresh)),
        Exp::If(c, t, e) => Exp::if_(
            vacate(c, target, fresh),
            vacate(t, target, fresh),
            vacate(e, target, fresh),
        ),
        Exp::Fold(l, i, s) => Exp::fold(
            vacate(l, target, fresh),
            vacate(i, target, fresh),
            vacate(s, target, fresh),
        ),
        Exp::Proj(side, e) => Exp::proj(*side, vacate(e, target, fresh)),
        Exp::Field(e, id) => Exp::field(vacate(e, target, fresh), *id),
        Exp::Record(fields) => {
            Exp::record(fields.iter().map(|(id, e)| (*id, vacate(e, target, fresh))))
        }
        Exp::NonEmptyHole(h, e) => {
            let inner = vacate(e, target, fresh);
            if matches!(inner, Exp::EmptyHole(_)) {
                inner
            } else {
                Exp::non_empty_hole(*h, inner)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exp::Op;

    fn arrow(a: Ty, b: Ty) -> Ty {
        Ty::Arrow(Box::new(a), Box::new(b))
    }

    fn id(n: u128) -> Id {
        Id::from_u128(n)
    }

    fn hole(n: u128) -> HoleId {
        HoleId::from_u128(n)
    }

    #[test]
    fn main_id_is_the_ascii_word_main_then_zeros() {
        let bytes = MAIN_ID.as_u128().to_be_bytes();
        assert_eq!(&bytes[0..4], b"main");
        assert!(bytes[4..].iter().all(|b| *b == 0));
    }

    #[test]
    fn a_document_needs_at_least_one_definition() {
        assert!(Doc::new(vec![]).is_none());
    }

    #[test]
    fn duplicate_definition_ids_are_rejected() {
        let a = Def::new(id(1), Ty::Num, Exp::num(1));
        let b = Def::new(id(1), Ty::Num, Exp::num(2));
        assert!(Doc::new(vec![a, b]).is_none());
    }

    #[test]
    fn a_definition_is_in_scope_in_every_body_including_its_own() {
        let f = id(1);
        let doc = Doc::new(vec![Def::new(
            f,
            arrow(Ty::Num, Ty::Num),
            Exp::lam(id(2), Ty::Num, Exp::ap(Exp::var(f), Exp::var(id(2)))),
        )])
        .expect("one definition");
        assert!(doc.is_well_typed());
    }

    #[test]
    fn mutual_recursion_typechecks_through_the_annotations() {
        let even = id(1);
        let odd = id(2);
        let n = id(3);
        let doc = Doc::new(vec![
            Def::new(
                even,
                arrow(Ty::Num, Ty::Bool),
                Exp::lam(
                    n,
                    Ty::Num,
                    Exp::if_(
                        Exp::bin_op(Op::Eq, Exp::var(n), Exp::num(0)),
                        Exp::bool_(true),
                        Exp::ap(
                            Exp::var(odd),
                            Exp::bin_op(Op::Sub, Exp::var(n), Exp::num(1)),
                        ),
                    ),
                ),
            ),
            Def::new(
                odd,
                arrow(Ty::Num, Ty::Bool),
                Exp::lam(
                    n,
                    Ty::Num,
                    Exp::if_(
                        Exp::bin_op(Op::Eq, Exp::var(n), Exp::num(0)),
                        Exp::bool_(false),
                        Exp::ap(
                            Exp::var(even),
                            Exp::bin_op(Op::Sub, Exp::var(n), Exp::num(1)),
                        ),
                    ),
                ),
            ),
        ])
        .expect("two definitions");
        assert!(doc.is_well_typed());
    }

    #[test]
    fn a_body_inconsistent_with_its_own_annotation_is_ill_typed() {
        let doc =
            Doc::new(vec![Def::new(id(1), Ty::Num, Exp::bool_(true))]).expect("one definition");
        assert!(!doc.is_well_typed());
    }

    #[test]
    fn a_wrong_annotation_makes_the_caller_ill_typed() {
        let helper = id(1);
        let caller = id(2);
        let doc = Doc::new(vec![
            Def::new(helper, Ty::Bool, Exp::bool_(true)),
            Def::new(
                caller,
                Ty::Num,
                Exp::bin_op(Op::Add, Exp::var(helper), Exp::num(1)),
            ),
        ])
        .expect("two definitions");
        assert!(!doc.is_well_typed());
    }

    #[test]
    fn an_unannotated_definition_is_usable_everywhere() {
        let helper = id(1);
        let caller = id(2);
        let doc = Doc::new(vec![
            Def::new(helper, Ty::Hole, Exp::empty_hole(hole(9))),
            Def::new(
                caller,
                Ty::Num,
                Exp::bin_op(Op::Add, Exp::var(helper), Exp::num(1)),
            ),
        ])
        .expect("two definitions");
        assert!(doc.is_well_typed());
    }

    #[test]
    fn a_dangling_reference_is_ill_typed() {
        let doc =
            Doc::new(vec![Def::new(id(1), Ty::Hole, Exp::var(id(99)))]).expect("one definition");
        assert!(!doc.is_well_typed());
    }

    #[test]
    fn vacating_a_reference_leaves_an_empty_hole() {
        let target = id(7);
        let e = Exp::bin_op(Op::Add, Exp::var(target), Exp::num(1));
        let mut n = 0u128;
        let out = vacate(&e, target, &mut || {
            n += 1;
            hole(n)
        });
        assert_eq!(
            out,
            Exp::bin_op(Op::Add, Exp::empty_hole(hole(1)), Exp::num(1))
        );
        assert!(!references(&out, target));
    }

    #[test]
    fn vacating_does_not_touch_a_shadowed_binder() {
        let target = id(7);
        let e = Exp::lam(target, Ty::Num, Exp::var(target));
        let out = vacate(&e, target, &mut || hole(1));
        assert_eq!(out, e);
    }

    #[test]
    fn vacating_a_quarantined_reference_collapses_the_quarantine() {
        let target = id(7);
        let e = Exp::non_empty_hole(hole(1), Exp::var(target));
        let out = vacate(&e, target, &mut || hole(2));
        assert_eq!(out, Exp::empty_hole(hole(2)));
    }

    #[test]
    fn main_is_found_by_name_not_by_position() {
        let mut names = NameTable::new();
        names.set(id(1), "helper");
        names.set(id(2), MAIN_NAME);
        let doc = Doc::new(vec![
            Def::new(id(1), Ty::Hole, Exp::num(1)),
            Def::new(id(2), Ty::Hole, Exp::num(2)),
        ])
        .expect("two definitions");
        assert_eq!(doc.main_id(&names), Some(id(2)));
    }

    #[test]
    fn a_document_without_main_reports_none() {
        let mut names = NameTable::new();
        names.set(id(1), "helper");
        let doc = Doc::new(vec![Def::new(id(1), Ty::Hole, Exp::num(1))]).expect("one");
        assert_eq!(doc.main_id(&names), None);
    }
}
