use std::collections::HashSet;

use nothing_core::ctx::Ctx;
use nothing_core::exp::{Exp, HoleId, Id, Op, Side, UuidStream};
use nothing_core::names::{NameTable, fresh_binder_name};
use nothing_core::ty::{Ty, matched_arrow, matched_prod};
use nothing_core::typing::{ana, is_well_typed, syn};

use crate::zipper::{Frame, Zipper, unzip};

#[derive(Clone, PartialEq, Debug)]
pub enum Action {
    MoveChild(usize),
    MoveParent,
    MoveNextSibling,
    MovePrevSibling,

    Delete,

    ConstructNum(i64),
    ConstructBool(bool),
    ConstructVar(Id),

    ConstructLam,
    ConstructAp,
    ConstructBinOp(Op),
    ConstructIf,
    ConstructLet,
    ConstructPair,
    ConstructProj(Side),
    ConstructNonEmptyHole,

    SetAnn(Ty),

    SetBinderId(Id),

    Rename(Id, String),

    Finish,
}

const FRESH_SEED: u128 = 0x1234_5678_9abc_def0_0f1e_2d3c_4b5a_6978;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Fresh {
    stream: UuidStream,
    used: HashSet<u128>,
}

impl Default for Fresh {
    fn default() -> Fresh {
        Fresh {
            stream: UuidStream::new(FRESH_SEED),
            used: HashSet::new(),
        }
    }
}

impl Fresh {
    pub fn new() -> Fresh {
        Fresh::default()
    }

    pub fn from_program(exp: &Exp) -> Fresh {
        let mut f = Fresh::default();
        f.observe(exp);
        f
    }

    pub fn observe(&mut self, exp: &Exp) {
        match exp {
            Exp::Var(id) => self.bump_id(*id),
            Exp::Lam(id, _, body) => {
                self.bump_id(*id);
                self.observe(body);
            }
            Exp::Ap(f, a) => {
                self.observe(f);
                self.observe(a);
            }
            Exp::Num(_) | Exp::Bool(_) => {}
            Exp::BinOp(_, l, r) | Exp::Pair(l, r) => {
                self.observe(l);
                self.observe(r);
            }
            Exp::If(c, t, e) => {
                self.observe(c);
                self.observe(t);
                self.observe(e);
            }
            Exp::Let(id, bound, body) => {
                self.bump_id(*id);
                self.observe(bound);
                self.observe(body);
            }
            Exp::Proj(_, e) => self.observe(e),
            Exp::EmptyHole(h) => self.bump_hole(*h),
            Exp::NonEmptyHole(h, e) => {
                self.bump_hole(*h);
                self.observe(e);
            }
        }
    }

    fn bump_id(&mut self, id: Id) {
        self.spend(id.as_u128());
    }

    fn bump_hole(&mut self, h: HoleId) {
        self.spend(h.as_u128());
    }

    fn spend(&mut self, bits: u128) {
        if self.used.insert(bits) {
            self.stream.stir(bits);
        }
    }

    fn next_bits(&mut self) -> u128 {
        loop {
            let candidate = self.stream.next_uuid().as_u128();
            if self.used.insert(candidate) {
                return candidate;
            }
        }
    }

    pub fn hole(&mut self) -> HoleId {
        HoleId::from_u128(self.next_bits())
    }

    pub fn id(&mut self) -> Id {
        Id::from_u128(self.next_bits())
    }
}

pub fn ctx_at(zipper: &Zipper) -> Ctx {
    zipper.ctx()
}

pub fn expected_ty_at(zipper: &Zipper) -> Ty {
    ctx_and_expected_ty_at(zipper).1
}

pub fn ctx_and_expected_ty_at(zipper: &Zipper) -> (Ctx, Ty) {
    let mut ctx = Ctx::empty();

    let mut expected = Ty::Hole;

    for frame in &zipper.path {
        match frame {
            Frame::LamBody(id, ann) => {
                let (_, out) = matched_arrow(&expected).unwrap_or((Ty::Hole, Ty::Hole));

                ctx = ctx.extend(*id, ann.clone());
                expected = out;
            }
            Frame::ApFun(_) => {
                expected = Ty::Arrow(Box::new(Ty::Hole), Box::new(expected));
            }
            Frame::ApArg(fun) => {
                expected = syn(&ctx, fun)
                    .as_ref()
                    .and_then(matched_arrow)
                    .map(|(in_ty, _)| in_ty)
                    .unwrap_or(Ty::Hole);
            }
            Frame::BinOpLeft(..) | Frame::BinOpRight(..) => expected = Ty::Num,
            Frame::IfCond(..) => expected = Ty::Bool,
            Frame::IfThen(_, else_) => {
                if expected == Ty::Hole {
                    expected = syn(&ctx, else_).unwrap_or(Ty::Hole);
                }
            }
            Frame::IfElse(_, then) => {
                if expected == Ty::Hole {
                    expected = syn(&ctx, then).unwrap_or(Ty::Hole);
                }
            }

            Frame::LetBound(..) => expected = Ty::Hole,
            Frame::LetBody(id, bound) => {
                let ty = syn(&ctx, bound).unwrap_or(Ty::Hole);
                ctx = ctx.extend(*id, ty);
            }
            Frame::PairFst(_) => {
                expected = matched_prod(&expected).unwrap_or((Ty::Hole, Ty::Hole)).0;
            }
            Frame::PairSnd(_) => {
                expected = matched_prod(&expected).unwrap_or((Ty::Hole, Ty::Hole)).1;
            }
            Frame::ProjBody(side) => {
                expected = match side {
                    Side::L => Ty::Prod(Box::new(expected), Box::new(Ty::Hole)),
                    Side::R => Ty::Prod(Box::new(Ty::Hole), Box::new(expected)),
                };
            }

            Frame::NonEmptyHoleBody(_) => expected = Ty::Hole,
        }
    }

    (ctx, expected)
}

pub fn apply_with(
    zipper: Zipper,
    action: Action,
    fresh: &mut Fresh,
    names: &mut NameTable,
) -> Option<Zipper> {
    match action {
        Action::MoveChild(n) => zipper.move_child(n),
        Action::MoveParent => zipper.move_parent(),
        Action::MoveNextSibling => zipper.move_next_sibling(),
        Action::MovePrevSibling => zipper.move_prev_sibling(),

        Action::Delete => {
            let hole = fresh.hole();
            Some(zipper.replace_focus(Exp::empty_hole(hole)))
        }

        Action::ConstructNum(n) => construct_leaf(zipper, Exp::num(n), fresh),
        Action::ConstructBool(b) => construct_leaf(zipper, Exp::bool_(b), fresh),
        Action::ConstructVar(id) => {
            if ctx_at(&zipper).lookup(&id).is_none() {
                None
            } else {
                construct_leaf(zipper, Exp::var(id), fresh)
            }
        }

        Action::ConstructLam => {
            let id = fresh.id();
            let built = construct_wrapping(zipper, Ty::Hole, fresh, |body, _| {
                Exp::lam(id, Ty::Hole, body)
            });
            name_new_binder(id, &built, names);
            built
        }
        Action::ConstructAp => construct_wrapping(
            zipper,
            Ty::Arrow(Box::new(Ty::Hole), Box::new(Ty::Hole)),
            fresh,
            |fun, fresh| Exp::ap(fun, Exp::empty_hole(fresh.hole())),
        ),
        Action::ConstructBinOp(op) => construct_wrapping(zipper, Ty::Num, fresh, |lhs, fresh| {
            Exp::bin_op(op, lhs, Exp::empty_hole(fresh.hole()))
        }),
        Action::ConstructIf => construct_wrapping(zipper, Ty::Bool, fresh, |cond, fresh| {
            Exp::if_(
                cond,
                Exp::empty_hole(fresh.hole()),
                Exp::empty_hole(fresh.hole()),
            )
        }),
        Action::ConstructLet => {
            let id = fresh.id();
            let built = construct_wrapping(zipper, Ty::Hole, fresh, |bound, fresh| {
                Exp::let_(id, bound, Exp::empty_hole(fresh.hole()))
            });
            name_new_binder(id, &built, names);
            built
        }
        Action::ConstructPair => construct_wrapping(zipper, Ty::Hole, fresh, |fst, fresh| {
            Exp::pair(fst, Exp::empty_hole(fresh.hole()))
        }),
        Action::ConstructProj(side) => construct_wrapping(
            zipper,
            Ty::Prod(Box::new(Ty::Hole), Box::new(Ty::Hole)),
            fresh,
            |body, _| Exp::proj(side, body),
        ),

        Action::ConstructNonEmptyHole => {
            construct_wrapping(zipper, Ty::Hole, fresh, |inner, fresh| {
                Exp::non_empty_hole(fresh.hole(), inner)
            })
        }

        Action::SetAnn(ann) => set_ann(zipper, ann),
        Action::SetBinderId(id) => set_binder_id(zipper, id),

        Action::Rename(id, name) => {
            names.rename(id, name);
            Some(zipper)
        }

        Action::Finish => finish(zipper),
    }
}

fn name_new_binder(id: Id, built: &Option<Zipper>, names: &mut NameTable) {
    if built.is_some() {
        let name = fresh_binder_name(names);
        names.set(id, name);
    }
}

fn first_empty_hole_child(exp: &Exp) -> Option<usize> {
    fn first(children: &[&Exp]) -> Option<usize> {
        children.iter().position(|c| matches!(c, Exp::EmptyHole(_)))
    }
    match exp {
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::EmptyHole(_) => None,
        Exp::Lam(_, _, b) | Exp::Proj(_, b) | Exp::NonEmptyHole(_, b) => first(&[b]),
        Exp::Ap(a, b) | Exp::BinOp(_, a, b) | Exp::Let(_, a, b) | Exp::Pair(a, b) => first(&[a, b]),
        Exp::If(c, t, e) => first(&[c, t, e]),
    }
}

fn construct_leaf(zipper: Zipper, leaf: Exp, fresh: &mut Fresh) -> Option<Zipper> {
    let (ctx, expected) = ctx_and_expected_ty_at(&zipper);
    place(zipper, leaf, &ctx, &expected, fresh)
}

fn construct_wrapping(
    zipper: Zipper,
    inner_expected: Ty,
    fresh: &mut Fresh,
    build: impl FnOnce(Exp, &mut Fresh) -> Exp,
) -> Option<Zipper> {
    let (ctx, expected) = ctx_and_expected_ty_at(&zipper);
    let focus = zipper.focus.clone();
    let fits = syn(&ctx, &focus).is_some() && ana(&ctx, &focus, &inner_expected);
    let principal = if fits {
        focus
    } else {
        Exp::non_empty_hole(fresh.hole(), focus)
    };
    let form = build(principal, fresh);
    place(zipper, form, &ctx, &expected, fresh)
}

fn place(zipper: Zipper, form: Exp, ctx: &Ctx, expected: &Ty, fresh: &mut Fresh) -> Option<Zipper> {
    let target = first_empty_hole_child(&form);

    if ana(ctx, &form, expected) {
        let candidate = zipper.clone().replace_focus(form.clone());
        if is_well_typed(&candidate.to_exp()) {
            return descend_into_form(candidate, target, false);
        }
    }

    let quarantined = Exp::non_empty_hole(fresh.hole(), form);
    let candidate = zipper.replace_focus(quarantined);
    if is_well_typed(&candidate.to_exp()) {
        descend_into_form(candidate, target, true)
    } else {
        None
    }
}

fn descend_into_form(
    zipper: Zipper,
    target: Option<usize>,
    through_quarantine: bool,
) -> Option<Zipper> {
    match target {
        None => Some(zipper),
        Some(i) => {
            let zipper = if through_quarantine {
                zipper.move_child(0)?
            } else {
                zipper
            };
            zipper.move_child(i)
        }
    }
}

fn keep_if_well_typed(zipper: Zipper) -> Option<Zipper> {
    if is_well_typed(&zipper.to_exp()) {
        Some(zipper)
    } else {
        None
    }
}

fn set_ann(zipper: Zipper, ann: Ty) -> Option<Zipper> {
    let updated = match &zipper.focus {
        Exp::Lam(id, _, body) => Exp::Lam(*id, ann, body.clone()),
        _ => return None,
    };
    keep_if_well_typed(zipper.replace_focus(updated))
}

fn set_binder_id(zipper: Zipper, id: Id) -> Option<Zipper> {
    let updated = match &zipper.focus {
        Exp::Lam(_, ann, body) => Exp::Lam(id, ann.clone(), body.clone()),
        Exp::Let(_, bound, body) => Exp::Let(id, bound.clone(), body.clone()),
        _ => return None,
    };
    keep_if_well_typed(zipper.replace_focus(updated))
}

fn finish(zipper: Zipper) -> Option<Zipper> {
    let inner = match &zipper.focus {
        Exp::NonEmptyHole(_, inner) => (**inner).clone(),
        _ => return None,
    };
    let (ctx, expected) = ctx_and_expected_ty_at(&zipper);
    if !ana(&ctx, &inner, &expected) {
        return None;
    }
    let candidate = zipper.replace_focus(inner);
    if is_well_typed(&candidate.to_exp()) {
        Some(candidate)
    } else {
        None
    }
}

pub fn apply(zipper: Zipper, action: Action) -> Option<Zipper> {
    let mut fresh = Fresh::from_program(&zipper.to_exp());
    let mut names = NameTable::new();
    apply_with(zipper, action, &mut fresh, &mut names)
}

#[derive(Clone, PartialEq, Debug)]
pub struct EditState {
    pub zipper: Zipper,
    pub fresh: Fresh,
    pub names: NameTable,
}

impl EditState {
    pub fn new(exp: Exp) -> EditState {
        EditState::with_names(exp, NameTable::new())
    }

    pub fn with_names(exp: Exp, names: NameTable) -> EditState {
        let fresh = Fresh::from_program(&exp);
        EditState {
            zipper: unzip(exp),
            fresh,
            names,
        }
    }

    pub fn empty() -> EditState {
        let mut fresh = Fresh::new();
        let hole = fresh.hole();
        EditState {
            zipper: unzip(Exp::empty_hole(hole)),
            fresh,
            names: NameTable::new(),
        }
    }

    pub fn exp(&self) -> Exp {
        self.zipper.to_exp()
    }

    pub fn names(&self) -> &NameTable {
        &self.names
    }

    pub fn render(&self) -> String {
        nothing_core::render::render(&self.exp(), &self.names)
    }

    pub fn apply(&self, action: Action) -> Option<EditState> {
        let mut fresh = self.fresh.clone();
        let mut names = self.names.clone();
        let zipper = apply_with(self.zipper.clone(), action, &mut fresh, &mut names)?;
        Some(EditState {
            zipper,
            fresh,
            names,
        })
    }

    pub fn apply_mut(&mut self, action: Action) -> bool {
        match self.apply(action) {
            Some(next) => {
                *self = next;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate;
    use crate::zipper::{all_positions, arity};
    use nothing_core::examples;
    use proptest::prelude::*;

    fn all_examples() -> Vec<(&'static str, Exp)> {
        vec![
            ("let_identity", examples::let_identity()),
            ("increment_applied", examples::increment_applied()),
            ("clamp_to_one", examples::clamp_to_one()),
            ("pair_and_project", examples::pair_and_project()),
            ("pair_with_empty_hole", examples::pair_with_empty_hole()),
            ("add_with_empty_hole", examples::add_with_empty_hole()),
            ("square_and_compare", examples::square_and_compare()),
            (
                "identity_hole_annotated_applied",
                examples::identity_hole_annotated_applied(),
            ),
            (
                "add_with_non_empty_hole",
                examples::add_with_non_empty_hole(),
            ),
            (
                "if_over_pairs_with_hole",
                examples::if_over_pairs_with_hole(),
            ),
        ]
    }

    #[test]
    fn movement_actions_relocate_the_cursor() {
        let e = examples::increment_applied();
        let z = unzip(e.clone());

        let fun = apply(z.clone(), Action::MoveChild(0)).unwrap();
        assert!(matches!(fun.focus, Exp::Lam(..)));

        let arg = apply(fun.clone(), Action::MoveNextSibling).unwrap();
        assert_eq!(arg.focus, Exp::num(41));

        let back = apply(arg.clone(), Action::MovePrevSibling).unwrap();
        assert_eq!(back, fun);

        let root = apply(arg, Action::MoveParent).unwrap();
        assert_eq!(root.focus, e);
    }

    #[test]
    fn out_of_range_movement_returns_none_rather_than_panicking() {
        let z = unzip(examples::add_with_empty_hole());
        assert!(apply(z.clone(), Action::MoveChild(2)).is_none());
        assert!(apply(z.clone(), Action::MoveChild(usize::MAX)).is_none());
        assert!(apply(z.clone(), Action::MoveParent).is_none());
        assert!(apply(z.clone(), Action::MoveNextSibling).is_none());
        assert!(apply(z, Action::MovePrevSibling).is_none());
    }

    #[test]
    fn movement_never_reaches_children_of_a_leaf() {
        let z = unzip(Exp::num(1));
        for n in 0..4 {
            assert!(apply(z.clone(), Action::MoveChild(n)).is_none());
        }
    }

    #[test]
    fn deleting_any_subexpression_of_any_example_stays_well_typed() {
        let mut positions_checked = 0;
        for (name, e) in all_examples() {
            assert!(is_well_typed(&e), "{name} was not well-typed to begin with");
            for z in all_positions(&e) {
                let after = apply(z.clone(), Action::Delete)
                    .expect("Delete applies at every cursor position");
                let program = after.to_exp();
                assert!(
                    is_well_typed(&program),
                    "deleting at depth {} of {name} broke well-typedness: {program:?}",
                    z.depth()
                );
                assert!(
                    matches!(after.focus, Exp::EmptyHole(_)),
                    "Delete must leave the cursor on the new empty hole"
                );
                positions_checked += 1;
            }
        }

        assert!(
            positions_checked >= 50,
            "only {positions_checked} cursor positions were checked"
        );
    }

    #[test]
    fn delete_at_the_root_yields_a_bare_hole() {
        let z = unzip(examples::square_and_compare());
        let after = apply(z, Action::Delete).unwrap();
        assert!(matches!(after.to_exp(), Exp::EmptyHole(_)));
    }

    #[test]
    fn delete_uses_a_hole_id_not_already_in_the_program() {
        let z = unzip(examples::add_with_empty_hole())
            .move_child(0)
            .unwrap();
        let after = apply(z, Action::Delete).unwrap();
        match after.focus {
            Exp::EmptyHole(h) => assert_ne!(h, HoleId::from_u128(0)),
            other => panic!("expected an empty hole, got {other:?}"),
        }
    }

    #[test]
    fn repeated_deletes_in_a_session_never_reuse_a_hole_id() {
        let mut state = EditState::new(Exp::pair(Exp::num(1), Exp::num(2)));
        assert!(state.apply_mut(Action::MoveChild(0)));
        assert!(state.apply_mut(Action::Delete));
        assert!(state.apply_mut(Action::MoveNextSibling));
        assert!(state.apply_mut(Action::Delete));

        match state.exp() {
            Exp::Pair(l, r) => match (*l, *r) {
                (Exp::EmptyHole(a), Exp::EmptyHole(b)) => assert_ne!(a, b),
                other => panic!("expected two empty holes, got {other:?}"),
            },
            other => panic!("expected a pair, got {other:?}"),
        }
    }

    #[test]
    fn delete_inside_a_non_empty_hole_keeps_the_hole() {
        let z = unzip(examples::add_with_non_empty_hole())
            .move_child(1)
            .unwrap()
            .move_child(0)
            .unwrap();
        assert_eq!(z.focus, Exp::bool_(true));
        let after = apply(z, Action::Delete).unwrap();
        let program = after.to_exp();
        assert!(is_well_typed(&program));
        match program {
            Exp::BinOp(Op::Add, _, rhs) => {
                assert!(
                    matches!(*rhs, Exp::NonEmptyHole(_, ref inner) if matches!(**inner, Exp::EmptyHole(_)))
                );
            }
            other => panic!("expected `1 + ⦇⦈⦈`, got {other:?}"),
        }
    }

    #[test]
    fn delete_weakens_a_binding_to_hole_without_breaking_its_uses() {
        let z = unzip(examples::pair_and_project()).move_child(0).unwrap();
        let program = apply(z, Action::Delete).unwrap().to_exp();
        assert!(is_well_typed(&program));
        match program {
            Exp::Let(_, bound, body) => {
                assert!(matches!(*bound, Exp::EmptyHole(_)));
                assert!(matches!(*body, Exp::Proj(Side::L, _)));
            }
            other => panic!("expected a let, got {other:?}"),
        }
    }

    fn arrow(a: Ty, b: Ty) -> Ty {
        Ty::Arrow(Box::new(a), Box::new(b))
    }

    fn prod(a: Ty, b: Ty) -> Ty {
        Ty::Prod(Box::new(a), Box::new(b))
    }

    fn hole(n: u128) -> Exp {
        Exp::empty_hole(HoleId::from_u128(n))
    }

    #[test]
    fn expected_ty_at_the_root_is_unconstrained() {
        assert_eq!(expected_ty_at(&unzip(examples::let_identity())), Ty::Hole);
    }

    #[test]
    fn expected_ty_at_a_binop_operand_is_num() {
        let root = unzip(examples::add_with_empty_hole());
        assert_eq!(
            expected_ty_at(&root.clone().move_child(0).unwrap()),
            Ty::Num
        );
        assert_eq!(expected_ty_at(&root.move_child(1).unwrap()), Ty::Num);
    }

    #[test]
    fn expected_ty_at_an_if_scrutinee_is_bool() {
        let z = unzip(examples::if_over_pairs_with_hole())
            .move_child(0)
            .unwrap();
        assert_eq!(expected_ty_at(&z), Ty::Bool);
    }

    #[test]
    fn expected_ty_at_an_if_branch_comes_from_the_other_branch() {
        let e = Exp::if_(Exp::bool_(true), hole(0), Exp::num(2));
        let root = unzip(e);
        assert_eq!(
            expected_ty_at(&root.clone().move_child(1).unwrap()),
            Ty::Num
        );
        assert_eq!(expected_ty_at(&root.move_child(2).unwrap()), Ty::Hole);
    }

    #[test]
    fn expected_ty_at_an_application_argument_is_the_functions_input() {
        let x = Id::from_u128(0);
        let e = Exp::ap(Exp::lam(x, Ty::Num, Exp::var(x)), hole(0));
        let root = unzip(e);
        assert_eq!(
            expected_ty_at(&root.clone().move_child(1).unwrap()),
            Ty::Num
        );

        assert_eq!(
            expected_ty_at(&root.move_child(0).unwrap()),
            arrow(Ty::Hole, Ty::Hole)
        );
    }

    #[test]
    fn expected_ty_is_pushed_through_an_application_into_a_lambda_body() {
        let f = Id::from_u128(0);
        let x = Id::from_u128(1);
        let e = Exp::ap(
            Exp::lam(f, arrow(Ty::Num, Ty::Bool), Exp::var(f)),
            Exp::lam(x, Ty::Hole, hole(0)),
        );
        assert!(is_well_typed(&e));
        let body = unzip(e).move_child(1).unwrap().move_child(0).unwrap();
        assert_eq!(expected_ty_at(&body), Ty::Bool);

        assert_eq!(ctx_at(&body).lookup(&x), Some(Ty::Hole));
    }

    #[test]
    fn expected_ty_is_pushed_into_a_pair_component() {
        let p = Id::from_u128(0);
        let e = Exp::ap(
            Exp::lam(p, prod(Ty::Num, Ty::Bool), Exp::var(p)),
            Exp::pair(hole(0), hole(1)),
        );
        let arg = unzip(e).move_child(1).unwrap();
        assert_eq!(expected_ty_at(&arg.clone().move_child(0).unwrap()), Ty::Num);
        assert_eq!(expected_ty_at(&arg.move_child(1).unwrap()), Ty::Bool);
    }

    #[test]
    fn expected_ty_under_a_projection_is_a_product() {
        let z = unzip(Exp::proj(Side::L, hole(0))).move_child(0).unwrap();
        assert_eq!(expected_ty_at(&z), prod(Ty::Hole, Ty::Hole));

        let z = unzip(Exp::bin_op(
            Op::Add,
            Exp::num(1),
            Exp::proj(Side::R, hole(0)),
        ))
        .move_child(1)
        .unwrap()
        .move_child(0)
        .unwrap();
        assert_eq!(expected_ty_at(&z), prod(Ty::Hole, Ty::Num));
    }

    #[test]
    fn nothing_is_expected_of_a_holes_contents() {
        let z = unzip(examples::add_with_non_empty_hole())
            .move_child(1)
            .unwrap()
            .move_child(0)
            .unwrap();
        assert_eq!(expected_ty_at(&z), Ty::Hole);
    }

    #[test]
    fn nothing_is_expected_of_a_let_bound_expression() {
        let z = unzip(examples::pair_and_project()).move_child(0).unwrap();
        assert_eq!(expected_ty_at(&z), Ty::Hole);
    }

    #[test]
    fn ctx_at_agrees_with_the_zippers_own_context_walk() {
        for (name, e) in all_examples() {
            for z in all_positions(&e) {
                assert_eq!(
                    ctx_and_expected_ty_at(&z).0,
                    z.ctx(),
                    "the two context walks disagree at depth {} of {name}",
                    z.depth()
                );
                assert_eq!(ctx_at(&z), z.ctx());
            }
        }
    }

    #[test]
    fn construct_num_writes_a_literal_and_keeps_the_cursor_on_it() {
        let state = EditState::empty().apply(Action::ConstructNum(7)).unwrap();
        assert_eq!(state.exp(), Exp::num(7));
        assert_eq!(state.zipper.focus, Exp::num(7));
        assert!(state.zipper.is_root(), "the cursor stays where it was");
    }

    #[test]
    fn construct_bool_writes_a_literal_and_keeps_the_cursor_on_it() {
        let state = EditState::empty()
            .apply(Action::ConstructBool(true))
            .unwrap();
        assert_eq!(state.exp(), Exp::bool_(true));
        assert_eq!(state.zipper.focus, Exp::bool_(true));
        assert!(state.zipper.is_root());
    }

    #[test]
    fn construct_var_writes_an_in_scope_reference_and_keeps_the_cursor_on_it() {
        let x = Id::from_u128(0);
        let z = unzip(Exp::lam(x, Ty::Num, hole(0))).move_child(0).unwrap();
        let after = apply(z, Action::ConstructVar(x)).unwrap();

        assert_eq!(after.focus, Exp::var(x));
        assert_eq!(after.child_index(), Some(0));
        assert_eq!(after.to_exp(), Exp::lam(x, Ty::Num, Exp::var(x)));
        assert!(is_well_typed(&after.to_exp()));
    }

    #[test]
    fn construct_var_fails_cleanly_when_the_binder_is_not_in_scope() {
        let z = unzip(hole(0));
        assert!(apply(z, Action::ConstructVar(Id::from_u128(3))).is_none());
    }

    #[test]
    fn construct_lam_builds_an_unannotated_binder_with_the_cursor_in_the_body() {
        let state = EditState::empty().apply(Action::ConstructLam).unwrap();
        match state.exp() {
            Exp::Lam(_, ann, body) => {
                assert_eq!(ann, Ty::Hole, "a new lambda starts unannotated");
                assert!(matches!(*body, Exp::EmptyHole(_)));
            }
            other => panic!("expected a lambda, got {other:?}"),
        }
        assert!(matches!(state.zipper.focus, Exp::EmptyHole(_)));
        assert_eq!(state.zipper.child_index(), Some(0), "cursor in the body");
        assert!(is_well_typed(&state.exp()));
    }

    #[test]
    fn construct_ap_on_a_hole_leaves_the_cursor_on_the_function() {
        let state = EditState::empty().apply(Action::ConstructAp).unwrap();
        match state.exp() {
            Exp::Ap(fun, arg) => match (*fun, *arg) {
                (Exp::EmptyHole(a), Exp::EmptyHole(b)) => assert_ne!(a, b),
                other => panic!("expected two empty holes, got {other:?}"),
            },
            other => panic!("expected an application, got {other:?}"),
        }

        assert_eq!(state.zipper.child_index(), Some(0));
        assert!(matches!(state.zipper.focus, Exp::EmptyHole(_)));
        assert!(is_well_typed(&state.exp()));
    }

    #[test]
    fn construct_binop_on_a_hole_leaves_the_cursor_on_the_left_operand() {
        let state = EditState::empty()
            .apply(Action::ConstructBinOp(Op::Mul))
            .unwrap();
        match state.exp() {
            Exp::BinOp(Op::Mul, l, r) => {
                assert!(matches!(*l, Exp::EmptyHole(_)));
                assert!(matches!(*r, Exp::EmptyHole(_)));
            }
            other => panic!("expected a multiplication, got {other:?}"),
        }
        assert_eq!(state.zipper.child_index(), Some(0));
        assert!(is_well_typed(&state.exp()));
    }

    #[test]
    fn construct_if_on_a_hole_leaves_the_cursor_on_the_scrutinee() {
        let state = EditState::empty().apply(Action::ConstructIf).unwrap();
        match state.exp() {
            Exp::If(c, t, e) => {
                assert!(matches!(*c, Exp::EmptyHole(_)));
                assert!(matches!(*t, Exp::EmptyHole(_)));
                assert!(matches!(*e, Exp::EmptyHole(_)));
            }
            other => panic!("expected a conditional, got {other:?}"),
        }
        assert_eq!(state.zipper.child_index(), Some(0));
        assert!(is_well_typed(&state.exp()));
    }

    #[test]
    fn construct_let_on_a_hole_leaves_the_cursor_on_the_bound_expression() {
        let state = EditState::empty().apply(Action::ConstructLet).unwrap();
        match state.exp() {
            Exp::Let(_, bound, body) => {
                assert!(matches!(*bound, Exp::EmptyHole(_)));
                assert!(matches!(*body, Exp::EmptyHole(_)));
            }
            other => panic!("expected a let, got {other:?}"),
        }
        assert_eq!(state.zipper.child_index(), Some(0));
        assert!(is_well_typed(&state.exp()));
    }

    #[test]
    fn construct_pair_on_a_hole_leaves_the_cursor_on_the_first_component() {
        let state = EditState::empty().apply(Action::ConstructPair).unwrap();
        match state.exp() {
            Exp::Pair(fst, snd) => {
                assert!(matches!(*fst, Exp::EmptyHole(_)));
                assert!(matches!(*snd, Exp::EmptyHole(_)));
            }
            other => panic!("expected a pair, got {other:?}"),
        }
        assert_eq!(state.zipper.child_index(), Some(0));
        assert!(is_well_typed(&state.exp()));
    }

    #[test]
    fn construct_proj_on_a_hole_leaves_the_cursor_on_the_operand() {
        let state = EditState::empty()
            .apply(Action::ConstructProj(Side::R))
            .unwrap();
        match state.exp() {
            Exp::Proj(Side::R, body) => assert!(matches!(*body, Exp::EmptyHole(_))),
            other => panic!("expected a projection, got {other:?}"),
        }
        assert_eq!(state.zipper.child_index(), Some(0));
        assert!(matches!(state.zipper.focus, Exp::EmptyHole(_)));
        assert!(is_well_typed(&state.exp()));
    }

    #[test]
    fn a_binder_constructed_twice_gets_two_distinct_ids() {
        let mut state = EditState::empty();
        assert!(state.apply_mut(Action::ConstructLet));
        assert!(state.apply_mut(Action::MoveNextSibling));
        assert!(state.apply_mut(Action::ConstructLam));
        match state.exp() {
            Exp::Let(a, _, body) => match *body {
                Exp::Lam(b, _, _) => assert_ne!(a, b),
                other => panic!("expected a lambda in the body, got {other:?}"),
            },
            other => panic!("expected a let, got {other:?}"),
        }
    }

    #[test]
    fn construct_binop_wraps_focus() {
        let z = unzip(Exp::num(1));
        let after = apply(z, Action::ConstructBinOp(Op::Add)).unwrap();

        match after.to_exp() {
            Exp::BinOp(Op::Add, l, r) => {
                assert_eq!(*l, Exp::num(1), "the focus must not be discarded");
                assert!(matches!(*r, Exp::EmptyHole(_)));
            }
            other => panic!("expected `1 + ⦇⦈`, got {other:?}"),
        }

        assert!(matches!(after.focus, Exp::EmptyHole(_)));
        assert_eq!(after.child_index(), Some(1));
        assert!(is_well_typed(&after.to_exp()));
    }

    #[test]
    fn construct_ap_wraps_focus() {
        let x = Id::from_u128(0);
        let f = Exp::lam(x, Ty::Num, Exp::var(x));
        let after = apply(unzip(f.clone()), Action::ConstructAp).unwrap();

        match after.to_exp() {
            Exp::Ap(fun, arg) => {
                assert_eq!(*fun, f, "the focus must not be discarded");
                assert!(matches!(*arg, Exp::EmptyHole(_)));
            }
            other => panic!("expected an application, got {other:?}"),
        }
        assert!(matches!(after.focus, Exp::EmptyHole(_)));
        assert_eq!(after.child_index(), Some(1), "cursor in the argument");
        assert!(is_well_typed(&after.to_exp()));
    }

    #[test]
    fn wrapping_does_not_drag_the_cursor_back_into_the_wrapped_expression() {
        let f = Exp::ap(hole(0), hole(1));
        let after = apply(unzip(f.clone()), Action::ConstructBinOp(Op::Add)).unwrap();
        assert_eq!(after.child_index(), Some(1));
        match after.to_exp() {
            Exp::BinOp(Op::Add, l, _) => assert_eq!(*l, f),
            other => panic!("expected an addition, got {other:?}"),
        }
    }

    #[test]
    fn construct_if_wraps_the_focus_as_the_scrutinee() {
        let cond = Exp::bin_op(Op::Lt, Exp::num(0), Exp::num(1));
        let after = apply(unzip(cond.clone()), Action::ConstructIf).unwrap();
        match after.to_exp() {
            Exp::If(c, t, e) => {
                assert_eq!(*c, cond);
                assert!(matches!(*t, Exp::EmptyHole(_)));
                assert!(matches!(*e, Exp::EmptyHole(_)));
            }
            other => panic!("expected a conditional, got {other:?}"),
        }
        assert_eq!(after.child_index(), Some(1), "cursor on the then-branch");
        assert!(is_well_typed(&after.to_exp()));
    }

    #[test]
    fn construct_lam_wraps_the_focus_as_the_body() {
        let after = apply(unzip(Exp::num(1)), Action::ConstructLam).unwrap();
        match after.to_exp() {
            Exp::Lam(_, ann, body) => {
                assert_eq!(ann, Ty::Hole);
                assert_eq!(*body, Exp::num(1));
            }
            other => panic!("expected a lambda, got {other:?}"),
        }
        assert!(matches!(after.focus, Exp::Lam(..)));
        assert!(after.is_root());
    }

    #[test]
    fn construct_proj_wraps_the_focus_as_the_operand() {
        let p = Id::from_u128(0);
        let z = unzip(Exp::let_(
            p,
            Exp::pair(Exp::num(1), Exp::bool_(true)),
            Exp::var(p),
        ))
        .move_child(1)
        .unwrap();
        let after = apply(z, Action::ConstructProj(Side::L)).unwrap();
        assert_eq!(
            after.focus,
            Exp::proj(Side::L, Exp::var(p)),
            "no new hole, so the cursor rests on the projection itself"
        );
        match after.to_exp() {
            Exp::Let(_, _, body) => assert_eq!(*body, Exp::proj(Side::L, Exp::var(p))),
            other => panic!("expected a let, got {other:?}"),
        }
        assert!(is_well_typed(&after.to_exp()));
    }

    #[test]
    fn construct_pair_wraps_the_focus_as_the_first_component() {
        let after = apply(unzip(Exp::num(1)), Action::ConstructPair).unwrap();
        match after.to_exp() {
            Exp::Pair(fst, snd) => {
                assert_eq!(*fst, Exp::num(1));
                assert!(matches!(*snd, Exp::EmptyHole(_)));
            }
            other => panic!("expected a pair, got {other:?}"),
        }
        assert_eq!(after.child_index(), Some(1));
    }

    #[test]
    fn construct_let_wraps_the_focus_as_the_bound_expression() {
        let after = apply(unzip(Exp::num(1)), Action::ConstructLet).unwrap();
        match after.to_exp() {
            Exp::Let(_, bound, body) => {
                assert_eq!(*bound, Exp::num(1));
                assert!(matches!(*body, Exp::EmptyHole(_)));
            }
            other => panic!("expected a let, got {other:?}"),
        }
        assert_eq!(after.child_index(), Some(1), "cursor in the body");
    }

    #[test]
    fn typing_one_plus_two_from_an_empty_hole_takes_exactly_three_actions() {
        let actions = [
            Action::ConstructNum(1),
            Action::ConstructBinOp(Op::Add),
            Action::ConstructNum(2),
        ];
        assert_eq!(actions.len(), 3);

        let mut state = EditState::empty();
        for action in actions {
            assert!(state.apply_mut(action.clone()), "{action:?} did not apply");
            assert!(is_well_typed(&state.exp()), "after {action:?}");
        }

        assert_eq!(
            state.exp(),
            Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2)),
            "three actions must produce exactly `1 + 2`"
        );
        assert_eq!(state.zipper.focus, Exp::num(2));
    }

    #[test]
    fn constructing_one_plus_true_quarantines_the_true() {
        let mut state = EditState::empty();
        assert!(state.apply_mut(Action::ConstructNum(1)));
        assert!(state.apply_mut(Action::ConstructBinOp(Op::Add)));
        assert!(
            state.apply_mut(Action::ConstructBool(true)),
            "the user is never told no"
        );

        let program = state.exp();
        assert!(is_well_typed(&program), "{program:?}");
        match program {
            Exp::BinOp(Op::Add, l, r) => {
                assert_eq!(*l, Exp::num(1));
                match *r {
                    Exp::NonEmptyHole(_, inner) => assert_eq!(*inner, Exp::bool_(true)),
                    other => panic!("the `true` should be quarantined, got {other:?}"),
                }
            }
            other => panic!("expected `1 + ⦇true⦈`, got {other:?}"),
        }

        assert!(matches!(state.zipper.focus, Exp::NonEmptyHole(..)));
    }

    #[test]
    fn construct_binop_on_a_bool_focus_quarantines_the_bool() {
        let after = apply(unzip(Exp::bool_(true)), Action::ConstructBinOp(Op::Add)).unwrap();
        let program = after.to_exp();
        assert!(is_well_typed(&program), "{program:?}");
        match program {
            Exp::BinOp(Op::Add, l, r) => {
                match *l {
                    Exp::NonEmptyHole(_, inner) => assert_eq!(*inner, Exp::bool_(true)),
                    other => panic!("expected the bool to be quarantined, got {other:?}"),
                }
                assert!(matches!(*r, Exp::EmptyHole(_)));
            }
            other => panic!("expected `⦇true⦈ + ⦇⦈`, got {other:?}"),
        }

        assert_eq!(after.child_index(), Some(1));
    }

    #[test]
    fn construct_ap_on_a_non_function_quarantines_it() {
        let after = apply(unzip(Exp::num(1)), Action::ConstructAp).unwrap();
        assert!(is_well_typed(&after.to_exp()));
        match after.to_exp() {
            Exp::Ap(fun, _) => assert!(matches!(*fun, Exp::NonEmptyHole(..))),
            other => panic!("expected an application, got {other:?}"),
        }
    }

    #[test]
    fn a_quarantined_form_is_still_entered_by_the_cursor() {
        let z = unzip(examples::add_with_empty_hole())
            .move_child(1)
            .unwrap();
        let after = apply(z, Action::ConstructPair).unwrap();
        assert!(is_well_typed(&after.to_exp()));
        match after.to_exp() {
            Exp::BinOp(Op::Add, _, r) => match *r {
                Exp::NonEmptyHole(_, inner) => assert!(matches!(*inner, Exp::Pair(..))),
                other => panic!("expected the pair to be quarantined, got {other:?}"),
            },
            other => panic!("expected an addition, got {other:?}"),
        }
        assert!(matches!(after.focus, Exp::EmptyHole(_)));
        assert_eq!(after.child_index(), Some(0), "inside the pair");
        assert_eq!(after.depth(), 3, "binop → quarantine → pair component");
    }

    #[test]
    fn a_construction_that_fits_is_not_quarantined() {
        let z = unzip(examples::add_with_empty_hole())
            .move_child(1)
            .unwrap();
        let after = apply(z, Action::ConstructNum(2)).unwrap();
        assert_eq!(
            after.to_exp(),
            Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2))
        );
    }

    #[test]
    fn quarantine_also_catches_a_branch_that_would_not_join() {
        let e = Exp::if_(Exp::bool_(true), hole(0), Exp::bool_(true));
        assert!(is_well_typed(&e));
        let z = unzip(e).move_child(1).unwrap();
        let after = apply(z, Action::ConstructNum(1)).unwrap();
        assert!(is_well_typed(&after.to_exp()), "{:?}", after.to_exp());
        match after.to_exp() {
            Exp::If(_, then, _) => assert!(matches!(*then, Exp::NonEmptyHole(..))),
            other => panic!("expected a conditional, got {other:?}"),
        }
    }

    fn contains_a_hole(e: &Exp) -> bool {
        match e {
            Exp::EmptyHole(_) | Exp::NonEmptyHole(..) => true,
            Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) => false,
            Exp::Lam(_, _, b) | Exp::Proj(_, b) => contains_a_hole(b),
            Exp::Ap(a, b) | Exp::BinOp(_, a, b) | Exp::Let(_, a, b) | Exp::Pair(a, b) => {
                contains_a_hole(a) || contains_a_hole(b)
            }
            Exp::If(c, t, e) => contains_a_hole(c) || contains_a_hole(t) || contains_a_hole(e),
        }
    }

    #[test]
    fn a_non_empty_hole_edited_until_it_fits_can_be_finished() {
        let mut state = EditState::new(examples::add_with_non_empty_hole());
        assert!(contains_a_hole(&state.exp()));

        assert!(state.apply_mut(Action::MoveChild(1)));
        assert!(state.apply_mut(Action::MoveChild(0)));
        assert!(state.apply_mut(Action::Delete));
        assert!(state.apply_mut(Action::ConstructNum(2)));
        assert!(state.apply_mut(Action::MoveParent));
        assert!(matches!(state.zipper.focus, Exp::NonEmptyHole(..)));

        assert!(state.apply_mut(Action::Finish));

        let program = state.exp();
        assert_eq!(program, Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2)));
        assert!(is_well_typed(&program));
        assert!(!contains_a_hole(&program), "no hole left: {program:?}");
        assert_eq!(state.zipper.focus, Exp::num(2), "cursor on the contents");
    }

    #[test]
    fn finish_refuses_while_the_contents_still_do_not_fit() {
        let state = EditState::new(examples::add_with_non_empty_hole());
        let at_hole = state.apply(Action::MoveChild(1)).unwrap();
        assert!(matches!(at_hole.zipper.focus, Exp::NonEmptyHole(..)));
        assert!(at_hole.apply(Action::Finish).is_none());
    }

    #[test]
    fn finish_does_not_apply_off_a_non_empty_hole() {
        for (name, e) in all_examples() {
            for z in all_positions(&e) {
                let is_non_empty_hole = matches!(z.focus, Exp::NonEmptyHole(..));
                let finished = apply(z.clone(), Action::Finish);
                if !is_non_empty_hole {
                    assert!(
                        finished.is_none(),
                        "Finish applied off a non-empty hole in {name}"
                    );
                }
                if let Some(after) = finished {
                    assert!(is_well_typed(&after.to_exp()));
                }
            }
        }
    }

    #[test]
    fn finish_undoes_an_automatic_quarantine() {
        let mut state = EditState::empty();
        assert!(state.apply_mut(Action::ConstructNum(1)));
        assert!(state.apply_mut(Action::ConstructBinOp(Op::Add)));
        assert!(state.apply_mut(Action::ConstructBool(true)));
        assert!(state.apply_mut(Action::MoveChild(0)));
        assert!(state.apply_mut(Action::ConstructNum(2)));
        assert!(state.apply_mut(Action::MoveParent));
        assert!(state.apply_mut(Action::Finish));
        assert_eq!(state.exp(), Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2)));
    }

    #[test]
    fn edit_state_leaves_itself_untouched_when_an_action_does_not_apply() {
        let mut state = EditState::new(examples::let_identity());
        let before = state.clone();
        assert!(!state.apply_mut(Action::MoveParent));
        assert_eq!(state, before);
    }

    #[test]
    fn empty_edit_state_is_a_well_typed_hole() {
        let state = EditState::empty();
        assert!(is_well_typed(&state.exp()));
        assert!(matches!(state.exp(), Exp::EmptyHole(_)));
    }

    #[test]
    fn fresh_from_program_never_reissues_an_id_the_program_already_uses() {
        let e = Exp::let_(
            Id::from_u128(7),
            Exp::empty_hole(HoleId::from_u128(12)),
            Exp::non_empty_hole(HoleId::from_u128(3), Exp::var(Id::from_u128(7))),
        );
        let taken = [7u128, 12, 3];
        let mut fresh = Fresh::from_program(&e);

        let mut issued: Vec<u128> = Vec::new();
        for _ in 0..100 {
            issued.push(fresh.hole().as_u128());
            issued.push(fresh.id().as_u128());
        }

        for bits in &issued {
            assert!(!taken.contains(bits), "{bits:#x} is already in the program");
        }
        for (i, bits) in issued.iter().enumerate() {
            assert!(!issued[..i].contains(bits), "{bits:#x} was issued twice");
        }

        assert_eq!(
            Fresh::from_program(&e).id(),
            Fresh::from_program(&e).id(),
            "a session's fresh ids must be reproducible, or the log cannot replay"
        );
    }

    fn every_construction() -> Vec<Action> {
        vec![
            Action::ConstructNum(1),
            Action::ConstructBool(true),
            Action::ConstructVar(Id::from_u128(0)),
            Action::ConstructLam,
            Action::ConstructAp,
            Action::ConstructBinOp(Op::Add),
            Action::ConstructBinOp(Op::Lt),
            Action::ConstructIf,
            Action::ConstructLet,
            Action::ConstructPair,
            Action::ConstructProj(Side::L),
            Action::ConstructProj(Side::R),
            Action::Finish,
        ]
    }

    fn contains_subexpression(haystack: &Exp, needle: &Exp) -> bool {
        if haystack == needle {
            return true;
        }
        (0..arity(haystack)).any(|i| {
            let child = unzip(haystack.clone()).move_child(i).expect("i < arity");
            contains_subexpression(&child.focus, needle)
        })
    }

    fn movement(byte: u8) -> Action {
        match byte % 6 {
            0 => Action::MoveChild(0),
            1 => Action::MoveChild(1),
            2 => Action::MoveChild(2),
            3 => Action::MoveParent,
            4 => Action::MoveNextSibling,
            _ => Action::MovePrevSibling,
        }
    }

    proptest! {
        #[test]
        fn movement_changes_only_the_focus(
            seed in any::<u64>(),
            moves in prop::collection::vec(any::<u8>(), 0..40),
        ) {
            let e = generate::well_typed_exp(seed);
            let mut z = unzip(e.clone());
            for byte in moves {
                let action = movement(byte);
                if let Some(next) = apply(z.clone(), action) {


                    prop_assert_eq!(next.to_exp(), e.clone());
                    z = next;
                }
            }
            prop_assert_eq!(z.to_exp(), e);
        }

        #[test]
        fn movement_fails_cleanly_at_the_edges(seed in any::<u64>()) {
            let e = generate::well_typed_exp(seed);
            for z in all_positions(&e) {
                let n = arity(&z.focus);
                prop_assert!(apply(z.clone(), Action::MoveChild(n)).is_none());
                prop_assert!(apply(z.clone(), Action::MoveChild(n + 1)).is_none());
                prop_assert!(apply(z.clone(), Action::MoveChild(usize::MAX)).is_none());
                if z.is_root() {
                    prop_assert!(apply(z.clone(), Action::MoveParent).is_none());
                    prop_assert!(apply(z.clone(), Action::MoveNextSibling).is_none());
                    prop_assert!(apply(z.clone(), Action::MovePrevSibling).is_none());
                } else {
                    prop_assert!(apply(z.clone(), Action::MoveParent).is_some());
                }
            }
        }

        #[test]
        fn delete_preserves_well_typedness_everywhere(seed in any::<u64>()) {
            let e = generate::well_typed_exp(seed);
            prop_assert!(is_well_typed(&e));
            for z in all_positions(&e) {
                let after = apply(z, Action::Delete).expect("Delete always applies");
                prop_assert!(is_well_typed(&after.to_exp()), "{:?}", after.to_exp());
            }
        }

        #[test]
        fn interleaved_movement_and_deletion_stays_well_typed(
            seed in any::<u64>(),
            steps in prop::collection::vec(any::<u8>(), 0..40),
        ) {
            let e = generate::well_typed_exp(seed);
            let mut state = EditState::new(e);
            for byte in steps {

                let action = if byte % 7 == 0 { Action::Delete } else { movement(byte) };
                state.apply_mut(action);
                prop_assert!(is_well_typed(&state.exp()), "{:?}", state.exp());
            }
        }

        #[test]
        fn construction_anywhere_preserves_well_typedness(seed in any::<u64>()) {
            let e = generate::well_typed_exp(seed);
            for z in all_positions(&e) {
                for action in every_construction() {
                    match apply(z.clone(), action.clone()) {
                        Some(after) => prop_assert!(
                            is_well_typed(&after.to_exp()),
                            "{action:?} at depth {} produced {:?}",
                            z.depth(),
                            after.to_exp()
                        ),
                        None => prop_assert!(
                            matches!(action, Action::ConstructVar(_) | Action::Finish),
                            "{action:?} was refused at depth {}",
                            z.depth()
                        ),
                    }
                }
            }
        }

        #[test]
        fn wrapping_constructions_never_discard_the_focus(seed in any::<u64>()) {
            let wrapping = [
                Action::ConstructLam,
                Action::ConstructAp,
                Action::ConstructBinOp(Op::Add),
                Action::ConstructIf,
                Action::ConstructLet,
                Action::ConstructPair,
                Action::ConstructProj(Side::L),
            ];
            let e = generate::well_typed_exp(seed);
            for z in all_positions(&e) {
                for action in wrapping.clone() {
                    let before = z.focus.clone();
                    let after = apply(z.clone(), action.clone()).expect("wrapping always applies");
                    prop_assert!(
                        contains_subexpression(&after.to_exp(), &before),
                        "{action:?} discarded {before:?}"
                    );
                }
            }
        }

        #[test]
        fn interleaved_editing_stays_well_typed(
            seed in any::<u64>(),
            steps in prop::collection::vec(any::<u8>(), 0..60),
        ) {
            let e = generate::well_typed_exp(seed);
            let mut state = EditState::new(e);
            let constructions = every_construction();
            for byte in steps {
                let action = match byte % 3 {
                    0 => movement(byte / 3),
                    1 => Action::Delete,
                    _ => constructions[(byte / 3) as usize % constructions.len()].clone(),
                };
                state.apply_mut(action.clone());
                prop_assert!(
                    is_well_typed(&state.exp()),
                    "after {action:?}: {:?}",
                    state.exp()
                );
            }
        }

        #[test]
        fn delete_leaves_the_cursor_on_a_fresh_hole(seed in any::<u64>()) {
            let e = generate::well_typed_exp(seed);
            for z in all_positions(&e) {
                let after = apply(z, Action::Delete).unwrap();
                prop_assert!(matches!(after.focus, Exp::EmptyHole(_)));
            }
        }
    }

    #[test]
    fn hole_type_is_what_delete_leaves_behind() {
        let after = apply(unzip(Exp::num(1)), Action::Delete).unwrap();
        assert_eq!(syn(&Ctx::empty(), &after.to_exp()), Some(Ty::Hole));
    }
}
