//! The reachability proptest (Phase 2).
//!
//! > For any two well-typed programs `a` and `b`, there exists a finite
//! > action sequence taking `a` to `b`.
//!
//! Proved *constructively*: [`path_to`] computes the sequence, and the
//! property replays it. Sensibility (see `sensibility.rs`) says the action
//! calculus never lets you out of the language; reachability says it never
//! walls you into a corner of it. Together they say the calculus is a
//! complete replacement for typing text, not a restricted subset of it —
//! which is the claim that has to hold before any of Phase 4's ergonomics
//! are worth measuring.
//!
//! # How the sequence is built
//!
//! Three stages, exactly as the spec describes:
//!
//! 1. **Move to root** — one `MoveParent` per level of cursor depth.
//! 2. **Delete to a single empty hole** — one `Delete` at the root. `a` is
//!    gone; nothing about it can influence what follows, which is why the
//!    path's length depends only on `b`.
//! 3. **Construct `b` downward** — one construction per node of `b`, in
//!    pre-order, with `MoveNextSibling`/`MoveParent` to walk between
//!    positions.
//!
//! Stage 3 relies on one uniform property of the construction rules: on an
//! **empty hole**, every non-leaf construction produces its form with all
//! children empty holes and leaves the cursor on child 0. So building a
//! subtree is always "construct the node, walk its children left to right
//! building each, then `MoveParent`", and the recursion returns with the
//! cursor on the node it built.
//!
//! Two things `b` can contain that plain construction cannot reproduce, and
//! what is done about each:
//!
//! - **Binder identities and lambda annotations.** Construction mints a
//!   fresh `Id` and annotates `?`, so `λx₄:Num. ◇` is not reachable by
//!   construction alone. `SetBinderId`/`SetAnn` (see [`nothing_action::act`])
//!   supply exactly the missing edits, which is why reachability here is a
//!   literal equality rather than an equality up to renaming.
//! - **Non-empty holes whose contents fit.** `1 + ⦇2⦈` is well-typed — a
//!   non-empty hole synthesises `?` — but *automatic* quarantine only fires
//!   on expressions that do **not** fit, so no sequence of the original
//!   actions produces it. `ConstructNonEmptyHole` closes that gap. Without
//!   it, this property would be false rather than merely untested.
//!
//! # What "exactly `b`" means
//!
//! Structural equality, with one explicit exception: **hole identities**.
//! A `HoleId` is minted by the editor's fresh supply and is deliberately
//! *not* part of what the program says — it is the handle the action log,
//! the renderer, and (Phase 6) an indeterminate evaluation result use to
//! point at a particular hole. Demanding that a replay reproduce another
//! program's hole ids would be demanding that two independent editing
//! sessions agree on serial numbers. So the general property compares
//! programs after canonically renumbering their holes in pre-order (which
//! rewrites *nothing* else — every operator, literal, binder id and
//! annotation must still match exactly), and
//! [`hole_free_targets_are_reached_exactly`] additionally pins literal
//! `==` on the targets where the question does not arise.

use nothing_action::act::{Action, EditState};
use nothing_action::generate;
use nothing_action::zipper::{Zipper, all_positions};
use nothing_core::exp::{Exp, HoleId, Id, Op, Side};
use nothing_core::ty::Ty;
use nothing_core::typing::is_well_typed;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// The construction
// ---------------------------------------------------------------------------

/// A finite action sequence taking the program `a` — cursor anywhere in it —
/// to the program `b`.
///
/// `a` is read only for its shape; the sequence deletes it wholesale. The
/// interesting content is [`build`].
pub fn path_to(a: &Exp, b: &Exp) -> Vec<Action> {
    path_to_from(&Zipper::new(a.clone()), b)
}

/// [`path_to`] from an arbitrary cursor position rather than the root.
pub fn path_to_from(cursor: &Zipper, b: &Exp) -> Vec<Action> {
    let mut actions = vec![Action::MoveParent; cursor.depth()];
    actions.push(Action::Delete);
    build(b, &mut actions);
    actions
}

/// Emit the actions that turn the empty hole under the cursor into
/// `target`, leaving the cursor on the expression that was built.
///
/// Pre-condition: the cursor is on an `EmptyHole`.
/// Post-condition: the focus is `target` (up to hole identity).
fn build(target: &Exp, actions: &mut Vec<Action>) {
    match target {
        // Already there: the cursor is sitting on an empty hole, which is
        // what an empty hole is. Only its serial number differs.
        Exp::EmptyHole(_) => {}

        // Leaves replace the focus and the cursor stays on them.
        Exp::Num(n) => actions.push(Action::ConstructNum(*n)),
        Exp::Bool(b) => actions.push(Action::ConstructBool(*b)),
        Exp::Var(id) => actions.push(Action::ConstructVar(*id)),

        // A lambda needs its binder and annotation written before its body
        // is built: the body is constructed *against* them — `ConstructVar`
        // must find the parameter in scope, and every expected type inside
        // the body flows from the annotation.
        Exp::Lam(id, ann, body) => {
            actions.push(Action::ConstructLam);
            actions.push(Action::MoveParent);
            actions.push(Action::SetBinderId(*id));
            actions.push(Action::SetAnn(ann.clone()));
            actions.push(Action::MoveChild(0));
            build(body, actions);
            actions.push(Action::MoveParent);
        }

        // Likewise a `let`: the binder must exist before the body that
        // mentions it.
        Exp::Let(id, bound, body) => {
            actions.push(Action::ConstructLet);
            actions.push(Action::MoveParent);
            actions.push(Action::SetBinderId(*id));
            actions.push(Action::MoveChild(0));
            build_children(&[bound, body], actions);
        }

        Exp::Ap(fun, arg) => {
            actions.push(Action::ConstructAp);
            build_children(&[fun, arg], actions);
        }
        Exp::BinOp(op, lhs, rhs) => {
            actions.push(Action::ConstructBinOp(*op));
            build_children(&[lhs, rhs], actions);
        }
        Exp::If(cond, then, else_) => {
            actions.push(Action::ConstructIf);
            build_children(&[cond, then, else_], actions);
        }
        Exp::Pair(fst, snd) => {
            actions.push(Action::ConstructPair);
            build_children(&[fst, snd], actions);
        }
        Exp::Proj(side, inner) => {
            actions.push(Action::ConstructProj(*side));
            build_children(&[inner], actions);
        }

        // Explicitly, never by relying on automatic quarantine: the target
        // may well be a non-empty hole whose contents fit, which automatic
        // quarantine would refuse to produce.
        Exp::NonEmptyHole(_, inner) => {
            actions.push(Action::ConstructNonEmptyHole);
            build_children(&[inner], actions);
        }
    }
}

/// Build each child of a just-constructed form, then return the cursor to
/// the form itself.
///
/// Pre-condition: the cursor is on child 0 of the new form, and every child
/// is an empty hole — which is what every construction leaves behind when
/// it is applied to an empty hole.
fn build_children(children: &[&Exp], actions: &mut Vec<Action>) {
    for (i, child) in children.iter().enumerate() {
        if i > 0 {
            actions.push(Action::MoveNextSibling);
        }
        build(child, actions);
    }
    actions.push(Action::MoveParent);
}

// ---------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------

/// Replay a path, checking that every action applies and that the program
/// is well-typed at every intermediate step. Returns the final program.
fn replay(start: Exp, actions: &[Action]) -> Result<Exp, String> {
    let mut state = EditState::new(start);
    for (i, action) in actions.iter().enumerate() {
        if !state.apply_mut(action.clone()) {
            return Err(format!(
                "action {i} of {} ({action:?}) did not apply to {:?}",
                actions.len(),
                state.exp()
            ));
        }
        if !is_well_typed(&state.exp()) {
            return Err(format!(
                "action {i} ({action:?}) left the ill-typed program {:?}",
                state.exp()
            ));
        }
    }
    Ok(state.exp())
}

// ---------------------------------------------------------------------------
// Equality up to hole identity
// ---------------------------------------------------------------------------

/// Renumber every hole in pre-order, rewriting nothing else.
fn canonical_hole_ids(exp: &Exp) -> Exp {
    fn go(exp: &Exp, next: &mut u64) -> Exp {
        let mut fresh = || {
            let h = HoleId::new(*next);
            *next += 1;
            h
        };
        match exp {
            Exp::Var(id) => Exp::Var(*id),
            Exp::Num(n) => Exp::Num(*n),
            Exp::Bool(b) => Exp::Bool(*b),
            Exp::EmptyHole(_) => Exp::EmptyHole(fresh()),
            Exp::NonEmptyHole(_, inner) => {
                let h = fresh();
                Exp::non_empty_hole(h, go(inner, next))
            }
            Exp::Lam(id, ann, body) => Exp::lam(*id, ann.clone(), go(body, next)),
            Exp::Ap(f, a) => Exp::ap(go(f, next), go(a, next)),
            Exp::BinOp(op, l, r) => Exp::bin_op(*op, go(l, next), go(r, next)),
            Exp::If(c, t, e) => Exp::if_(go(c, next), go(t, next), go(e, next)),
            Exp::Let(id, bound, body) => Exp::let_(*id, go(bound, next), go(body, next)),
            Exp::Pair(l, r) => Exp::pair(go(l, next), go(r, next)),
            Exp::Proj(side, inner) => Exp::proj(*side, go(inner, next)),
        }
    }
    go(exp, &mut 0)
}

/// Equal as programs: identical in every respect except the serial numbers
/// on holes, which are the editor's bookkeeping and not part of what the
/// program says.
fn eq_up_to_hole_ids(x: &Exp, y: &Exp) -> bool {
    canonical_hole_ids(x) == canonical_hole_ids(y)
}

fn is_hole_free(exp: &Exp) -> bool {
    match exp {
        Exp::EmptyHole(_) | Exp::NonEmptyHole(..) => false,
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) => true,
        Exp::Lam(_, _, b) | Exp::Proj(_, b) => is_hole_free(b),
        Exp::Ap(a, b) | Exp::BinOp(_, a, b) | Exp::Let(_, a, b) | Exp::Pair(a, b) => {
            is_hole_free(a) && is_hole_free(b)
        }
        Exp::If(c, t, e) => is_hole_free(c) && is_hole_free(t) && is_hole_free(e),
    }
}

// ---------------------------------------------------------------------------
// The property
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1_000,
        max_shrink_iters: 2_000,
        ..ProptestConfig::default()
    })]

    /// **The reachability proptest.** 1,000 random pairs of well-typed
    /// programs; for each, the computed path takes `a` to `b`.
    #[test]
    fn any_well_typed_program_reaches_any_other(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
    ) {
        let a = generate::well_typed_exp(seed_a);
        let b = generate::well_typed_exp(seed_b);
        prop_assert!(is_well_typed(&a));
        prop_assert!(is_well_typed(&b));

        let actions = path_to(&a, &b);
        let reached = replay(a.clone(), &actions).map_err(TestCaseError::fail)?;

        prop_assert!(
            eq_up_to_hole_ids(&reached, &b),
            "path of {} actions from {a:?} reached {reached:?}, not {b:?}",
            actions.len()
        );
        // The path is finite and proportional to the target, never to the
        // program being replaced: `a` is deleted, not edited into shape.
        prop_assert!(actions.len() <= 8 * generate::size(&b) + 1);
    }

    /// The cursor does not have to start at the root: the path's first
    /// stage walks it there, whichever of the program's positions it was in.
    #[test]
    fn reachability_holds_from_every_starting_cursor_position(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
    ) {
        let a = generate::well_typed_exp(seed_a);
        let b = generate::well_typed_exp(seed_b);
        for cursor in all_positions(&a) {
            let actions = path_to_from(&cursor, &b);
            let mut state = EditState::new(a.clone());
            state.zipper = cursor.clone();
            for (i, action) in actions.iter().enumerate() {
                prop_assert!(
                    state.apply_mut(action.clone()),
                    "action {i} ({action:?}) did not apply from depth {}",
                    cursor.depth()
                );
                prop_assert!(is_well_typed(&state.exp()));
            }
            prop_assert!(eq_up_to_hole_ids(&state.exp(), &b));
        }
    }

    /// Reachability is not a special property of the *generated* pairs: any
    /// program is reachable from the empty program, which is where a real
    /// editing session starts.
    #[test]
    fn every_program_is_reachable_from_nothing(seed in any::<u64>()) {
        let b = generate::well_typed_exp(seed);
        let empty = EditState::empty().exp();
        let reached = replay(empty, &path_to(&Exp::EmptyHole(HoleId::new(0)), &b))
            .map_err(TestCaseError::fail)?;
        prop_assert!(eq_up_to_hole_ids(&reached, &b));
    }

    /// Reachability composes: a → b → a gets back to exactly where it
    /// started, so no program is a one-way door.
    #[test]
    fn a_round_trip_returns_to_the_original_program(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
    ) {
        let a = generate::well_typed_exp(seed_a);
        let b = generate::well_typed_exp(seed_b);
        let there = replay(a.clone(), &path_to(&a, &b)).map_err(TestCaseError::fail)?;
        let back = replay(there.clone(), &path_to(&there, &a)).map_err(TestCaseError::fail)?;
        prop_assert!(eq_up_to_hole_ids(&back, &a));
    }
}

/// Hole identity is the *only* latitude the general property takes. On
/// targets containing no holes at all, the replay reproduces the target
/// under plain `==`: every binder id, every annotation, every literal,
/// every operator.
#[test]
fn hole_free_targets_are_reached_exactly() {
    let mut checked = 0usize;
    let mut biggest = 0usize;
    for seed in 0..4_000u64 {
        let b = generate::well_typed_exp(seed);
        if !is_hole_free(&b) || generate::size(&b) < 2 {
            continue;
        }
        let a = generate::well_typed_exp(seed.wrapping_mul(2_654_435_761).wrapping_add(7));
        let reached = replay(a.clone(), &path_to(&a, &b)).expect("the path applies");
        assert_eq!(reached, b, "from {a:?}");
        checked += 1;
        biggest = biggest.max(generate::size(&b));
    }
    assert!(
        checked >= 100,
        "only {checked} hole-free targets were found; the exact-equality claim is thin"
    );
    assert!(
        biggest >= 5,
        "the biggest hole-free target had only {biggest} nodes"
    );
}

/// The targets the property runs on must actually contain the hard cases,
/// or reachability is only proved for the easy fragment. This asserts the
/// generator's output — the same output the proptest consumes — contains
/// every syntactic form, both kinds of hole, concrete lambda annotations,
/// and non-empty holes whose contents *do* fit (the case that needs
/// `ConstructNonEmptyHole`).
#[test]
fn the_targets_cover_the_hard_cases() {
    fn survey(e: &Exp, seen: &mut Vec<&'static str>) {
        let note = |what: &'static str, seen: &mut Vec<&'static str>| {
            if !seen.contains(&what) {
                seen.push(what);
            }
        };
        match e {
            Exp::Var(_) => note("Var", seen),
            Exp::Num(_) => note("Num", seen),
            Exp::Bool(_) => note("Bool", seen),
            Exp::EmptyHole(_) => note("EmptyHole", seen),
            Exp::NonEmptyHole(_, inner) => {
                note("NonEmptyHole", seen);
                survey(inner, seen);
            }
            Exp::Lam(_, ann, body) => {
                note("Lam", seen);
                if *ann != Ty::Hole {
                    note("AnnotatedLam", seen);
                }
                survey(body, seen);
            }
            Exp::Ap(f, a) => {
                note("Ap", seen);
                survey(f, seen);
                survey(a, seen);
            }
            Exp::BinOp(_, l, r) => {
                note("BinOp", seen);
                survey(l, seen);
                survey(r, seen);
            }
            Exp::If(c, t, el) => {
                note("If", seen);
                survey(c, seen);
                survey(t, seen);
                survey(el, seen);
            }
            Exp::Let(_, b, body) => {
                note("Let", seen);
                survey(b, seen);
                survey(body, seen);
            }
            Exp::Pair(l, r) => {
                note("Pair", seen);
                survey(l, seen);
                survey(r, seen);
            }
            Exp::Proj(_, inner) => {
                note("Proj", seen);
                survey(inner, seen);
            }
        }
    }

    let mut seen = Vec::new();
    for seed in 0..2_000u64 {
        survey(&generate::well_typed_exp(seed), &mut seen);
    }
    for form in [
        "Var",
        "Num",
        "Bool",
        "EmptyHole",
        "NonEmptyHole",
        "Lam",
        "AnnotatedLam",
        "Ap",
        "BinOp",
        "If",
        "Let",
        "Pair",
        "Proj",
    ] {
        assert!(
            seen.contains(&form),
            "no target ever contained a {form}: reachability is untested for it"
        );
    }
}

// ---------------------------------------------------------------------------
// The two gaps `path_to` had to close, pinned individually
// ---------------------------------------------------------------------------

/// `SetAnn` and `SetBinderId`, exercised on the smallest program that needs
/// them: construction alone can only produce `λ<fresh>:?. ◇`.
#[test]
fn a_specific_annotated_binder_is_reachable() {
    let x = Id::new(42);
    let target = Exp::lam(x, Ty::Num, Exp::bin_op(Op::Add, Exp::var(x), Exp::num(1)));
    assert!(is_well_typed(&target));

    let start = Exp::bool_(true);
    let reached = replay(start.clone(), &path_to(&start, &target)).expect("the path applies");
    assert_eq!(reached, target);
}

/// `SetAnn` refuses rather than corrupting: annotating `λx:?. x + 1` as
/// `Bool` would leave the body untypable, so the action does not apply and
/// the program is untouched.
#[test]
fn set_ann_fails_cleanly_when_the_annotation_would_break_the_body() {
    let x = Id::new(0);
    let program = Exp::lam(x, Ty::Hole, Exp::bin_op(Op::Add, Exp::var(x), Exp::num(1)));
    let state = EditState::new(program.clone());

    assert!(state.apply(Action::SetAnn(Ty::Bool)).is_none());
    let ok = state.apply(Action::SetAnn(Ty::Num)).expect("Num fits");
    assert_eq!(
        ok.exp(),
        Exp::lam(x, Ty::Num, Exp::bin_op(Op::Add, Exp::var(x), Exp::num(1)))
    );
    // The refusal left the original alone.
    assert_eq!(state.exp(), program);
}

/// `SetAnn` and `SetBinderId` apply only to the forms that have the thing
/// they set, and re-identifying a binder that is referenced is refused —
/// it would orphan the reference.
#[test]
fn binder_metadata_actions_do_not_apply_elsewhere() {
    let x = Id::new(0);

    // Not a lambda: no annotation to write.
    let state = EditState::new(Exp::num(1));
    assert!(state.apply(Action::SetAnn(Ty::Num)).is_none());
    assert!(state.apply(Action::SetBinderId(x)).is_none());

    // A lambda has no `let` binding and vice versa, but both have binders.
    let lam = EditState::new(Exp::lam(x, Ty::Num, Exp::num(1)));
    assert_eq!(
        lam.apply(Action::SetBinderId(Id::new(9)))
            .expect("the body does not mention the parameter")
            .exp(),
        Exp::lam(Id::new(9), Ty::Num, Exp::num(1))
    );

    // ...but re-identifying a binder that *is* referenced would orphan the
    // reference, so it is refused.
    let used = EditState::new(Exp::lam(x, Ty::Num, Exp::var(x)));
    assert!(used.apply(Action::SetBinderId(Id::new(9))).is_none());

    let let_ = EditState::new(Exp::let_(x, Exp::num(1), Exp::num(2)));
    assert_eq!(
        let_.apply(Action::SetBinderId(Id::new(9)))
            .expect("the body does not mention the binding")
            .exp(),
        Exp::let_(Id::new(9), Exp::num(1), Exp::num(2))
    );
}

/// `ConstructNonEmptyHole` builds the quarantine the automatic rule will
/// not: `1 + ⦇2⦈` is well-typed, and no *automatic* quarantine produces it,
/// because `2` fits perfectly well where it stands.
#[test]
fn a_non_empty_hole_whose_contents_fit_is_reachable_only_explicitly() {
    let target = Exp::bin_op(
        Op::Add,
        Exp::num(1),
        Exp::non_empty_hole(HoleId::new(0), Exp::num(2)),
    );
    assert!(is_well_typed(&target), "a fitting quarantine is legal");

    // Automatic quarantine does not fire here: writing `2` into the right
    // operand simply writes `2`.
    let plain = EditState::new(Exp::bin_op(
        Op::Add,
        Exp::num(1),
        Exp::EmptyHole(HoleId::new(0)),
    ))
    .apply(Action::MoveChild(1))
    .unwrap()
    .apply(Action::ConstructNum(2))
    .unwrap();
    assert_eq!(plain.exp(), Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2)));

    // The explicit action does.
    let start = Exp::num(0);
    let reached = replay(start.clone(), &path_to(&start, &target)).expect("the path applies");
    assert!(eq_up_to_hole_ids(&reached, &target), "{reached:?}");
    match reached {
        Exp::BinOp(Op::Add, _, rhs) => match *rhs {
            Exp::NonEmptyHole(_, inner) => assert_eq!(*inner, Exp::num(2)),
            other => panic!("expected a quarantined 2, got {other:?}"),
        },
        other => panic!("expected an addition, got {other:?}"),
    }
}

/// `ConstructNonEmptyHole` is the inverse of `Finish`, and the pair
/// round-trips.
#[test]
fn construct_non_empty_hole_and_finish_are_inverse() {
    let program = Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2));
    let mut state = EditState::new(program.clone());
    assert!(state.apply_mut(Action::MoveChild(1)));
    assert!(state.apply_mut(Action::ConstructNonEmptyHole));
    match state.zipper.to_exp() {
        Exp::BinOp(Op::Add, _, rhs) => assert!(matches!(*rhs, Exp::NonEmptyHole(..))),
        other => panic!("expected a quarantined operand, got {other:?}"),
    }
    assert!(is_well_typed(&state.exp()));

    // The cursor landed inside the new hole (its contents are not an empty
    // hole, so it rests on the hole itself); `Finish` unwraps it again.
    if !matches!(state.zipper.focus, Exp::NonEmptyHole(..)) {
        assert!(state.apply_mut(Action::MoveParent));
    }
    assert!(state.apply_mut(Action::Finish));
    assert_eq!(state.exp(), program);
}

/// The examples are hand-written rather than generated, and include the
/// shapes the generator is least likely to hit; every one of them is
/// reachable from every other.
#[test]
fn every_example_program_reaches_every_other_example() {
    use nothing_core::examples;
    let examples = [
        examples::let_identity(),
        examples::increment_applied(),
        examples::clamp_to_one(),
        examples::pair_and_project(),
        examples::pair_with_empty_hole(),
        examples::add_with_empty_hole(),
        examples::square_and_compare(),
        examples::identity_hole_annotated_applied(),
        examples::add_with_non_empty_hole(),
        examples::if_over_pairs_with_hole(),
    ];
    for a in &examples {
        for b in &examples {
            let reached = replay(a.clone(), &path_to(a, b)).expect("the path applies");
            assert!(
                eq_up_to_hole_ids(&reached, b),
                "reached {reached:?} instead of {b:?}"
            );
        }
    }
}

/// A worked example, spelled out: the path from `1 + 2` to `fst (3, true)`
/// is short, every action in it applies, and the result is the target.
#[test]
fn a_worked_path_is_short_and_exact() {
    let a = Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2));
    let b = Exp::proj(Side::L, Exp::pair(Exp::num(3), Exp::bool_(true)));
    let actions = path_to(&a, &b);
    assert_eq!(
        actions,
        vec![
            Action::Delete,
            Action::ConstructProj(Side::L),
            Action::ConstructPair,
            Action::ConstructNum(3),
            Action::MoveNextSibling,
            Action::ConstructBool(true),
            Action::MoveParent,
            Action::MoveParent,
        ]
    );
    assert_eq!(replay(a, &actions).expect("the path applies"), b);
}
