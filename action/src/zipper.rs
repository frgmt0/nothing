use nothing_core::ctx::Ctx;
use nothing_core::exp::{Exp, HoleId, Id, Op, Side};
use nothing_core::ty::Ty;
use nothing_core::ty::matched_cmd;
use nothing_core::typing::{arm_payload_ty, syn};

#[derive(Clone, PartialEq, Debug)]
pub enum Frame {
    LamBody(Id, Ty),
    ApFun(Exp),
    ApArg(Exp),
    BinOpLeft(Op, Exp),
    BinOpRight(Op, Exp),
    IfCond(Exp, Exp),
    IfThen(Exp, Exp),
    IfElse(Exp, Exp),
    LetBound(Id, Exp),
    LetBody(Id, Exp),
    PairFst(Exp),
    PairSnd(Exp),
    ProjBody(Side),
    ConsHead(Exp),
    ConsTail(Exp),
    FoldList(Exp, Exp),
    FoldInit(Exp, Exp),
    FoldStep(Exp, Exp),
    RecordField(Vec<(Id, Exp)>, usize, Id),
    FieldSubject(Id),
    InjPayload(Id),
    MatchScrutinee(Vec<(Id, Id, Exp)>),
    MatchArm(Exp, Vec<(Id, Id, Exp)>, usize, Id, Id),
    PrintText,
    PureValue,
    BindCommand(Id, Exp),
    BindBody(Id, Exp),
    NonEmptyHoleBody(HoleId),
}

impl Frame {
    pub fn rebuild(self, focus: Exp) -> Exp {
        match self {
            Frame::LamBody(id, ty) => Exp::Lam(id, ty, Box::new(focus)),
            Frame::ApFun(arg) => Exp::Ap(Box::new(focus), Box::new(arg)),
            Frame::ApArg(fun) => Exp::Ap(Box::new(fun), Box::new(focus)),
            Frame::BinOpLeft(op, rhs) => Exp::BinOp(op, Box::new(focus), Box::new(rhs)),
            Frame::BinOpRight(op, lhs) => Exp::BinOp(op, Box::new(lhs), Box::new(focus)),
            Frame::IfCond(then, else_) => Exp::If(Box::new(focus), Box::new(then), Box::new(else_)),
            Frame::IfThen(cond, else_) => Exp::If(Box::new(cond), Box::new(focus), Box::new(else_)),
            Frame::IfElse(cond, then) => Exp::If(Box::new(cond), Box::new(then), Box::new(focus)),
            Frame::LetBound(id, body) => Exp::Let(id, Box::new(focus), Box::new(body)),
            Frame::LetBody(id, bound) => Exp::Let(id, Box::new(bound), Box::new(focus)),
            Frame::PairFst(snd) => Exp::Pair(Box::new(focus), Box::new(snd)),
            Frame::PairSnd(fst) => Exp::Pair(Box::new(fst), Box::new(focus)),
            Frame::ProjBody(side) => Exp::Proj(side, Box::new(focus)),
            Frame::ConsHead(tail) => Exp::Cons(Box::new(focus), Box::new(tail)),
            Frame::ConsTail(head) => Exp::Cons(Box::new(head), Box::new(focus)),
            Frame::FoldList(init, step) => {
                Exp::Fold(Box::new(focus), Box::new(init), Box::new(step))
            }
            Frame::FoldInit(list, step) => {
                Exp::Fold(Box::new(list), Box::new(focus), Box::new(step))
            }
            Frame::FoldStep(list, init) => {
                Exp::Fold(Box::new(list), Box::new(init), Box::new(focus))
            }
            Frame::RecordField(others, index, id) => {
                let mut fields = others;
                fields.insert(index, (id, focus));
                Exp::Record(fields)
            }
            Frame::FieldSubject(id) => Exp::Field(Box::new(focus), id),
            Frame::InjPayload(ctor) => Exp::Inj(ctor, Box::new(focus)),
            Frame::MatchScrutinee(arms) => Exp::Match(Box::new(focus), arms),
            Frame::MatchArm(scrutinee, others, index, ctor, binder) => {
                let mut arms = others;
                arms.insert(index, (ctor, binder, focus));
                Exp::Match(Box::new(scrutinee), arms)
            }
            Frame::PrintText => Exp::Print(Box::new(focus)),
            Frame::PureValue => Exp::CmdPure(Box::new(focus)),
            Frame::BindCommand(id, body) => Exp::CmdBind(Box::new(focus), id, Box::new(body)),
            Frame::BindBody(id, command) => Exp::CmdBind(Box::new(command), id, Box::new(focus)),
            Frame::NonEmptyHoleBody(h) => Exp::NonEmptyHole(h, Box::new(focus)),
        }
    }

    pub fn child_index(&self) -> usize {
        if let Frame::RecordField(_, index, _) = self {
            return *index;
        }
        if let Frame::MatchArm(_, _, index, _, _) = self {
            return *index + 1;
        }
        match self {
            Frame::LamBody(..)
            | Frame::ApFun(..)
            | Frame::BinOpLeft(..)
            | Frame::IfCond(..)
            | Frame::LetBound(..)
            | Frame::PairFst(..)
            | Frame::ProjBody(..)
            | Frame::ConsHead(..)
            | Frame::FoldList(..)
            | Frame::FieldSubject(..)
            | Frame::RecordField(..)
            | Frame::InjPayload(..)
            | Frame::MatchScrutinee(..)
            | Frame::MatchArm(..)
            | Frame::PrintText
            | Frame::PureValue
            | Frame::BindCommand(..)
            | Frame::NonEmptyHoleBody(..) => 0,
            Frame::ApArg(..)
            | Frame::BinOpRight(..)
            | Frame::IfThen(..)
            | Frame::LetBody(..)
            | Frame::PairSnd(..)
            | Frame::ConsTail(..)
            | Frame::BindBody(..)
            | Frame::FoldInit(..) => 1,
            Frame::IfElse(..) | Frame::FoldStep(..) => 2,
        }
    }

    pub fn parent_arity(&self) -> usize {
        if let Frame::RecordField(others, _, _) = self {
            return others.len() + 1;
        }
        if let Frame::MatchScrutinee(arms) = self {
            return arms.len() + 1;
        }
        if let Frame::MatchArm(_, others, _, _, _) = self {
            return others.len() + 2;
        }
        match self {
            Frame::LamBody(..)
            | Frame::ProjBody(..)
            | Frame::FieldSubject(..)
            | Frame::RecordField(..)
            | Frame::InjPayload(..)
            | Frame::MatchScrutinee(..)
            | Frame::MatchArm(..)
            | Frame::PrintText
            | Frame::PureValue
            | Frame::NonEmptyHoleBody(..) => 1,
            Frame::ApFun(..)
            | Frame::ApArg(..)
            | Frame::BinOpLeft(..)
            | Frame::BinOpRight(..)
            | Frame::LetBound(..)
            | Frame::LetBody(..)
            | Frame::PairFst(..)
            | Frame::PairSnd(..)
            | Frame::ConsHead(..)
            | Frame::ConsTail(..)
            | Frame::BindCommand(..)
            | Frame::BindBody(..) => 2,
            Frame::IfCond(..)
            | Frame::IfThen(..)
            | Frame::IfElse(..)
            | Frame::FoldList(..)
            | Frame::FoldInit(..)
            | Frame::FoldStep(..) => 3,
        }
    }
}

pub fn arity(exp: &Exp) -> usize {
    match exp {
        Exp::Var(_)
        | Exp::Num(_)
        | Exp::Bool(_)
        | Exp::Str(_)
        | Exp::Nil
        | Exp::Readline
        | Exp::EmptyHole(_) => 0,
        Exp::Record(fields) => fields.len(),
        Exp::Match(_, arms) => arms.len() + 1,
        Exp::Lam(..)
        | Exp::Proj(..)
        | Exp::Field(..)
        | Exp::Inj(..)
        | Exp::Print(..)
        | Exp::CmdPure(..)
        | Exp::NonEmptyHole(..) => 1,
        Exp::Ap(..)
        | Exp::BinOp(..)
        | Exp::Let(..)
        | Exp::Pair(..)
        | Exp::Cons(..)
        | Exp::CmdBind(..) => 2,
        Exp::If(..) | Exp::Fold(..) => 3,
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Zipper {
    pub focus: Exp,
    pub path: Vec<Frame>,
}

pub fn unzip(exp: Exp) -> Zipper {
    Zipper {
        focus: exp,
        path: Vec::new(),
    }
}

pub fn zip(z: Zipper) -> Exp {
    let Zipper { mut focus, path } = z;
    for frame in path.into_iter().rev() {
        focus = frame.rebuild(focus);
    }
    focus
}

impl Zipper {
    pub fn new(exp: Exp) -> Zipper {
        unzip(exp)
    }

    pub fn to_exp(&self) -> Exp {
        zip(self.clone())
    }

    pub fn into_exp(self) -> Exp {
        zip(self)
    }

    pub fn is_root(&self) -> bool {
        self.path.is_empty()
    }

    pub fn depth(&self) -> usize {
        self.path.len()
    }

    pub fn child_index(&self) -> Option<usize> {
        self.path.last().map(Frame::child_index)
    }

    pub fn replace_focus(mut self, exp: Exp) -> Zipper {
        self.focus = exp;
        self
    }

    pub fn move_child(self, n: usize) -> Option<Zipper> {
        if n >= arity(&self.focus) {
            return None;
        }
        let Zipper { focus, mut path } = self;
        let (frame, child) = match focus {
            Exp::Lam(id, ty, body) => (Frame::LamBody(id, ty), *body),
            Exp::Ap(fun, arg) => {
                if n == 0 {
                    (Frame::ApFun(*arg), *fun)
                } else {
                    (Frame::ApArg(*fun), *arg)
                }
            }
            Exp::BinOp(op, lhs, rhs) => {
                if n == 0 {
                    (Frame::BinOpLeft(op, *rhs), *lhs)
                } else {
                    (Frame::BinOpRight(op, *lhs), *rhs)
                }
            }
            Exp::If(cond, then, else_) => match n {
                0 => (Frame::IfCond(*then, *else_), *cond),
                1 => (Frame::IfThen(*cond, *else_), *then),
                _ => (Frame::IfElse(*cond, *then), *else_),
            },
            Exp::Let(id, bound, body) => {
                if n == 0 {
                    (Frame::LetBound(id, *body), *bound)
                } else {
                    (Frame::LetBody(id, *bound), *body)
                }
            }
            Exp::Pair(fst, snd) => {
                if n == 0 {
                    (Frame::PairFst(*snd), *fst)
                } else {
                    (Frame::PairSnd(*fst), *snd)
                }
            }
            Exp::Cons(head, tail) => {
                if n == 0 {
                    (Frame::ConsHead(*tail), *head)
                } else {
                    (Frame::ConsTail(*head), *tail)
                }
            }
            Exp::Fold(list, init, step) => match n {
                0 => (Frame::FoldList(*init, *step), *list),
                1 => (Frame::FoldInit(*list, *step), *init),
                _ => (Frame::FoldStep(*list, *init), *step),
            },
            Exp::Proj(side, inner) => (Frame::ProjBody(side), *inner),
            Exp::Field(subject, id) => (Frame::FieldSubject(id), *subject),
            Exp::Record(mut fields) => {
                let (id, child) = fields.remove(n);
                (Frame::RecordField(fields, n, id), child)
            }
            Exp::Inj(ctor, payload) => (Frame::InjPayload(ctor), *payload),
            Exp::Match(scrutinee, mut arms) => {
                if n == 0 {
                    (Frame::MatchScrutinee(arms), *scrutinee)
                } else {
                    let (ctor, binder, body) = arms.remove(n - 1);
                    (Frame::MatchArm(*scrutinee, arms, n - 1, ctor, binder), body)
                }
            }
            Exp::NonEmptyHole(h, inner) => (Frame::NonEmptyHoleBody(h), *inner),
            Exp::Print(text) => (Frame::PrintText, *text),
            Exp::CmdPure(value) => (Frame::PureValue, *value),
            Exp::CmdBind(command, id, body) => {
                if n == 0 {
                    (Frame::BindCommand(id, *body), *command)
                } else {
                    (Frame::BindBody(id, *command), *body)
                }
            }

            Exp::Var(_)
            | Exp::Num(_)
            | Exp::Bool(_)
            | Exp::Str(_)
            | Exp::Nil
            | Exp::Readline
            | Exp::EmptyHole(_) => {
                return None;
            }
        };
        path.push(frame);
        Some(Zipper { focus: child, path })
    }

    pub fn move_parent(self) -> Option<Zipper> {
        if self.path.is_empty() {
            return None;
        }
        let Zipper { focus, mut path } = self;
        let frame = path.pop().expect("checked non-empty");
        Some(Zipper {
            focus: frame.rebuild(focus),
            path,
        })
    }

    pub fn move_next_sibling(self) -> Option<Zipper> {
        self.move_sibling(1)
    }

    pub fn move_prev_sibling(self) -> Option<Zipper> {
        self.move_sibling(-1)
    }

    fn move_sibling(self, delta: isize) -> Option<Zipper> {
        let frame = self.path.last()?;
        let target = frame.child_index() as isize + delta;
        if target < 0 || target as usize >= frame.parent_arity() {
            return None;
        }
        let target = target as usize;
        self.move_parent()
            .expect("path is non-empty")
            .move_child(target)
    }

    pub fn binders(&self) -> Vec<Id> {
        let ctx = self.ctx();
        self.path
            .iter()
            .filter_map(|frame| match frame {
                Frame::LamBody(id, _) => Some(*id),
                Frame::LetBody(id, _) => Some(*id),
                Frame::MatchArm(_, _, _, _, binder) => Some(*binder),
                Frame::BindBody(id, _) => Some(*id),
                _ => None,
            })
            .filter(|id| ctx.lookup(id).is_some())
            .collect()
    }

    pub fn record_field_id(&self) -> Option<Id> {
        match self.path.last()? {
            Frame::RecordField(_, _, id) => Some(*id),
            _ => None,
        }
    }

    pub fn arm_constructor_id(&self) -> Option<Id> {
        match self.path.last()? {
            Frame::MatchArm(_, _, _, ctor, _) => Some(*ctor),
            _ => None,
        }
    }

    pub fn arm_binder_id(&self) -> Option<Id> {
        match self.path.last()? {
            Frame::MatchArm(_, _, _, _, binder) => Some(*binder),
            _ => None,
        }
    }

    pub fn arm_index(&self) -> Option<usize> {
        match self.path.last()? {
            Frame::MatchArm(_, _, index, _, _) => Some(*index),
            _ => None,
        }
    }

    pub fn injected_constructor_id(&self) -> Option<Id> {
        match &self.focus {
            Exp::Inj(ctor, _) => Some(*ctor),
            _ => None,
        }
    }

    pub fn projected_field_id(&self) -> Option<Id> {
        match &self.focus {
            Exp::Field(_, id) => Some(*id),
            _ => None,
        }
    }

    pub fn binder_id(&self) -> Option<Id> {
        match &self.focus {
            Exp::Lam(id, _, _) | Exp::Let(id, _, _) | Exp::CmdBind(_, id, _) => Some(*id),
            _ => None,
        }
    }

    pub fn ctx(&self) -> Ctx {
        self.ctx_in(&Ctx::empty())
    }

    pub fn ctx_in(&self, base: &Ctx) -> Ctx {
        let mut ctx = base.clone();
        for frame in &self.path {
            match frame {
                Frame::LamBody(id, ty) => ctx = ctx.extend(*id, ty.clone()),
                Frame::LetBody(id, bound) => {
                    let ty = syn(&ctx, bound).unwrap_or(Ty::Hole);
                    ctx = ctx.extend(*id, ty);
                }
                Frame::MatchArm(scrutinee, _, _, ctor, binder) => {
                    let scrutinee_ty = syn(&ctx, scrutinee).unwrap_or(Ty::Hole);
                    ctx = ctx.extend(*binder, arm_payload_ty(&scrutinee_ty, *ctor));
                }
                Frame::BindBody(id, command) => {
                    let command_ty = syn(&ctx, command).unwrap_or(Ty::Hole);
                    ctx = ctx.extend(*id, matched_cmd(&command_ty).unwrap_or(Ty::Hole));
                }

                _ => {}
            }
        }
        ctx
    }
}

pub fn index_path(z: &Zipper) -> Vec<usize> {
    z.path.iter().map(Frame::child_index).collect()
}

pub fn moves_between(from: &Zipper, to: &Zipper) -> Vec<crate::act::Action> {
    use crate::act::Action;
    let from_path = index_path(from);
    let to_path = index_path(to);
    let common = from_path
        .iter()
        .zip(to_path.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let mut actions = vec![Action::MoveParent; from_path.len() - common];
    actions.extend(to_path[common..].iter().map(|&i| Action::MoveChild(i)));
    actions
}

pub fn unfinished_positions(exp: &Exp) -> Vec<Vec<usize>> {
    all_positions(exp)
        .iter()
        .filter(|z| matches!(z.focus, Exp::EmptyHole(_) | Exp::NonEmptyHole(..)))
        .map(index_path)
        .collect()
}

pub fn all_positions(exp: &Exp) -> Vec<Zipper> {
    fn go(z: Zipper, out: &mut Vec<Zipper>) {
        let n = arity(&z.focus);
        out.push(z.clone());
        for i in 0..n {
            let child = z
                .clone()
                .move_child(i)
                .expect("i < arity, so the move cannot fail");
            go(child, out);
        }
    }

    let mut out = Vec::new();
    go(unzip(exp.clone()), &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate;
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
    fn zip_unzip_is_the_identity_on_every_example() {
        for (name, e) in all_examples() {
            assert_eq!(zip(unzip(e.clone())), e, "round-trip failed for {name}");
        }
    }

    #[test]
    fn zip_from_every_position_reproduces_the_program() {
        for (name, e) in all_examples() {
            for z in all_positions(&e) {
                assert_eq!(
                    z.to_exp(),
                    e,
                    "rebuilding from a cursor at depth {} in {name} changed the program",
                    z.depth()
                );
            }
        }
    }

    #[test]
    fn all_positions_visits_every_node_once() {
        for (name, e) in all_examples() {
            assert_eq!(
                all_positions(&e).len(),
                generate::size(&e),
                "position count != node count for {name}"
            );
        }
    }

    #[test]
    fn move_child_out_of_range_fails_cleanly() {
        let e = examples::add_with_empty_hole();
        let z = unzip(e);
        assert!(z.clone().move_child(0).is_some());
        assert!(z.clone().move_child(1).is_some());
        assert!(z.clone().move_child(2).is_none());
        assert!(z.clone().move_child(usize::MAX).is_none());

        let leaf = z.move_child(0).unwrap();
        assert_eq!(leaf.focus, Exp::num(1));
        assert!(leaf.move_child(0).is_none());
    }

    #[test]
    fn move_parent_at_the_root_fails_cleanly() {
        let z = unzip(examples::let_identity());
        assert!(z.move_parent().is_none());
    }

    #[test]
    fn descend_then_ascend_returns_to_the_same_place() {
        for (_, e) in all_examples() {
            let root = unzip(e);
            for i in 0..arity(&root.focus) {
                let there_and_back = root.clone().move_child(i).unwrap().move_parent().unwrap();
                assert_eq!(there_and_back, root);
            }
        }
    }

    #[test]
    fn sibling_movement_walks_the_children_and_stops_at_the_ends() {
        let e = examples::if_over_pairs_with_hole();
        let root = unzip(e);

        let cond = root.clone().move_child(0).unwrap();
        assert!(
            cond.clone().move_prev_sibling().is_none(),
            "no child before 0"
        );

        let then = cond.move_next_sibling().unwrap();
        assert_eq!(then.child_index(), Some(1));
        let else_ = then.clone().move_next_sibling().unwrap();
        assert_eq!(else_.child_index(), Some(2));
        assert!(
            else_.clone().move_next_sibling().is_none(),
            "no child after 2"
        );

        assert_eq!(else_.move_prev_sibling().unwrap(), then);
    }

    #[test]
    fn sibling_movement_at_the_root_fails_cleanly() {
        let z = unzip(examples::let_identity());
        assert!(z.clone().move_next_sibling().is_none());
        assert!(z.move_prev_sibling().is_none());
    }

    #[test]
    fn ctx_at_a_lambda_body_sees_the_parameter() {
        let z = unzip(examples::clamp_to_one()).move_child(0).unwrap();
        assert_eq!(z.ctx().lookup(&examples::binder(0)), Some(Ty::Num));
    }

    #[test]
    fn ctx_at_a_let_body_sees_the_binding_but_the_bound_expression_does_not() {
        let root = unzip(examples::pair_and_project());
        let p = examples::binder(0);

        let bound = root.clone().move_child(0).unwrap();
        assert_eq!(
            bound.ctx().lookup(&p),
            None,
            "a let does not bind its own RHS"
        );

        let body = root.move_child(1).unwrap();
        assert_eq!(
            body.ctx().lookup(&p),
            Some(Ty::Prod(Box::new(Ty::Num), Box::new(Ty::Bool)))
        );
    }

    #[test]
    fn ctx_at_a_bind_body_sees_the_binder_at_whatever_the_command_yields() {
        let line = Id::from_u128(0xB11D);
        let program = Exp::cmd_bind(Exp::readline(), line, Exp::print(Exp::var(line)));
        let root = unzip(program);

        let command = root.clone().move_child(0).unwrap();
        assert_eq!(
            command.ctx().lookup(&line),
            None,
            "a bind does not bind its own command, exactly as a let does not bind its RHS"
        );

        let body = root.move_child(1).unwrap();
        assert_eq!(
            body.ctx().lookup(&line),
            Some(Ty::Str),
            "readline yields a Str, so that is what the binder is"
        );

        let unwritten = unzip(Exp::cmd_bind(
            Exp::empty_hole(HoleId::from_u128(1)),
            line,
            Exp::print(Exp::var(line)),
        ))
        .move_child(1)
        .unwrap();
        assert_eq!(
            unwritten.ctx().lookup(&line),
            Some(Ty::Hole),
            "and an unwritten command yields an unknown, not nothing at all"
        );
    }

    #[test]
    fn ctx_at_the_root_is_empty() {
        assert_eq!(unzip(examples::let_identity()).ctx(), Ctx::empty());
    }

    fn wander(mut z: Zipper, moves: &[u8]) -> Zipper {
        for m in moves {
            let attempt = match m % 6 {
                0 => z.clone().move_child(0),
                1 => z.clone().move_child(1),
                2 => z.clone().move_child(2),
                3 => z.clone().move_parent(),
                4 => z.clone().move_next_sibling(),
                _ => z.clone().move_prev_sibling(),
            };
            if let Some(next) = attempt {
                z = next;
            }
        }
        z
    }

    proptest! {
        #[test]
        fn zip_unzip_roundtrip(seed in any::<u64>()) {
            let e = generate::well_typed_exp(seed);
            prop_assert_eq!(zip(unzip(e.clone())), e);
        }

        #[test]
        fn zip_after_arbitrary_movement_reproduces_the_program(
            seed in any::<u64>(),
            moves in prop::collection::vec(any::<u8>(), 0..40),
        ) {
            let e = generate::well_typed_exp(seed);
            let z = wander(unzip(e.clone()), &moves);
            prop_assert_eq!(z.to_exp(), e);
        }

        #[test]
        fn every_position_of_an_arbitrary_program_rebuilds_it(seed in any::<u64>()) {
            let e = generate::well_typed_exp(seed);
            for z in all_positions(&e) {
                prop_assert_eq!(z.to_exp(), e.clone());
            }
        }

        #[test]
        fn ascending_to_the_root_yields_the_program(seed in any::<u64>()) {
            let e = generate::well_typed_exp(seed);
            for z in all_positions(&e) {
                let mut cur = z;
                while let Some(up) = cur.clone().move_parent() {
                    cur = up;
                }
                prop_assert!(cur.is_root());
                prop_assert_eq!(cur.focus, e.clone());
            }
        }
    }
}
