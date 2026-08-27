//! The zipper cursor (Phase 2).
//!
//! A cursor into a program is a [`Zipper`]: the focused subexpression, plus
//! a path of [`Frame`]s recording, for each ancestor, which child position
//! we descended into and what the *other* children were. That is enough to
//! walk back up and reconstruct the whole program — no parent pointers, no
//! arena, no indices.
//!
//! The spec is explicit about not using indices into a flat arena for v1,
//! and the reason shows up immediately in [`crate::act`]: an action rule
//! written against a zipper is a local rewrite of `focus` (and occasionally
//! of one frame), so the rule reads exactly like the judgment it
//! implements. An arena would make every rule a graph edit.
//!
//! Path convention: `path[0]` is the **outermost** frame (the root's) and
//! the last element is the immediate parent of the focus. Descending pushes;
//! ascending pops.

use nothing_core::ctx::Ctx;
use nothing_core::exp::{Exp, HoleId, Id, Op, Side};
use nothing_core::ty::Ty;
use nothing_core::typing::syn;

/// One step of the path from the root to the focus: the parent node with a
/// hole where the focused child was.
///
/// There is exactly one variant per child position of every `Exp` form that
/// has children. Each carries the parent's own metadata (operator, binder,
/// hole id, projection side) and its *sibling* subexpressions, which is
/// precisely what [`Frame::rebuild`] needs and nothing more.
///
/// Leaf forms (`Var`, `Num`, `Bool`, `EmptyHole`) have no children and so
/// contribute no frames.
#[derive(Clone, PartialEq, Debug)]
pub enum Frame {
    /// `λ<id>:<ty>. ◇`
    LamBody(Id, Ty),
    /// `◇ <arg>`
    ApFun(Exp),
    /// `<fun> ◇`
    ApArg(Exp),
    /// `◇ <op> <rhs>`
    BinOpLeft(Op, Exp),
    /// `<lhs> <op> ◇`
    BinOpRight(Op, Exp),
    /// `if ◇ then <then> else <else>`
    IfCond(Exp, Exp),
    /// `if <cond> then ◇ else <else>`
    IfThen(Exp, Exp),
    /// `if <cond> then <then> else ◇`
    IfElse(Exp, Exp),
    /// `let <id> = ◇ in <body>`
    LetBound(Id, Exp),
    /// `let <id> = <bound> in ◇`
    LetBody(Id, Exp),
    /// `(◇, <snd>)`
    PairFst(Exp),
    /// `(<fst>, ◇)`
    PairSnd(Exp),
    /// `proj_<side> ◇`
    ProjBody(Side),
    /// `⦇◇⦈`
    NonEmptyHoleBody(HoleId),
}

impl Frame {
    /// Put `focus` back into this frame's hole, reconstructing the parent.
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
            Frame::NonEmptyHoleBody(h) => Exp::NonEmptyHole(h, Box::new(focus)),
        }
    }

    /// Which child of its parent the focus is: the `n` that
    /// [`Zipper::move_child`] was called with to create this frame.
    pub fn child_index(&self) -> usize {
        match self {
            Frame::LamBody(..)
            | Frame::ApFun(..)
            | Frame::BinOpLeft(..)
            | Frame::IfCond(..)
            | Frame::LetBound(..)
            | Frame::PairFst(..)
            | Frame::ProjBody(..)
            | Frame::NonEmptyHoleBody(..) => 0,
            Frame::ApArg(..)
            | Frame::BinOpRight(..)
            | Frame::IfThen(..)
            | Frame::LetBody(..)
            | Frame::PairSnd(..) => 1,
            Frame::IfElse(..) => 2,
        }
    }

    /// How many children the parent node has — the bound for sibling
    /// movement.
    pub fn parent_arity(&self) -> usize {
        match self {
            Frame::LamBody(..) | Frame::ProjBody(..) | Frame::NonEmptyHoleBody(..) => 1,
            Frame::ApFun(..)
            | Frame::ApArg(..)
            | Frame::BinOpLeft(..)
            | Frame::BinOpRight(..)
            | Frame::LetBound(..)
            | Frame::LetBody(..)
            | Frame::PairFst(..)
            | Frame::PairSnd(..) => 2,
            Frame::IfCond(..) | Frame::IfThen(..) | Frame::IfElse(..) => 3,
        }
    }
}

/// How many children `exp` has. Leaves have none; this is the range
/// [`Zipper::move_child`] accepts.
pub fn arity(exp: &Exp) -> usize {
    match exp {
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::EmptyHole(_) => 0,
        Exp::Lam(..) | Exp::Proj(..) | Exp::NonEmptyHole(..) => 1,
        Exp::Ap(..) | Exp::BinOp(..) | Exp::Let(..) | Exp::Pair(..) => 2,
        Exp::If(..) => 3,
    }
}

/// A cursor into a program: the focused subexpression plus the path back to
/// the root.
///
/// The pair `(path, focus)` *is* the program — [`Zipper::to_exp`] recovers
/// it — so there is no separate "document" to keep in sync with the cursor.
#[derive(Clone, PartialEq, Debug)]
pub struct Zipper {
    /// The subexpression under the cursor.
    pub focus: Exp,
    /// Outermost frame first; the last element is the focus's parent.
    pub path: Vec<Frame>,
}

/// Place the cursor at the root of `exp`.
pub fn unzip(exp: Exp) -> Zipper {
    Zipper {
        focus: exp,
        path: Vec::new(),
    }
}

/// Rebuild the whole program from a cursor, discarding the cursor.
///
/// `zip(unzip(e)) == e` for every `e`, and more usefully `zip` composed with
/// any sequence of movements is still `e` — movement changes where you are,
/// never what the program is.
pub fn zip(z: Zipper) -> Exp {
    let Zipper { mut focus, path } = z;
    for frame in path.into_iter().rev() {
        focus = frame.rebuild(focus);
    }
    focus
}

impl Zipper {
    /// Place the cursor at the root of `exp`. Same as [`unzip`].
    pub fn new(exp: Exp) -> Zipper {
        unzip(exp)
    }

    /// The whole program, **without** consuming the cursor.
    pub fn to_exp(&self) -> Exp {
        zip(self.clone())
    }

    /// The whole program, consuming the cursor. Same as [`zip`].
    pub fn into_exp(self) -> Exp {
        zip(self)
    }

    /// Is the cursor at the root?
    pub fn is_root(&self) -> bool {
        self.path.is_empty()
    }

    /// How deep the cursor is below the root.
    pub fn depth(&self) -> usize {
        self.path.len()
    }

    /// Which child of its parent the focus is, or `None` at the root.
    pub fn child_index(&self) -> Option<usize> {
        self.path.last().map(Frame::child_index)
    }

    /// Replace the focused expression, leaving the cursor where it is.
    /// This is the one primitive every editing action is written in terms
    /// of.
    pub fn replace_focus(mut self, exp: Exp) -> Zipper {
        self.focus = exp;
        self
    }

    /// Descend into child `n` of the focus.
    ///
    /// Returns `None` — never panics — when the focus is a leaf or `n` is
    /// out of range. Child numbering follows the order the children appear
    /// in the form: `Ap` is `(fun, arg)`, `BinOp` is `(lhs, rhs)`, `If` is
    /// `(cond, then, else)`, `Let` is `(bound, body)`, `Pair` is
    /// `(fst, snd)`.
    pub fn move_child(self, n: usize) -> Option<Zipper> {
        // Check before destructuring so a failed move never has to
        // reassemble what it took apart.
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
            Exp::Proj(side, inner) => (Frame::ProjBody(side), *inner),
            Exp::NonEmptyHole(h, inner) => (Frame::NonEmptyHoleBody(h), *inner),
            // Unreachable: `arity` reported 0 for these, so the guard above
            // already returned.
            Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::EmptyHole(_) => return None,
        };
        path.push(frame);
        Some(Zipper { focus: child, path })
    }

    /// Ascend to the parent, refocusing on the reconstructed parent node.
    /// `None` at the root.
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

    /// Move to the sibling at `child_index + 1`, if there is one.
    pub fn move_next_sibling(self) -> Option<Zipper> {
        self.move_sibling(1)
    }

    /// Move to the sibling at `child_index - 1`, if there is one.
    pub fn move_prev_sibling(self) -> Option<Zipper> {
        self.move_sibling(-1)
    }

    /// Shared implementation: go up, then straight back down into a
    /// different child. Written this way rather than by shuffling frames in
    /// place because it is obviously program-preserving — every step is one
    /// of the two primitives.
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

    /// The typing context in force at the cursor: every binder whose scope
    /// the cursor is inside.
    ///
    /// This is the payoff of bidirectional typing that Phase 4's completion
    /// and Phase 10's hole-context query both read from — the editor
    /// already knows what is in scope at the cursor, because the path says
    /// which binders it descended through.
    ///
    /// A `let`'s binding type is the *synthesised* type of its bound
    /// expression. If that expression happens not to synthesise (only
    /// possible on a program that is not well-typed, which actions never
    /// produce), the binder is recorded at type `?` rather than dropped, so
    /// the result is always a total context.
    pub fn ctx(&self) -> Ctx {
        let mut ctx = Ctx::empty();
        for frame in &self.path {
            match frame {
                Frame::LamBody(id, ty) => ctx = ctx.extend(*id, ty.clone()),
                Frame::LetBody(id, bound) => {
                    let ty = syn(&ctx, bound).unwrap_or(Ty::Hole);
                    ctx = ctx.extend(*id, ty);
                }
                // A `let`'s bound expression is outside the scope of its
                // own binder, and no other frame binds anything.
                _ => {}
            }
        }
        ctx
    }
}

/// Every cursor position in `exp`, in pre-order (root first, then each
/// child's subtree left to right).
///
/// Used by the tests to quantify over "any cursor position" literally
/// rather than by sampling, and by the REPL harness to enumerate targets.
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

    // --- zip / unzip ---

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

    /// Every position is visited exactly once, so `all_positions` really is
    /// "every cursor position" and the Delete test below really does cover
    /// every subexpression.
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

    // --- movement primitives ---

    #[test]
    fn move_child_out_of_range_fails_cleanly() {
        let e = examples::add_with_empty_hole(); // 1 + ⦇⦈, arity 2
        let z = unzip(e);
        assert!(z.clone().move_child(0).is_some());
        assert!(z.clone().move_child(1).is_some());
        assert!(z.clone().move_child(2).is_none());
        assert!(z.clone().move_child(usize::MAX).is_none());

        // A leaf has no children at all.
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
        // if true then (1, 2) else (⦇⦈, 4) — three children.
        let e = examples::if_over_pairs_with_hole();
        let root = unzip(e);

        let cond = root.clone().move_child(0).unwrap();
        assert!(cond.clone().move_prev_sibling().is_none(), "no child before 0");

        let then = cond.move_next_sibling().unwrap();
        assert_eq!(then.child_index(), Some(1));
        let else_ = then.clone().move_next_sibling().unwrap();
        assert_eq!(else_.child_index(), Some(2));
        assert!(else_.clone().move_next_sibling().is_none(), "no child after 2");

        // ...and back the other way.
        assert_eq!(else_.move_prev_sibling().unwrap(), then);
    }

    #[test]
    fn sibling_movement_at_the_root_fails_cleanly() {
        let z = unzip(examples::let_identity());
        assert!(z.clone().move_next_sibling().is_none());
        assert!(z.move_prev_sibling().is_none());
    }

    // --- ctx ---

    #[test]
    fn ctx_at_a_lambda_body_sees_the_parameter() {
        // λn:Num. if n < 1 then 1 else n
        let z = unzip(examples::clamp_to_one()).move_child(0).unwrap();
        assert_eq!(z.ctx().lookup(&Id::new(0)), Some(Ty::Num));
    }

    #[test]
    fn ctx_at_a_let_body_sees_the_binding_but_the_bound_expression_does_not() {
        // let p = (1, true) in fst p
        let root = unzip(examples::pair_and_project());
        let p = Id::new(0);

        let bound = root.clone().move_child(0).unwrap();
        assert_eq!(bound.ctx().lookup(&p), None, "a let does not bind its own RHS");

        let body = root.move_child(1).unwrap();
        assert_eq!(
            body.ctx().lookup(&p),
            Some(Ty::Prod(Box::new(Ty::Num), Box::new(Ty::Bool)))
        );
    }

    #[test]
    fn ctx_at_the_root_is_empty() {
        assert_eq!(unzip(examples::let_identity()).ctx(), Ctx::empty());
    }

    // --- proptests ---

    /// Interpret a byte as a movement and apply it, ignoring moves that do
    /// not apply at the current position.
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
        /// The spec's literal criterion.
        #[test]
        fn zip_unzip_roundtrip(seed in any::<u64>()) {
            let e = generate::well_typed_exp(seed);
            prop_assert_eq!(zip(unzip(e.clone())), e);
        }

        /// Stronger than the literal criterion: after an arbitrary walk
        /// through the program, zipping still reproduces it exactly.
        #[test]
        fn zip_after_arbitrary_movement_reproduces_the_program(
            seed in any::<u64>(),
            moves in prop::collection::vec(any::<u8>(), 0..40),
        ) {
            let e = generate::well_typed_exp(seed);
            let z = wander(unzip(e.clone()), &moves);
            prop_assert_eq!(z.to_exp(), e);
        }

        /// Every position of an arbitrary program rebuilds the program.
        #[test]
        fn every_position_of_an_arbitrary_program_rebuilds_it(seed in any::<u64>()) {
            let e = generate::well_typed_exp(seed);
            for z in all_positions(&e) {
                prop_assert_eq!(z.to_exp(), e.clone());
            }
        }

        /// Ascending from any position eventually reaches the root, and the
        /// root's focus is the whole program.
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
