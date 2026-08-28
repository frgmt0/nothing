
use nothing_action::act::{Action, apply};
use nothing_action::generate::{self, Gen};
use nothing_action::zipper::{Zipper, all_positions};
use nothing_core::exp::{Exp, Id, Op, Side};
use nothing_core::ty::Ty;
use nothing_core::typing::is_well_typed;
use proptest::prelude::*;
use proptest::strategy::{Union, ValueTree};
use proptest::test_runner::TestRunner;


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
        Action::Rename(..) => "Rename",
        Action::Finish => "Finish",
    }
}

const ALL_VARIANTS: [&str; 20] = [
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
    "Rename",
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

fn arb_ty() -> impl Strategy<Value = Ty> {
    any::<u64>().prop_map(|seed| Gen::new(seed).ty(2))
}

fn arb_id() -> impl Strategy<Value = Id> {
    (0u128..8).prop_map(Id::from_u128)
}

fn arb_name() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("x".to_string()),
        Just("xs".to_string()),
        Just("items".to_string()),
        Just(String::new()),
    ]
}

pub fn arb_action() -> impl Strategy<Value = Action> {
    Union::new(vec![


        (0usize..5).prop_map(Action::MoveChild).boxed(),
        Just(Action::MoveParent).boxed(),
        Just(Action::MoveNextSibling).boxed(),
        Just(Action::MovePrevSibling).boxed(),

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
        (arb_id(), arb_name())
            .prop_map(|(id, name)| Action::Rename(id, name))
            .boxed(),
        Just(Action::Finish).boxed(),
    ])
}

fn one_of_every_action() -> Vec<Action> {
    one_of_every_action_in(&[])
}


fn one_of_every_action_in(scope: &[Id]) -> Vec<Action> {
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
        Action::ConstructVar(Id::from_u128(0)),
        Action::ConstructVar(Id::from_u128(1)),
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
        Action::SetBinderId(Id::from_u128(0)),
        Action::SetBinderId(Id::from_u128(9)),
        Action::Rename(Id::from_u128(0), "x".to_string()),
        Action::Rename(Id::from_u128(9), "items".to_string()),
        Action::Finish,
    ];
    actions.extend(scope.iter().copied().map(Action::ConstructVar));
    actions.sort_by_key(|a| variant_name(a));
    actions
}


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


fn check_sensible(z: Zipper, action: &Action) -> Result<bool, TestCaseError> {
    let before = z.to_exp();
    match apply(z, action.clone()) {


        None => Ok(false),


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

    #[test]
    fn any_action_at_any_cursor_position_of_any_well_typed_program_is_sensible(
        seed in any::<u64>(),
        position in any::<prop::sample::Index>(),
        action in arb_action(),
    ) {
        let program = generate::well_typed_exp(seed);


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


        }
    }
}


#[test]
fn every_action_succeeds_somewhere_in_the_search_space() {
    let mut applied: Vec<(&'static str, usize)> = ALL_VARIANTS.iter().map(|n| (*n, 0)).collect();
    let mut total_applications = 0usize;

    for seed in 0..300u64 {
        let program = generate::well_typed_exp(seed);
        assert!(is_well_typed(&program));
        for cursor in all_positions(&program) {
            for action in one_of_every_action_in(&cursor.binders()) {
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

#[test]
fn actions_that_do_not_apply_leave_the_program_untouched() {
    let mut refusals = 0usize;
    for seed in 0..200u64 {
        let program = generate::well_typed_exp(seed);
        for cursor in all_positions(&program) {
            for action in one_of_every_action_in(&cursor.binders()) {
                let before = cursor.to_exp();
                if apply(cursor.clone(), action.clone()).is_none() {
                    refusals += 1;


                    assert_eq!(cursor.to_exp(), before);
                }
            }
        }
    }
    assert!(refusals > 0, "no action ever failed: the None branch is untested");
}

#[test]
fn the_check_would_catch_an_unsound_action() {


    let unsound = Exp::bin_op(Op::Add, Exp::num(1), Exp::bool_(true));
    assert!(
        !is_well_typed(&unsound),
        "this program must be ill-typed for the guard to mean anything"
    );

    let cursor = Zipper::new(Exp::bin_op(
        Op::Add,
        Exp::num(1),
        Exp::empty_hole(nothing_core::exp::HoleId::from_u128(0)),
    ))
    .move_child(1)
    .expect("the right operand exists");
    let after = apply(cursor, Action::ConstructBool(true)).expect("never told no");
    assert_ne!(after.to_exp(), unsound);
    assert!(is_well_typed(&after.to_exp()));
}