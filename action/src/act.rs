//! The action grammar and the action judgment (Phase 2).
//!
//! Following Hazelnut (Omar et al., POPL 2017), an edit is the judgment
//!
//! ```text
//! (cursor, program) --action--> (cursor', program')
//! ```
//!
//! and the theorem the whole design rests on is that this judgment carries
//! well-typedness left to right. Operationally that means [`apply`] has
//! exactly two outcomes: `None`, meaning the action does not apply here and
//! nothing happened, or `Some(z)`, where `z.to_exp()` is well-typed. There
//! is no third outcome where the program is left damaged, so there is no
//! error-recovery path to write and no "invalid program" state to render.
//!
//! # The construction rules, in one paragraph
//!
//! Every construction is the same shape. The *leaf* constructions
//! ([`Action::ConstructNum`], [`Action::ConstructBool`],
//! [`Action::ConstructVar`]) replace the focus outright. Every other
//! construction *wraps* the focus into the new form's **principal
//! position** — always child 0: a lambda's body, an application's function,
//! a binary operator's left operand, a conditional's scrutinee, a `let`'s
//! bound expression, a pair's first component, a projection's operand — and
//! fills the remaining children with fresh empty holes. Nothing is ever
//! discarded, which is what makes `1 + 2` three keystrokes rather than a
//! detour through a scratch buffer.
//!
//! The cursor then lands on **the first empty hole among the new form's
//! immediate children**, in source order, or stays on the new expression
//! when there is no such child. That single rule produces the two
//! behaviours you actually want: from an empty hole, `Construct If` leaves
//! the cursor on the scrutinee (the hole that was already there), while
//! from a written-out `n < 1` it leaves the cursor on the fresh `then`
//! hole. Only the immediate children are considered, so wrapping an
//! expression that itself contains holes does not teleport the cursor
//! backwards into it.
//!
//! # Never saying no
//!
//! A construction that would make the program type-inconsistent is not
//! rejected. The offending subexpression is quarantined in a fresh
//! *non-empty hole*, which synthesises `?` and therefore fits anywhere, and
//! the edit goes through. There are two places that can happen, and both
//! do:
//!
//! - the wrapped focus does not fit its new position inside the form —
//!   `Construct +` on `true` gives `⦇true⦈ + ⦇⦈`, not a refusal;
//! - the finished form does not fit the position it is being written into
//!   — `Construct true` at a `Num`-expecting hole gives `⦇true⦈`.
//!
//! [`Action::Finish`] is the inverse: when a quarantined expression has
//! been edited until it fits again, `Finish` unwraps it.
//!
//! # Extending this module
//!
//! `apply_with` is deliberately the only function that matches on
//! [`Action`], so a new variant produces one compile error in one place.
//! New actions should take fresh `HoleId`s and `Id`s from the [`Fresh`]
//! supply that [`apply_with`] already threads, and should read the expected
//! type and in-scope bindings at the cursor from
//! [`ctx_and_expected_ty_at`] rather than re-deriving them.

use nothing_core::ctx::Ctx;
use nothing_core::exp::{Exp, HoleId, Id, Op, Side};
use nothing_core::ty::{Ty, matched_arrow, matched_prod};
use nothing_core::typing::{ana, is_well_typed, syn};

use crate::zipper::{Frame, Zipper, unzip};

/// An edit action.
///
/// The movement actions relocate the cursor and provably do not touch the
/// program. The editing actions rewrite the program at the cursor and
/// provably preserve well-typedness.
#[derive(Clone, PartialEq, Debug)]
pub enum Action {
    // --- Movement (never changes the program) ---
    /// Descend into child `n` of the focus. Children are numbered in
    /// source order: `Ap(fun, arg)`, `BinOp(lhs, rhs)`,
    /// `If(cond, then, else)`, `Let(bound, body)`, `Pair(fst, snd)`,
    /// `Lam(body)`, `Proj(body)`, `NonEmptyHole(body)`.
    MoveChild(usize),
    /// Ascend to the parent node.
    MoveParent,
    /// Move to the next child of the same parent.
    MoveNextSibling,
    /// Move to the previous child of the same parent.
    MovePrevSibling,

    // --- Editing ---
    /// Replace the focused expression with a fresh empty hole.
    ///
    /// Delete never removes a node without leaving a gap. The gap is not a
    /// courtesy to the renderer, it is the mechanism: an empty hole
    /// synthesises `?`, `?` is consistent with every type, and so the
    /// context the deleted expression sat in still typechecks. Removing the
    /// node outright would have no well-typed meaning for `1 + ⌷` at all.
    Delete,

    // --- Construction: leaves (replace the focus) ---
    /// Write a numeric literal at the cursor.
    ConstructNum(i64),
    /// Write a boolean literal at the cursor.
    ConstructBool(bool),
    /// Write a reference to an in-scope binder at the cursor.
    ///
    /// The one construction that can legitimately fail: a variable that is
    /// not in scope has no meaning to quarantine, so this returns `None`
    /// rather than inventing a binding. In the editor the candidate list is
    /// drawn from [`ctx_at`], so the failing case is unreachable from the
    /// keyboard.
    ConstructVar(Id),

    // --- Construction: forms (wrap the focus into child 0) ---
    /// `e` becomes `λx:?. e`.
    ConstructLam,
    /// `e` becomes `e ⦇⦈` — the wrapping rule. Never discards `e`.
    ConstructAp,
    /// `e` becomes `e <op> ⦇⦈` — the wrapping rule. This is what makes
    /// typing `1 + 2` feel like typing text: three actions, no backtracking.
    ConstructBinOp(Op),
    /// `e` becomes `if e then ⦇⦈ else ⦇⦈`.
    ConstructIf,
    /// `e` becomes `let x = e in ⦇⦈`.
    ConstructLet,
    /// `e` becomes `(e, ⦇⦈)`.
    ConstructPair,
    /// `e` becomes `fst e` / `snd e`.
    ConstructProj(Side),
    /// `e` becomes `⦇e⦈` — quarantine the focus deliberately.
    ///
    /// The explicit counterpart of the automatic quarantine performed by
    /// construction, and the exact inverse of [`Action::Finish`]. It exists
    /// because automatic quarantine only fires when an expression does *not*
    /// fit: without this action a program containing a non-empty hole whose
    /// contents happen to fit (`1 + ⦇2⦈` — well-typed, since a non-empty
    /// hole synthesises `?`) would be unreachable by any action sequence,
    /// and the Phase 2 reachability claim would simply be false. See
    /// `action/tests/reachability.rs`.
    ConstructNonEmptyHole,

    /// Write the type annotation on the focused lambda.
    ///
    /// Construction mints lambdas unannotated (`λx:?. ◇`), so without this
    /// there is no action that produces `λx:Num. ◇` at all — half the
    /// programs the type grammar admits would be unreachable. Fails cleanly
    /// off a lambda, and when the new annotation would break the program
    /// (annotating `λx:?. x + 1` as `Bool` leaves the body untypable): the
    /// program is left untouched, exactly as if the action did not apply.
    SetAnn(Ty),

    /// Re-identify the focused binder — a lambda's parameter or a `let`'s
    /// binding.
    ///
    /// Identity, not display name: this is what lets a *specific* binder
    /// identity be constructed rather than whatever the fresh supply
    /// happened to mint, which is what makes reachability an equality
    /// rather than an equality-up-to-renaming. Fails cleanly off a binder,
    /// and whenever the change would capture or orphan a reference — the
    /// well-typedness check catches both.
    ///
    /// Phase 5 replaces `Id` with an opaque UUID and makes *renaming* a
    /// name-table write; this action is about the AST's identity, and is a
    /// different thing from that rename.
    SetBinderId(Id),

    /// Unwrap a non-empty hole whose contents now fit their context.
    ///
    /// The dual of automatic quarantine: construction puts an expression
    /// into a non-empty hole when it does not fit, `Finish` takes it back
    /// out once it does. Fails cleanly when the cursor is not on a
    /// non-empty hole, or when the contents still do not fit — in which
    /// case the program is unchanged and still well-typed.
    Finish,
}

/// A supply of fresh [`HoleId`]s and [`Id`]s.
///
/// Fresh ids matter because a hole's identity is what the editor, the
/// action log, and (Phase 6) the indeterminate results of evaluation use to
/// refer to "that specific hole" across edits. Two distinct holes sharing
/// an id would silently merge in all three.
///
/// Two ways to get one:
///
/// - [`Fresh::from_program`] scans a program for the largest id in use and
///   starts one past it. Correct with no bookkeeping, `O(n)` per call —
///   this is what the standalone [`apply`] uses.
/// - [`EditState`] carries a `Fresh` across a whole editing session, so the
///   scan happens once and every subsequent action is `O(1)`. This is what
///   the editor and the REPL harness should use.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Fresh {
    next_hole: u64,
    next_id: u64,
}

impl Fresh {
    /// A supply starting from zero. Only safe on a program known to
    /// contain no ids, e.g. a brand-new empty hole; prefer
    /// [`Fresh::from_program`].
    pub fn new() -> Fresh {
        Fresh::default()
    }

    /// A supply guaranteed not to collide with anything already in `exp`.
    pub fn from_program(exp: &Exp) -> Fresh {
        let mut f = Fresh::default();
        f.observe(exp);
        f
    }

    /// Advance the supply past every id occurring in `exp`.
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
        self.next_id = self.next_id.max(id.0 + 1);
    }

    fn bump_hole(&mut self, h: HoleId) {
        self.next_hole = self.next_hole.max(h.0 + 1);
    }

    /// Take a fresh hole identity.
    pub fn hole(&mut self) -> HoleId {
        let h = HoleId::new(self.next_hole);
        self.next_hole += 1;
        h
    }

    /// Take a fresh binder identity. Unused by the actions implemented so
    /// far; `ConstructLam` and `ConstructLet` will want it.
    pub fn id(&mut self) -> Id {
        let id = Id::new(self.next_id);
        self.next_id += 1;
        id
    }
}

/// The typing context in force at the cursor: every binder whose scope the
/// cursor is inside, with its type.
///
/// The free-function form of [`Zipper::ctx`]; it exists because it is half
/// of the pair the editor actually asks for (the other half is
/// [`expected_ty_at`]), and because Phase 4's completion and Phase 10's
/// hole-context query both name it directly.
pub fn ctx_at(zipper: &Zipper) -> Ctx {
    zipper.ctx()
}

/// The type expected at the cursor: what an expression written here would
/// be analysed against.
///
/// This is the payoff of bidirectional typing, and the reason the whole
/// project uses it rather than Hindley–Milner. It is computed *top-down*,
/// pushing the root's expectation (`?`, since a program may be anything)
/// through each frame of the path:
///
/// - a lambda's body inherits the output side of the matched arrow;
/// - an application's function is expected to be `? -> τ` where `τ` is what
///   the application itself is expected to produce; its argument is
///   expected to be the input side of the function's synthesised type;
/// - a binary operator's operands are `Num`, a conditional's scrutinee is
///   `Bool`;
/// - a conditional's branches inherit the conditional's expectation, and
///   when that is `?` (the conditional is in synthesis position) the
///   *other* branch's synthesised type stands in, because the two branches
///   must join;
/// - a `let`'s bound expression is unconstrained (`?`) and its body
///   inherits the `let`'s expectation;
/// - a pair's components come from the matched product;
/// - `fst ◇` expected at `τ` expects `τ * ?` of its operand;
/// - anything inside a hole is unconstrained, which is exactly what a hole
///   is for.
///
/// Unknown is always `?` rather than a failure, so this is total: there is
/// no cursor position at which the editor cannot say what it wants.
pub fn expected_ty_at(zipper: &Zipper) -> Ty {
    ctx_and_expected_ty_at(zipper).1
}

/// [`ctx_at`] and [`expected_ty_at`] in one walk of the path.
///
/// The two are computed together because the expected type genuinely needs
/// the context — an application's argument type comes from *synthesising*
/// the function, which requires the binders in scope there.
pub fn ctx_and_expected_ty_at(zipper: &Zipper) -> (Ctx, Ty) {
    let mut ctx = Ctx::empty();
    // A whole program may have any type at all.
    let mut expected = Ty::Hole;

    for frame in &zipper.path {
        match frame {
            Frame::LamBody(id, ann) => {
                let (_, out) = matched_arrow(&expected).unwrap_or((Ty::Hole, Ty::Hole));
                // The *annotation* enters the context, not the matched
                // input type: that is what `ana` does for `Lam`, and the
                // two need only be consistent.
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
            // A `let` synthesises its bound expression, so nothing is
            // expected of it, and does not bind over it.
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
            // The contents of a hole are unconstrained — that is the whole
            // point of putting them there.
            Frame::NonEmptyHoleBody(_) => expected = Ty::Hole,
        }
    }

    (ctx, expected)
}

/// The action judgment, with an explicit supply of fresh names.
///
/// This is the single place [`Action`] is matched on. Every new action gets
/// an arm here.
///
/// `fresh` may be advanced even when the action fails; ids are cheap and
/// skipping a few is harmless, whereas re-using one is not.
pub fn apply_with(zipper: Zipper, action: Action, fresh: &mut Fresh) -> Option<Zipper> {
    match action {
        Action::MoveChild(n) => zipper.move_child(n),
        Action::MoveParent => zipper.move_parent(),
        Action::MoveNextSibling => zipper.move_next_sibling(),
        Action::MovePrevSibling => zipper.move_prev_sibling(),

        // del: (ê, τ) --Delete--> (⦇⦈, ?)
        //
        // The cursor stays on the new hole, which is what makes
        // delete-then-retype a two-action edit rather than three.
        Action::Delete => {
            let hole = fresh.hole();
            Some(zipper.replace_focus(Exp::empty_hole(hole)))
        }

        // --- Construction: leaves ---
        Action::ConstructNum(n) => construct_leaf(zipper, Exp::num(n), fresh),
        Action::ConstructBool(b) => construct_leaf(zipper, Exp::bool_(b), fresh),
        Action::ConstructVar(id) => {
            if ctx_at(&zipper).lookup(&id).is_none() {
                None
            } else {
                construct_leaf(zipper, Exp::var(id), fresh)
            }
        }

        // --- Construction: forms. The second argument of each is the type
        // the focus must have once it lands in the form's principal
        // position; failing it is what triggers quarantine of the focus.
        Action::ConstructLam => construct_wrapping(zipper, Ty::Hole, fresh, |body, fresh| {
            Exp::lam(fresh.id(), Ty::Hole, body)
        }),
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
        Action::ConstructLet => construct_wrapping(zipper, Ty::Hole, fresh, |bound, fresh| {
            Exp::let_(fresh.id(), bound, Exp::empty_hole(fresh.hole()))
        }),
        Action::ConstructPair => construct_wrapping(zipper, Ty::Hole, fresh, |fst, fresh| {
            Exp::pair(fst, Exp::empty_hole(fresh.hole()))
        }),
        Action::ConstructProj(side) => construct_wrapping(
            zipper,
            Ty::Prod(Box::new(Ty::Hole), Box::new(Ty::Hole)),
            fresh,
            |body, _| Exp::proj(side, body),
        ),
        // Nothing is expected of a hole's contents, so the focus is never
        // quarantined *inside* the hole it is being put into.
        Action::ConstructNonEmptyHole => {
            construct_wrapping(zipper, Ty::Hole, fresh, |inner, fresh| {
                Exp::non_empty_hole(fresh.hole(), inner)
            })
        }

        // --- Binder metadata (rewrite in place; the cursor does not move) ---
        Action::SetAnn(ann) => set_ann(zipper, ann),
        Action::SetBinderId(id) => set_binder_id(zipper, id),

        // fin: (⦇e⦈, τ) --Finish--> (e, τ)  when e now fits
        Action::Finish => finish(zipper),
    }
}

/// Which immediate child of `exp` the cursor should land on after
/// constructing it: the first empty hole in source order, if any.
///
/// Deliberately only one level deep. A construction that wraps an
/// expression already containing holes must not drag the cursor backwards
/// into that expression — after `Construct +` on `f ⦇⦈` the cursor belongs
/// in the fresh right operand, not back in `f`'s argument.
fn first_empty_hole_child(exp: &Exp) -> Option<usize> {
    fn first(children: &[&Exp]) -> Option<usize> {
        children
            .iter()
            .position(|c| matches!(c, Exp::EmptyHole(_)))
    }
    match exp {
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::EmptyHole(_) => None,
        Exp::Lam(_, _, b) | Exp::Proj(_, b) | Exp::NonEmptyHole(_, b) => first(&[b]),
        Exp::Ap(a, b) | Exp::BinOp(_, a, b) | Exp::Let(_, a, b) | Exp::Pair(a, b) => first(&[a, b]),
        Exp::If(c, t, e) => first(&[c, t, e]),
    }
}

/// Write `leaf` (a `Num`, `Bool` or `Var`) over whatever is focused.
fn construct_leaf(zipper: Zipper, leaf: Exp, fresh: &mut Fresh) -> Option<Zipper> {
    let (ctx, expected) = ctx_and_expected_ty_at(&zipper);
    place(zipper, leaf, &ctx, &expected, fresh)
}

/// Wrap the focus into a new form's principal position.
///
/// `inner_expected` is what that position requires of the focus. The focus
/// must also *synthesise* a type: every form here has a synthesis rule that
/// reads its principal child, so a child with no synthesised type would
/// leave the form with none either, and the parent needs one. When either
/// check fails the focus is quarantined in a non-empty hole — which
/// synthesises `?` and is consistent with everything — rather than the
/// action being refused.
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

/// Put `form` at the cursor, quarantining it in a non-empty hole if it does
/// not fit there, and move the cursor to the form's first empty child.
///
/// The fit test is two-part. The first part is the local, bidirectional
/// one the editor can explain to a user: does `form` analyse against the
/// type expected here? The second is a whole-program check, because the
/// local test is *necessary but not sufficient* at positions where the
/// parent synthesises from the child — writing `1` into the `then` branch
/// of `if c then ◇ else true` passes locally (a branch in synthesis
/// position expects `?`) yet leaves the conditional with no join. Rather
/// than complicate the expected-type judgment until it can see every such
/// case, the invariant is enforced where it is stated: a returned zipper
/// always zips to a well-typed program.
fn place(
    zipper: Zipper,
    form: Exp,
    ctx: &Ctx,
    expected: &Ty,
    fresh: &mut Fresh,
) -> Option<Zipper> {
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
        // Unreachable for a well-typed program and any construction: a
        // non-empty hole synthesises `?`. Kept as a clean failure rather
        // than an `expect`, because "the action did not apply" is always a
        // legal outcome and a damaged program never is.
        None
    }
}

/// Move the cursor onto the newly constructed form's `target` child,
/// stepping through the quarantine wrapper first when there is one.
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

/// Accept a rewritten program only if it is still well-typed.
///
/// The in-place rewrites below have nothing to quarantine — an annotation
/// and a binder identity are not expressions, so there is no subexpression
/// to wrap in a non-empty hole — which leaves "the action does not apply"
/// as the only honest outcome when the rewrite would break the program.
/// That is still one of the two legal outcomes of the judgment: the caller
/// gets `None` and an untouched program.
fn keep_if_well_typed(zipper: Zipper) -> Option<Zipper> {
    if is_well_typed(&zipper.to_exp()) {
        Some(zipper)
    } else {
        None
    }
}

/// Write a new annotation onto the focused lambda.
fn set_ann(zipper: Zipper, ann: Ty) -> Option<Zipper> {
    let updated = match &zipper.focus {
        Exp::Lam(id, _, body) => Exp::Lam(*id, ann, body.clone()),
        _ => return None,
    };
    keep_if_well_typed(zipper.replace_focus(updated))
}

/// Re-identify the focused binder.
fn set_binder_id(zipper: Zipper, id: Id) -> Option<Zipper> {
    let updated = match &zipper.focus {
        Exp::Lam(_, ann, body) => Exp::Lam(id, ann.clone(), body.clone()),
        Exp::Let(_, bound, body) => Exp::Let(id, bound.clone(), body.clone()),
        _ => return None,
    };
    keep_if_well_typed(zipper.replace_focus(updated))
}

/// Unwrap a non-empty hole whose contents now fit.
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

/// The action judgment, in the shape the spec states it:
/// `(cursor, program) --action--> (cursor', program')`.
///
/// Fresh names are drawn from a supply seeded by scanning the current
/// program, so a standalone call is always self-consistent. Editing
/// sessions should use [`EditState`] instead, which threads one supply
/// across all actions and avoids the scan.
pub fn apply(zipper: Zipper, action: Action) -> Option<Zipper> {
    let mut fresh = Fresh::from_program(&zipper.to_exp());
    apply_with(zipper, action, &mut fresh)
}

/// A program being edited: the cursor plus the fresh-name supply.
///
/// This is the thing an editor session, the REPL harness, and (Phase 2,
/// later) the action log all hold. It exists so that fresh ids are `O(1)`
/// and monotonic across a session rather than rescanned per action.
#[derive(Clone, PartialEq, Debug)]
pub struct EditState {
    pub zipper: Zipper,
    pub fresh: Fresh,
}

impl EditState {
    /// Start editing `exp` with the cursor at its root.
    pub fn new(exp: Exp) -> EditState {
        let fresh = Fresh::from_program(&exp);
        EditState {
            zipper: unzip(exp),
            fresh,
        }
    }

    /// Start editing the empty program, `⦇⦈`.
    pub fn empty() -> EditState {
        let mut fresh = Fresh::new();
        let hole = fresh.hole();
        EditState {
            zipper: unzip(Exp::empty_hole(hole)),
            fresh,
        }
    }

    /// The whole program under edit.
    pub fn exp(&self) -> Exp {
        self.zipper.to_exp()
    }

    /// Apply an action, returning the new state. `None` means the action
    /// did not apply; `self` is untouched either way.
    pub fn apply(&self, action: Action) -> Option<EditState> {
        let mut fresh = self.fresh.clone();
        let zipper = apply_with(self.zipper.clone(), action, &mut fresh)?;
        Some(EditState { zipper, fresh })
    }

    /// Apply an action in place, reporting whether it applied. The state is
    /// left untouched when it did not.
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

    // --- movement ---

    #[test]
    fn movement_actions_relocate_the_cursor() {
        // (λx:Num. x + 1) 41
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
        let z = unzip(examples::add_with_empty_hole()); // 1 + ⦇⦈
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

    // --- Delete ---

    /// The spec's criterion, taken literally: *every* subexpression of
    /// *every* example program, enumerated rather than sampled.
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
        // A guard against the enumeration silently collapsing to nothing.
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
        // 1 + ⦇⦈₀ : deleting the `1` must not reuse hole id 0.
        let z = unzip(examples::add_with_empty_hole())
            .move_child(0)
            .unwrap();
        let after = apply(z, Action::Delete).unwrap();
        match after.focus {
            Exp::EmptyHole(h) => assert_ne!(h, HoleId::new(0)),
            other => panic!("expected an empty hole, got {other:?}"),
        }
    }

    #[test]
    fn repeated_deletes_in_a_session_never_reuse_a_hole_id() {
        // Delete each child of a pair in turn; the two holes must differ.
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
        // 1 + ⦇true⦈ — delete the `true`, leaving ⦇⦈ inside the non-empty
        // hole. Still well-typed: the inner empty hole synthesises `?`.
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
                assert!(matches!(*rhs, Exp::NonEmptyHole(_, ref inner) if matches!(**inner, Exp::EmptyHole(_))));
            }
            other => panic!("expected `1 + ⦇⦈⦈`, got {other:?}"),
        }
    }

    #[test]
    fn delete_weakens_a_binding_to_hole_without_breaking_its_uses() {
        // let p = (1, true) in fst p — delete the pair. `p : ?` now, and
        // `fst p` still typechecks via matched_prod(?) = (?, ?).
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

    // --- ctx_at / expected_ty_at ---

    fn arrow(a: Ty, b: Ty) -> Ty {
        Ty::Arrow(Box::new(a), Box::new(b))
    }

    fn prod(a: Ty, b: Ty) -> Ty {
        Ty::Prod(Box::new(a), Box::new(b))
    }

    fn hole(n: u64) -> Exp {
        Exp::empty_hole(HoleId::new(n))
    }

    #[test]
    fn expected_ty_at_the_root_is_unconstrained() {
        assert_eq!(expected_ty_at(&unzip(examples::let_identity())), Ty::Hole);
    }

    #[test]
    fn expected_ty_at_a_binop_operand_is_num() {
        // 1 + ⦇⦈
        let root = unzip(examples::add_with_empty_hole());
        assert_eq!(expected_ty_at(&root.clone().move_child(0).unwrap()), Ty::Num);
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
        // if true then ⦇⦈ else 2 — the `then` branch is in synthesis
        // position, so its expectation is whatever the `else` branch
        // synthesised: the two must join.
        let e = Exp::if_(Exp::bool_(true), hole(0), Exp::num(2));
        let root = unzip(e);
        assert_eq!(expected_ty_at(&root.clone().move_child(1).unwrap()), Ty::Num);
        assert_eq!(expected_ty_at(&root.move_child(2).unwrap()), Ty::Hole);
    }

    #[test]
    fn expected_ty_at_an_application_argument_is_the_functions_input() {
        // (λx:Num. x) ⦇⦈
        let x = Id::new(0);
        let e = Exp::ap(Exp::lam(x, Ty::Num, Exp::var(x)), hole(0));
        let root = unzip(e);
        assert_eq!(expected_ty_at(&root.clone().move_child(1).unwrap()), Ty::Num);
        // ...and the function position wants an arrow, which is what stops
        // `Construct 5` there from being accepted silently.
        assert_eq!(
            expected_ty_at(&root.move_child(0).unwrap()),
            arrow(Ty::Hole, Ty::Hole)
        );
    }

    #[test]
    fn expected_ty_is_pushed_through_an_application_into_a_lambda_body() {
        // (λf:(Num -> Bool). f) (λx:?. ⦇⦈)
        //                                 ^ expected here: Bool
        let f = Id::new(0);
        let x = Id::new(1);
        let e = Exp::ap(
            Exp::lam(f, arrow(Ty::Num, Ty::Bool), Exp::var(f)),
            Exp::lam(x, Ty::Hole, hole(0)),
        );
        assert!(is_well_typed(&e));
        let body = unzip(e)
            .move_child(1)
            .unwrap() // the argument lambda
            .move_child(0)
            .unwrap(); // its body
        assert_eq!(expected_ty_at(&body), Ty::Bool);
        // ...and the binder is in scope there, at its annotation.
        assert_eq!(ctx_at(&body).lookup(&x), Some(Ty::Hole));
    }

    #[test]
    fn expected_ty_is_pushed_into_a_pair_component() {
        // (λp:Num * Bool. p) (⦇⦈, ⦇⦈)
        let p = Id::new(0);
        let e = Exp::ap(
            Exp::lam(p, prod(Ty::Num, Ty::Bool), Exp::var(p)),
            Exp::pair(hole(0), hole(1)),
        );
        let arg = unzip(e).move_child(1).unwrap();
        assert_eq!(
            expected_ty_at(&arg.clone().move_child(0).unwrap()),
            Ty::Num
        );
        assert_eq!(expected_ty_at(&arg.move_child(1).unwrap()), Ty::Bool);
    }

    #[test]
    fn expected_ty_under_a_projection_is_a_product() {
        // fst ⦇⦈ : the operand must be a product, with the projection's own
        // expectation on the selected side.
        let z = unzip(Exp::proj(Side::L, hole(0))).move_child(0).unwrap();
        assert_eq!(expected_ty_at(&z), prod(Ty::Hole, Ty::Hole));

        // 1 + snd ⦇⦈ : the projection is expected to be Num, so its
        // operand is expected to be ? * Num.
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
        // 1 + ⦇true⦈ — inside the hole, anything goes. That is what lets a
        // quarantined expression be edited freely until it fits again.
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

    /// The context half of [`ctx_and_expected_ty_at`] is a second
    /// implementation of the walk `Zipper::ctx` does; this pins them
    /// together at every cursor position of every example.
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

    // --- Construction: leaves ---

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
        // λx:Num. ⦇⦈ with the cursor in the body.
        let x = Id::new(0);
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
        assert!(apply(z, Action::ConstructVar(Id::new(3))).is_none());
    }

    // --- Construction: forms, on an empty hole ---

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
            Exp::Ap(fun, arg) => {
                // Two *distinct* holes: the original one became the
                // function, and the argument is fresh.
                match (*fun, *arg) {
                    (Exp::EmptyHole(a), Exp::EmptyHole(b)) => assert_ne!(a, b),
                    other => panic!("expected two empty holes, got {other:?}"),
                }
            }
            other => panic!("expected an application, got {other:?}"),
        }
        // The first empty child in source order is the function.
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
        // let x = ⦇⦈ in ⦇⦈, then a lambda in the body.
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

    // --- The wrapping rule ---

    /// The spec's named test. Constructing an operator on a written-out
    /// expression must keep it as the left operand rather than discarding
    /// it, and must leave the cursor in the fresh right operand.
    #[test]
    fn construct_binop_wraps_focus() {
        let z = unzip(Exp::num(1));
        let after = apply(z, Action::ConstructBinOp(Op::Add)).unwrap();

        // Shape: 1 + ⦇⦈, with the 1 preserved.
        match after.to_exp() {
            Exp::BinOp(Op::Add, l, r) => {
                assert_eq!(*l, Exp::num(1), "the focus must not be discarded");
                assert!(matches!(*r, Exp::EmptyHole(_)));
            }
            other => panic!("expected `1 + ⦇⦈`, got {other:?}"),
        }
        // Cursor: in the new right-hand hole.
        assert!(matches!(after.focus, Exp::EmptyHole(_)));
        assert_eq!(after.child_index(), Some(1));
        assert!(is_well_typed(&after.to_exp()));
    }

    #[test]
    fn construct_ap_wraps_focus() {
        // λx:Num. x  becomes  (λx:Num. x) ⦇⦈
        let x = Id::new(0);
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
        // `⦇⦈ ⦇⦈` already contains holes; `Construct +` must put the cursor
        // in the *new* right operand, not back inside the application.
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
        // n < 1  becomes  if n < 1 then ⦇⦈ else ⦇⦈, cursor on `then`.
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
        // 1 becomes λx:?. 1 — the form introduced no hole, so the cursor
        // rests on the new lambda rather than jumping anywhere.
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
        // let p = (1, true) in p  →  ... fst p, cursor still on `p`.
        let p = Id::new(0);
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

    /// The spec's other criterion for the wrapping rule: writing `1 + 2`
    /// from nothing costs exactly three actions, with no movement and no
    /// deletion.
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

    // --- Automatic non-empty-hole insertion ---

    /// The spec's criterion, verbatim: constructing `1 + true` succeeds,
    /// the program is well-typed, and the `true` is inside a non-empty hole.
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
        // The cursor rests on the quarantining hole, so `Finish` is one
        // keystroke away once the contents are fixed.
        assert!(matches!(state.zipper.focus, Exp::NonEmptyHole(..)));
    }

    #[test]
    fn construct_binop_on_a_bool_focus_quarantines_the_bool() {
        // The same guard, applied to *wrapping* rather than to writing into
        // a hole: `true` cannot be an operand of `+`, so it is quarantined
        // instead of the action being refused.
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
        // Still the wrapping rule: the cursor is in the new right operand.
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
        // A conditional written into a `Num`-expecting hole does not fit,
        // so the whole form is quarantined — but the cursor still lands on
        // the scrutinee, through the quarantining hole.
        let z = unzip(examples::add_with_empty_hole()) // 1 + ⦇⦈
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
        // The guard must not fire needlessly: a Num in a Num position is
        // written plainly.
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
        // `if true then ◇ else true` is in synthesis position, so both
        // branches must join. Writing a Num into the `then` branch is
        // locally plausible and globally impossible, and is quarantined.
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

    // --- Finish ---

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

    /// The spec's criterion: a program with a non-empty hole, edited so its
    /// contents fit, can be finished, and the result is well-typed with no
    /// hole left in it.
    #[test]
    fn a_non_empty_hole_edited_until_it_fits_can_be_finished() {
        // 1 + ⦇true⦈
        let mut state = EditState::new(examples::add_with_non_empty_hole());
        assert!(contains_a_hole(&state.exp()));

        // Move into the hole and replace its contents with something that
        // does fit the `Num` position the hole is standing in.
        assert!(state.apply_mut(Action::MoveChild(1)));
        assert!(state.apply_mut(Action::MoveChild(0)));
        assert!(state.apply_mut(Action::Delete));
        assert!(state.apply_mut(Action::ConstructNum(2)));
        assert!(state.apply_mut(Action::MoveParent));
        assert!(matches!(state.zipper.focus, Exp::NonEmptyHole(..)));

        // Now it fits, so it can be finished.
        assert!(state.apply_mut(Action::Finish));

        let program = state.exp();
        assert_eq!(program, Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2)));
        assert!(is_well_typed(&program));
        assert!(!contains_a_hole(&program), "no hole left: {program:?}");
        assert_eq!(state.zipper.focus, Exp::num(2), "cursor on the contents");
    }

    #[test]
    fn finish_refuses_while_the_contents_still_do_not_fit() {
        // 1 + ⦇true⦈ — the `true` is still a Bool in a Num position.
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
        // Construct `1 + true`, fix the operand, finish: a complete
        // round-trip through the quarantine mechanism.
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

    // --- EditState ---

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
    fn fresh_from_program_clears_every_id_in_use() {
        let e = Exp::let_(
            Id::new(7),
            Exp::empty_hole(HoleId::new(12)),
            Exp::non_empty_hole(HoleId::new(3), Exp::var(Id::new(7))),
        );
        let mut fresh = Fresh::from_program(&e);
        assert_eq!(fresh.hole(), HoleId::new(13));
        assert_eq!(fresh.id(), Id::new(8));
    }

    // --- proptests ---

    /// Every construction action, with representative payloads.
    fn every_construction() -> Vec<Action> {
        vec![
            Action::ConstructNum(1),
            Action::ConstructBool(true),
            Action::ConstructVar(Id::new(0)),
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

    /// Does `needle` occur anywhere in `haystack`?
    fn contains_subexpression(haystack: &Exp, needle: &Exp) -> bool {
        if haystack == needle {
            return true;
        }
        (0..arity(haystack)).any(|i| {
            let child = unzip(haystack.clone())
                .move_child(i)
                .expect("i < arity");
            contains_subexpression(&child.focus, needle)
        })
    }

    /// Turn a byte into a movement action.
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
        /// The spec's criterion for movement: it never changes the
        /// underlying program, only the focus.
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
                    // The program is identical after every single step,
                    // not merely at the end of the walk.
                    prop_assert_eq!(next.to_exp(), e.clone());
                    z = next;
                }
            }
            prop_assert_eq!(z.to_exp(), e);
        }

        /// Movement out of range fails cleanly at every position, for every
        /// movement action, rather than panicking or silently clamping.
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

        /// Delete preserves well-typedness at every position of an
        /// arbitrary well-typed program, not just of the examples.
        #[test]
        fn delete_preserves_well_typedness_everywhere(seed in any::<u64>()) {
            let e = generate::well_typed_exp(seed);
            prop_assert!(is_well_typed(&e));
            for z in all_positions(&e) {
                let after = apply(z, Action::Delete).expect("Delete always applies");
                prop_assert!(is_well_typed(&after.to_exp()), "{:?}", after.to_exp());
            }
        }

        /// A whole session of interleaved movement and deletion stays
        /// well-typed after every action — the Phase 2 sensibility property
        /// restricted to the actions implemented so far.
        #[test]
        fn interleaved_movement_and_deletion_stays_well_typed(
            seed in any::<u64>(),
            steps in prop::collection::vec(any::<u8>(), 0..40),
        ) {
            let e = generate::well_typed_exp(seed);
            let mut state = EditState::new(e);
            for byte in steps {
                // One in seven steps deletes; the rest wander.
                let action = if byte % 7 == 0 { Action::Delete } else { movement(byte) };
                state.apply_mut(action);
                prop_assert!(is_well_typed(&state.exp()), "{:?}", state.exp());
            }
        }

        /// The invariant this phase exists to establish, for every
        /// construction action at every cursor position: the action either
        /// fails cleanly or leaves a well-typed program. Nothing is ever
        /// refused merely for being ill-typed — that is what quarantine is
        /// for — so every construction here in fact succeeds.
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

        /// Constructions never discard what was under the cursor: after any
        /// wrapping construction the focused expression is still somewhere
        /// in the program.
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

        /// A whole editing session of interleaved movement, deletion,
        /// construction and finishing stays well-typed after every step.
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

        /// Deleting the same position twice is idempotent up to the hole's
        /// identity: the second delete replaces a hole with a hole.
        #[test]
        fn delete_leaves_the_cursor_on_a_fresh_hole(seed in any::<u64>()) {
            let e = generate::well_typed_exp(seed);
            for z in all_positions(&e) {
                let after = apply(z, Action::Delete).unwrap();
                prop_assert!(matches!(after.focus, Exp::EmptyHole(_)));
            }
        }
    }

    /// Delete does not merely produce *a* hole, it produces something that
    /// synthesises `?` — the property every enclosing rule relies on.
    #[test]
    fn hole_type_is_what_delete_leaves_behind() {
        let after = apply(unzip(Exp::num(1)), Action::Delete).unwrap();
        assert_eq!(syn(&Ctx::empty(), &after.to_exp()), Some(Ty::Hole));
    }
}
