//! The sensibility proptest (Phase 2).
//!
//! > For any well-typed program and any cursor position and any action,
//! > either the action fails cleanly (returns `None`) or the resulting
//! > program is well-typed.
//!
//! This is the theorem the whole project rests on — it is what makes
//! "syntax error" and "type error as a broken state" stop existing as
//! categories — so it is stated here as literally as the type system
//! allows and quantified over as widely as the runtime budget allows:
//!
//! - **any well-typed program**: `generate::well_typed_exp` over an
//!   arbitrary `u64` seed, checked to be well-typed before the action is
//!   applied so a degenerate generator cannot make the property vacuous;
//! - **any cursor position**: `zipper::all_positions`, which is verified
//!   elsewhere to enumerate every node exactly once — sampled uniformly in
//!   the 10,000-case test and enumerated *exhaustively* in
//!   [`every_action_at_every_position_is_sensible`];
//! - **any action**: [`arb_action`], one branch per `Action` variant, with
//!   [`variant_name`] as a compile-time guard that no variant is silently
//!   left out of the strategy and [`the_strategy_covers_every_action_variant`]
//!   as a runtime guard that no branch is unreachable.
//!
//! The property is deliberately *not* weakened: it does not exclude any
//! action, any position, or any program, and it does not accept "the
//! program is different but plausible" — the post-condition is
//! `is_well_typed`, full stop. The only latitude the property grants is the
//! one the judgment itself grants: an action is allowed to not apply.

use nothing_action::act::{Action, apply};
use nothing_action::generate::{self, Gen};
use nothing_action::zipper::{Zipper, all_positions};
use nothing_core::exp::{Exp, Id, Op, Side};
use nothing_core::ty::Ty;
use nothing_core::typing::is_well_typed;
use proptest::prelude::*;
use proptest::strategy::{Union, ValueTree};
use proptest::test_runner::TestRunner;

// ---------------------------------------------------------------------------
// An arbitrary action
// ---------------------------------------------------------------------------

/// The name of an action's variant.
///
/// This exists to be exhaustive: no wildcard arm, so adding a variant to
/// [`Action`] fails to compile here, and whoever adds it is pointed at
/// [`arb_action`] and forced to decide how it is generated. A test that
/// silently stops covering a new action would be worse than no test.
fn variant_name(action: &Action) -> &'static str {
    match action {
        Action::MoveChild(_) => "MoveChild",
        Action::MoveParent => "MoveParent",
        Action::MoveNextSibling => "MoveNextSibling",
        Action::MovePrevSibling => "MovePrevSibling",
        Action::Delete => "Delete",
        Action::ConstructNum(_) => "ConstructNum",
        Action::ConstructBool(_) => "ConstructBool",
        Action::ConstructVar(_) => "ConstructVar",
        Action::ConstructLam => "ConstructLam",
        Action::ConstructAp => "ConstructAp",
        Action::ConstructBinOp(_) => "ConstructBinOp",
        Action::ConstructIf => "ConstructIf",
        Action::ConstructLet => "ConstructLet",
        Action::ConstructPair => "ConstructPair",
        Action::ConstructProj(_) => "ConstructProj",
        Action::ConstructNonEmptyHole => "ConstructNonEmptyHole",
        Action::SetAnn(_) => "SetAnn",
        Action::SetBinderId(_) => "SetBinderId",
        Action::Finish => "Finish",
    }
}

/// Every variant name, in declaration order. Kept next to [`variant_name`]
/// so the two are read together.
const ALL_VARIANTS: [&str; 19] = [
    "MoveChild",
    "MoveParent",
    "MoveNextSibling",
    "MovePrevSibling",
    "Delete",
    "ConstructNum",
    "ConstructBool",
    "ConstructVar",
    "ConstructLam",
    "ConstructAp",
    "ConstructBinOp",
    "ConstructIf",
    "ConstructLet",
    "ConstructPair",
    "ConstructProj",
    "ConstructNonEmptyHole",
    "SetAnn",
    "SetBinderId",
    "Finish",
];

fn arb_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        Just(Op::Add),
        Just(Op::Sub),
        Just(Op::Mul),
        Just(Op::Lt),
        Just(Op::Eq),
    ]
}

fn arb_side() -> impl Strategy<Value = Side> {
    prop_oneof![Just(Side::L), Just(Side::R)]
}

/// An arbitrary type, borrowed from the program generator rather than
/// rebuilt here — one type grammar, one place to extend.
fn arb_ty() -> impl Strategy<Value = Ty> {
    any::<u64>().prop_map(|seed| Gen::new(seed).ty(2))
}

/// Binder identities are drawn from a small range so that
/// `ConstructVar`/`SetBinderId` land on binders the generator actually
/// minted (its ids start at 0 and count up) often enough to exercise the
/// success paths, while still straying out of scope often enough to
/// exercise the clean-failure paths.
fn arb_id() -> impl Strategy<Value = Id> {
    (0u64..8).prop_map(Id::new)
}

/// An arbitrary action: one branch per variant of [`Action`].
///
/// `Union` rather than `prop_oneof!` only because the variant count is past
/// what the macro's tuple encoding handles; the meaning is the same, a
/// uniform choice among the branches.
pub fn arb_action() -> impl Strategy<Value = Action> {
    Union::new(vec![
        // Movement. The child index ranges past the maximum arity (3) so
        // out-of-range descent is generated too.
        (0usize..5).prop_map(Action::MoveChild).boxed(),
        Just(Action::MoveParent).boxed(),
        Just(Action::MoveNextSibling).boxed(),
        Just(Action::MovePrevSibling).boxed(),
        // Editing.
        Just(Action::Delete).boxed(),
        any::<i64>().prop_map(Action::ConstructNum).boxed(),
        any::<bool>().prop_map(Action::ConstructBool).boxed(),
        arb_id().prop_map(Action::ConstructVar).boxed(),
        Just(Action::ConstructLam).boxed(),
        Just(Action::ConstructAp).boxed(),
        arb_op().prop_map(Action::ConstructBinOp).boxed(),
        Just(Action::ConstructIf).boxed(),
        Just(Action::ConstructLet).boxed(),
        Just(Action::ConstructPair).boxed(),
        arb_side().prop_map(Action::ConstructProj).boxed(),
        Just(Action::ConstructNonEmptyHole).boxed(),
        arb_ty().prop_map(Action::SetAnn).boxed(),
        arb_id().prop_map(Action::SetBinderId).boxed(),
        Just(Action::Finish).boxed(),
    ])
}

/// One representative payload per variant, for the exhaustive test. Payload
/// choices are the ones most likely to *succeed* (an operator that fits, a
/// binder the generator mints, an annotation that is not `?`), because a
/// refused action proves nothing about the property.
fn one_of_every_action() -> Vec<Action> {
    let mut actions = vec![
        Action::MoveChild(0),
        Action::MoveChild(1),
        Action::MoveChild(2),
        Action::MoveChild(3),
        Action::MoveParent,
        Action::MoveNextSibling,
        Action::MovePrevSibling,
        Action::Delete,
        Action::ConstructNum(7),
        Action::ConstructBool(true),
        Action::ConstructVar(Id::new(0)),
        Action::ConstructVar(Id::new(1)),
        Action::ConstructLam,
        Action::ConstructAp,
        Action::ConstructBinOp(Op::Add),
        Action::ConstructBinOp(Op::Lt),
        Action::ConstructIf,
        Action::ConstructLet,
        Action::ConstructPair,
        Action::ConstructProj(Side::L),
        Action::ConstructProj(Side::R),
        Action::ConstructNonEmptyHole,
        Action::SetAnn(Ty::Num),
        Action::SetAnn(Ty::Hole),
        Action::SetAnn(Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Bool))),
        Action::SetBinderId(Id::new(0)),
        Action::SetBinderId(Id::new(9)),
        Action::Finish,
    ];
    actions.sort_by_key(|a| variant_name(a));
    actions
}

// ---------------------------------------------------------------------------
// Guards on the quantifiers themselves
// ---------------------------------------------------------------------------

/// Neither the strategy nor the exhaustive list may quietly stop covering a
/// variant. Without this, adding an action and forgetting to generate it
/// would leave the property passing while testing less.
#[test]
fn the_strategy_covers_every_action_variant() {
    let mut runner = TestRunner::deterministic();
    let strategy = arb_action();
    let mut seen: Vec<&'static str> = Vec::new();
    for _ in 0..4_000 {
        let action = strategy
            .new_tree(&mut runner)
            .expect("the action strategy is total")
            .current();
        let name = variant_name(&action);
        if !seen.contains(&name) {
            seen.push(name);
        }
    }

    for name in ALL_VARIANTS {
        assert!(
            seen.contains(&name),
            "arb_action never generated {name}: the sensibility property is not \
             quantifying over every action"
        );
    }
    assert_eq!(
        seen.len(),
        ALL_VARIANTS.len(),
        "arb_action generated a variant not listed in ALL_VARIANTS"
    );

    let exhaustive: Vec<&'static str> = one_of_every_action()
        .iter()
        .map(variant_name)
        .fold(Vec::new(), |mut acc, name| {
            if !acc.contains(&name) {
                acc.push(name);
            }
            acc
        });
    for name in ALL_VARIANTS {
        assert!(
            exhaustive.contains(&name),
            "one_of_every_action is missing {name}"
        );
    }
}

/// `all_positions` is the "any cursor position" quantifier; if it ever
/// collapsed to just the root the property would still pass while testing
/// almost nothing.
#[test]
fn the_position_quantifier_reaches_more_than_the_root() {
    let mut total = 0usize;
    let mut deepest = 0usize;
    for seed in 0..200u64 {
        let e = generate::well_typed_exp(seed);
        let positions = all_positions(&e);
        assert_eq!(positions.len(), generate::size(&e));
        total += positions.len();
        deepest = deepest.max(positions.iter().map(Zipper::depth).max().unwrap_or(0));
    }
    assert!(total > 600, "only {total} cursor positions over 200 programs");
    assert!(deepest >= 3, "the deepest cursor position was only {deepest}");
}

// ---------------------------------------------------------------------------
// The property
// ---------------------------------------------------------------------------

/// The judgment's two legal outcomes, checked. Returns whether the action
/// applied, so the callers can report how much of the search space actually
/// exercised the interesting branch.
fn check_sensible(z: Zipper, action: &Action) -> Result<bool, TestCaseError> {
    let before = z.to_exp();
    match apply(z, action.clone()) {
        // Outcome 1: the action does not apply. Nothing happened — the
        // caller still holds the program it had.
        None => Ok(false),
        // Outcome 2: the action applied, and what came back is a program in
        // the language. There is no third outcome.
        Some(after) => {
            let program = after.to_exp();
            prop_assert!(
                is_well_typed(&program),
                "{action:?} turned the well-typed program {before:?} into {program:?}"
            );
            Ok(true)
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 10_000,
        max_shrink_iters: 4_000,
        ..ProptestConfig::default()
    })]

    /// **The sensibility proptest.** 10,000 cases of
    /// (arbitrary well-typed program × arbitrary cursor position ×
    /// arbitrary action).
    #[test]
    fn any_action_at_any_cursor_position_of_any_well_typed_program_is_sensible(
        seed in any::<u64>(),
        position in any::<prop::sample::Index>(),
        action in arb_action(),
    ) {
        let program = generate::well_typed_exp(seed);
        // The premise of the judgment. If this ever fails the property is
        // vacuous, so it is asserted rather than assumed.
        prop_assert!(is_well_typed(&program), "the generator produced {program:?}");

        let positions = all_positions(&program);
        let cursor = positions[position.index(positions.len())].clone();
        check_sensible(cursor, &action)?;
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1_000,
        max_shrink_iters: 2_000,
        ..ProptestConfig::default()
    })]

    /// The same property with the two inner quantifiers made *exhaustive*
    /// rather than sampled: every cursor position of the program, crossed
    /// with one action of every variant. A thousand programs at an average
    /// of eight positions and twenty-eight actions is a quarter of a million
    /// more applications of the judgment, each of them checked.
    #[test]
    fn every_action_at_every_position_is_sensible(seed in any::<u64>()) {
        let program = generate::well_typed_exp(seed);
        prop_assert!(is_well_typed(&program));
        for cursor in all_positions(&program) {
            for action in one_of_every_action() {
                check_sensible(cursor.clone(), &action)?;
            }
        }
    }

    /// Sensibility is a one-step property, but an editing session is a
    /// sequence: this replays a long random run of arbitrary actions and
    /// checks the invariant after *every* step, so that no reachable state
    /// — not merely no state one step from the generator — can break it.
    #[test]
    fn a_long_random_session_never_leaves_the_language(
        seed in any::<u64>(),
        actions in prop::collection::vec(arb_action(), 1..80),
    ) {
        let program = generate::well_typed_exp(seed);
        let mut cursor = Zipper::new(program);
        for action in actions {
            if let Some(next) = apply(cursor.clone(), action.clone()) {
                let program = next.to_exp();
                prop_assert!(
                    is_well_typed(&program),
                    "{action:?} produced {program:?}"
                );
                cursor = next;
            }
            // On `None` the cursor is unchanged, by construction: `apply`
            // was handed a clone.
        }
    }
}

// ---------------------------------------------------------------------------
// The property is not vacuous
// ---------------------------------------------------------------------------

/// A property whose interesting branch never runs is not a property. This
/// measures how often each action actually *applies* over the same space
/// the proptests explore, and fails if any action never succeeds anywhere —
/// which would mean the sensibility test was passing on `None` alone.
#[test]
fn every_action_succeeds_somewhere_in_the_search_space() {
    let mut applied: Vec<(&'static str, usize)> = ALL_VARIANTS.iter().map(|n| (*n, 0)).collect();
    let mut total_applications = 0usize;

    for seed in 0..300u64 {
        let program = generate::well_typed_exp(seed);
        assert!(is_well_typed(&program));
        for cursor in all_positions(&program) {
            for action in one_of_every_action() {
                total_applications += 1;
                let name = variant_name(&action);
                if let Some(after) = apply(cursor.clone(), action.clone()) {
                    assert!(
                        is_well_typed(&after.to_exp()),
                        "{action:?} produced {:?}",
                        after.to_exp()
                    );
                    let slot = applied
                        .iter_mut()
                        .find(|(n, _)| *n == name)
                        .expect("every variant is in ALL_VARIANTS");
                    slot.1 += 1;
                }
            }
        }
    }

    assert!(
        total_applications > 50_000,
        "only {total_applications} judgments were exercised"
    );
    for (name, count) in &applied {
        assert!(
            *count > 0,
            "{name} never applied anywhere in {total_applications} attempts — the \
             sensibility property is passing vacuously for it"
        );
    }
}

/// The other half of non-vacuity: the actions that *can* fail must be seen
/// to fail cleanly, leaving the caller's program untouched rather than
/// panicking or returning something damaged.
#[test]
fn actions_that_do_not_apply_leave_the_program_untouched() {
    let mut refusals = 0usize;
    for seed in 0..200u64 {
        let program = generate::well_typed_exp(seed);
        for cursor in all_positions(&program) {
            for action in one_of_every_action() {
                let before = cursor.to_exp();
                if apply(cursor.clone(), action.clone()).is_none() {
                    refusals += 1;
                    // The cursor was cloned into `apply`, so the caller's
                    // program is trivially unchanged — assert it anyway,
                    // because "fails cleanly" is half the property.
                    assert_eq!(cursor.to_exp(), before);
                }
            }
        }
    }
    assert!(refusals > 0, "no action ever failed: the None branch is untested");
}

/// A last guard against the property being weakened by accident: a
/// deliberately broken "action" must be caught by exactly the check the
/// proptests run. If this ever stops failing, `check_sensible` has stopped
/// checking anything.
#[test]
fn the_check_would_catch_an_unsound_action() {
    // `1 + true` — the program an action that skipped quarantine would
    // produce.
    let unsound = Exp::bin_op(Op::Add, Exp::num(1), Exp::bool_(true));
    assert!(
        !is_well_typed(&unsound),
        "this program must be ill-typed for the guard to mean anything"
    );
    // ...and the real action for the same edit does not produce it.
    let cursor = Zipper::new(Exp::bin_op(
        Op::Add,
        Exp::num(1),
        Exp::empty_hole(nothing_core::exp::HoleId::new(0)),
    ))
    .move_child(1)
    .expect("the right operand exists");
    let after = apply(cursor, Action::ConstructBool(true)).expect("never told no");
    assert_ne!(after.to_exp(), unsound);
    assert!(is_well_typed(&after.to_exp()));
}
