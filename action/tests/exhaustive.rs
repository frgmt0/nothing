use nothing_action::act::{Action, EditState, all_document_positions};
use nothing_action::generate;
use nothing_core::ctx::Ctx;
use nothing_core::doc::Doc;
use nothing_core::exp::{Exp, Id};
use nothing_core::ty::{Ty, variant_constructors};
use nothing_core::typing::{arm_payload_ty, is_well_typed, syn};
use proptest::prelude::*;

#[derive(Clone, PartialEq, Debug)]
struct MissingArm {
    missing: Vec<Id>,
    rendered: String,
}

fn missing_arms(ctx: &Ctx, exp: &Exp, out: &mut Vec<MissingArm>) {
    match exp {
        Exp::Var(_)
        | Exp::Num(_)
        | Exp::Bool(_)
        | Exp::Str(_)
        | Exp::Nil
        | Exp::Readline
        | Exp::EmptyHole(_) => {}
        Exp::Print(e) | Exp::CmdPure(e) => missing_arms(ctx, e, out),
        Exp::CmdBind(command, id, body) => {
            missing_arms(ctx, command, out);
            let yielded = syn(ctx, command)
                .as_ref()
                .and_then(nothing_core::ty::matched_cmd)
                .unwrap_or(Ty::Hole);
            missing_arms(&ctx.extend(*id, yielded), body, out);
        }
        Exp::Lam(id, ann, body) => missing_arms(&ctx.extend(*id, ann.clone()), body, out),
        Exp::Let(id, bound, body) => {
            missing_arms(ctx, bound, out);
            let bound_ty = syn(ctx, bound).unwrap_or(Ty::Hole);
            missing_arms(&ctx.extend(*id, bound_ty), body, out);
        }
        Exp::Ap(a, b) | Exp::BinOp(_, a, b) | Exp::Pair(a, b) | Exp::Cons(a, b) => {
            missing_arms(ctx, a, out);
            missing_arms(ctx, b, out);
        }
        Exp::If(a, b, c) | Exp::Fold(a, b, c) => {
            missing_arms(ctx, a, out);
            missing_arms(ctx, b, out);
            missing_arms(ctx, c, out);
        }
        Exp::Proj(_, e) | Exp::Field(e, _) | Exp::Inj(_, e) | Exp::NonEmptyHole(_, e) => {
            missing_arms(ctx, e, out)
        }
        Exp::Record(fields) => {
            for (_, value) in fields {
                missing_arms(ctx, value, out);
            }
        }
        Exp::Match(scrutinee, arms) => {
            missing_arms(ctx, scrutinee, out);
            let scrutinee_ty = syn(ctx, scrutinee).unwrap_or(Ty::Hole);
            let answered: Vec<Id> = arms.iter().map(|(ctor, _, _)| *ctor).collect();
            let required = variant_constructors(&scrutinee_ty).unwrap_or_default();
            let missing: Vec<Id> = required
                .into_iter()
                .filter(|ctor| !answered.contains(ctor))
                .collect();
            if !missing.is_empty() {
                out.push(MissingArm {
                    missing,
                    rendered: format!("{exp:?}"),
                });
            }
            for (ctor, binder, body) in arms {
                let payload = arm_payload_ty(&scrutinee_ty, *ctor);
                missing_arms(&ctx.extend(*binder, payload), body, out);
            }
        }
    }
}

fn unanswered(doc: &Doc) -> Vec<MissingArm> {
    let mut out = Vec::new();
    for def in doc.defs() {
        missing_arms(&doc.ctx(), &def.body, &mut out);
    }
    out
}

fn matches_in(doc: &Doc) -> usize {
    fn go(exp: &Exp) -> usize {
        let here = usize::from(matches!(exp, Exp::Match(..)));
        here + children(exp).into_iter().map(go).sum::<usize>()
    }
    doc.defs().iter().map(|def| go(&def.body)).sum()
}

fn children(exp: &Exp) -> Vec<&Exp> {
    match exp {
        Exp::Var(_)
        | Exp::Num(_)
        | Exp::Bool(_)
        | Exp::Str(_)
        | Exp::Nil
        | Exp::Readline
        | Exp::EmptyHole(_) => Vec::new(),
        Exp::Lam(_, _, b)
        | Exp::Proj(_, b)
        | Exp::Field(b, _)
        | Exp::Print(b)
        | Exp::CmdPure(b)
        | Exp::NonEmptyHole(_, b) => vec![b],
        Exp::Inj(_, b) => vec![b],
        Exp::Ap(a, b)
        | Exp::BinOp(_, a, b)
        | Exp::Let(_, a, b)
        | Exp::Pair(a, b)
        | Exp::CmdBind(a, _, b)
        | Exp::Cons(a, b) => vec![a, b],
        Exp::If(a, b, c) | Exp::Fold(a, b, c) => vec![a, b, c],
        Exp::Record(fields) => fields.iter().map(|(_, value)| value).collect(),
        Exp::Match(scrutinee, arms) => {
            let mut out = vec![&**scrutinee];
            out.extend(arms.iter().map(|(_, _, body)| body));
            out
        }
    }
}

fn arb_action() -> impl Strategy<Value = Action> {
    prop_oneof![
        (0usize..5).prop_map(Action::MoveChild),
        Just(Action::MoveParent),
        Just(Action::MoveNextSibling),
        Just(Action::MovePrevSibling),
        Just(Action::Delete),
        Just(Action::ConstructInj),
        Just(Action::ConstructMatch),
        Just(Action::AddArm),
        Just(Action::RemoveArm),
        Just(Action::ConstructLam),
        Just(Action::ConstructAp),
        Just(Action::ConstructIf),
        Just(Action::ConstructLet),
        Just(Action::ConstructPair),
        Just(Action::ConstructRecord),
        Just(Action::ConstructNonEmptyHole),
        Just(Action::Finish),
        Just(Action::CreateDefinition),
        Just(Action::DeleteDefinition),
        (0u128..8).prop_map(|n| Action::SetConstructor(Id::from_u128(n))),
        (0u128..8).prop_map(|n| Action::SetArmBinderId(Id::from_u128(n))),
        (0u128..8).prop_map(|n| Action::ConstructVar(Id::from_u128(n))),
        (0u128..8).prop_map(|n| Action::SetField(Id::from_u128(n))),
    ]
}

#[test]
fn the_check_would_notice_a_missing_arm_if_one_could_exist() {
    let red = Id::from_u128(101);
    let green = Id::from_u128(102);
    let x = Id::from_u128(103);

    let both = Exp::if_(
        Exp::bool_(true),
        Exp::inj(red, Exp::num(1)),
        Exp::inj(green, Exp::num(2)),
    );
    assert_eq!(
        syn(&Ctx::empty(), &both),
        Some(nothing_core::ty::variant([
            (red, Ty::Num),
            (green, Ty::Num)
        ])),
        "the scrutinee must be able to produce both cases"
    );

    let honest = Exp::match_(
        both.clone(),
        [(red, x, Exp::var(x)), (green, x, Exp::var(x))],
    );
    assert!(is_well_typed(&honest));
    assert!(unanswered(&Doc::single(honest)).is_empty());

    let missing = Exp::match_(both, [(red, x, Exp::var(x))]);
    let found = unanswered(&Doc::single(missing.clone()));
    assert_eq!(found.len(), 1, "the checker must see the unanswered case");
    assert_eq!(found[0].missing, vec![green]);
    assert!(
        !is_well_typed(&missing),
        "and the type system must refuse it, which is why no action can build it"
    );
}

#[test]
fn a_match_on_the_unknown_type_answers_for_nothing_and_so_needs_no_arms() {
    let e = Exp::match_(Exp::empty_hole(nothing_core::exp::HoleId::from_u128(0)), []);
    assert!(is_well_typed(&e));
    assert!(unanswered(&Doc::single(e)).is_empty());
}

#[test]
fn constructing_a_match_writes_one_arm_per_constructor_and_lands_in_the_first() {
    let red = Id::from_u128(201);
    let green = Id::from_u128(202);
    let blue = Id::from_u128(203);
    let x = Id::from_u128(204);

    let scrutinee = Exp::if_(
        Exp::bool_(true),
        Exp::inj(red, Exp::num(1)),
        Exp::if_(
            Exp::bool_(false),
            Exp::inj(green, Exp::num(2)),
            Exp::inj(blue, Exp::num(3)),
        ),
    );
    let start = EditState::new(Exp::lam(x, Ty::Num, scrutinee))
        .apply(Action::MoveChild(0))
        .expect("the lambda has a body");
    let after = start
        .apply(Action::ConstructMatch)
        .expect("a match fits over a variant");

    let arms = match after.zipper.clone().to_exp() {
        Exp::Lam(_, _, body) => match *body {
            Exp::Match(_, arms) => arms,
            other => panic!("expected a match, got {other:?}"),
        },
        other => panic!("expected a lambda, got {other:?}"),
    };
    assert_eq!(
        arms.iter().map(|(ctor, _, _)| *ctor).collect::<Vec<Id>>(),
        vec![red, green, blue],
        "one arm per constructor, in the order the type lists them"
    );
    assert!(
        arms.iter()
            .all(|(_, _, body)| matches!(body, Exp::EmptyHole(_))),
        "every arm starts as a hole"
    );
    assert!(
        matches!(after.zipper.focus, Exp::EmptyHole(_)),
        "and the cursor lands in the first of them"
    );
    assert!(after.is_well_typed());
    assert!(unanswered(&after.doc()).is_empty());
}

#[test]
fn adding_an_arm_reaches_every_match_that_answers_the_same_way() {
    let red = Id::from_u128(301);
    let x = Id::from_u128(302);
    let y = Id::from_u128(303);

    let a_match = || {
        Exp::match_(
            Exp::inj(red, Exp::num(1)),
            [(
                red,
                x,
                Exp::bin_op(nothing_core::exp::Op::Add, Exp::var(x), Exp::num(0)),
            )],
        )
    };
    let doc = Doc::new(vec![
        nothing_core::doc::Def::new(Id::from_u128(310), Ty::Hole, a_match()),
        nothing_core::doc::Def::new(
            Id::from_u128(311),
            Ty::Hole,
            Exp::lam(y, Ty::Num, a_match()),
        ),
    ])
    .expect("two distinct definitions");

    let state = EditState::with_doc(&doc, nothing_core::names::NameTable::new(), 0)
        .expect("a first definition");
    let after = state
        .apply(Action::AddArm)
        .expect("an arm can always be added to a match");

    for def in after.doc().defs() {
        let count = arm_count(&def.body);
        assert_eq!(
            count,
            Some(2),
            "every match on the same case set grew an arm: {:?}",
            def.body
        );
    }
    assert!(after.is_well_typed());
    assert!(unanswered(&after.doc()).is_empty());

    let back = after
        .apply(Action::RemoveArm)
        .expect("the arm that nothing injects can go again");
    for def in back.doc().defs() {
        assert_eq!(arm_count(&def.body), Some(1));
    }
}

fn arm_count(exp: &Exp) -> Option<usize> {
    match exp {
        Exp::Match(_, arms) => Some(arms.len()),
        _ => children(exp).into_iter().find_map(arm_count),
    }
}

#[test]
fn an_arm_the_scrutinee_still_injects_cannot_be_removed() {
    let red = Id::from_u128(401);
    let x = Id::from_u128(402);
    let state = EditState::new(Exp::match_(
        Exp::inj(red, Exp::num(1)),
        [(red, x, Exp::var(x))],
    ))
    .apply(Action::MoveChild(1))
    .expect("the arm body exists");

    assert!(
        state.apply(Action::RemoveArm).is_none(),
        "removing the only arm answering an injected case would leave a match that cannot answer"
    );
    assert!(
        state.apply(Action::Delete).is_some(),
        "but emptying the arm body is always allowed: the two edits are different"
    );
}

#[test]
fn an_arm_can_be_re_aimed_at_another_case_but_never_off_one_the_scrutinee_injects() {
    let red = Id::from_u128(501);
    let green = Id::from_u128(502);
    let x = Id::from_u128(503);
    let y = Id::from_u128(504);

    let dead = EditState::new(Exp::match_(
        Exp::empty_hole(nothing_core::exp::HoleId::from_u128(9)),
        [(red, x, Exp::num(1)), (green, y, Exp::num(2))],
    ))
    .apply(Action::MoveChild(1))
    .expect("the first arm's body exists");
    let blue = Id::from_u128(505);
    let re_aimed = dead
        .apply(Action::SetConstructor(blue))
        .expect("an arm nothing injects can be pointed at another case");
    assert!(re_aimed.is_well_typed());
    assert!(
        re_aimed.render_document().contains("_00000000"),
        "the arm now answers for a case with no display name: {}",
        re_aimed.render_document()
    );
    assert!(
        dead.apply(Action::SetConstructor(green)).is_none(),
        "and never onto a case another arm already answers for"
    );

    let live = EditState::new(Exp::match_(
        Exp::inj(red, Exp::num(1)),
        [(red, x, Exp::var(x))],
    ))
    .apply(Action::MoveChild(1))
    .expect("the arm body exists");
    assert!(
        live.apply(Action::SetConstructor(blue)).is_none(),
        "re-aiming the only arm answering an injected case is refused for the same reason \
         removing it is"
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 10_000,
        max_shrink_iters: 4_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn every_match_a_random_action_walk_can_reach_answers_for_every_case(
        seed in any::<u64>(),
        actions in prop::collection::vec(arb_action(), 1..40),
    ) {
        let (doc, names) = generate::well_typed_doc(seed);
        let mut state = EditState::with_doc(&doc, names, 0)
            .expect("a generated document always has a first definition");
        prop_assert!(unanswered(&state.doc()).is_empty());

        for action in actions {
            if let Some(next) = state.apply(action.clone()) {
                let gaps = unanswered(&next.doc());
                prop_assert!(
                    gaps.is_empty(),
                    "{action:?} produced {:?} in\n{}",
                    gaps.first(),
                    next.render_document()
                );
                state = next;
            }
        }
    }
}

#[test]
fn the_walk_actually_visits_matches() {
    let mut seen = 0usize;
    let mut with_matches = 0usize;
    for seed in 0..400u64 {
        let (doc, _) = generate::well_typed_doc(seed);
        let here = matches_in(&doc);
        seen += here;
        if here > 0 {
            with_matches += 1;
        }
        assert!(unanswered(&doc).is_empty(), "seed {seed} generated a gap");
    }
    assert!(
        with_matches > 20,
        "only {with_matches} of 400 generated documents contained a match, so the \
         exhaustiveness property is close to vacuous"
    );
    assert!(seen > 40, "only {seen} matches over 400 documents");
}

#[test]
fn every_cursor_position_of_a_document_with_a_match_is_still_exhaustive() {
    for seed in 0..200u64 {
        let (doc, names) = generate::well_typed_doc(seed);
        let root = EditState::with_doc(&doc, names, 0).expect("a first definition");
        for cursor in all_document_positions(&root) {
            for action in [
                Action::AddArm,
                Action::RemoveArm,
                Action::ConstructMatch,
                Action::ConstructInj,
                Action::Delete,
            ] {
                if let Some(after) = cursor.apply(action.clone()) {
                    assert!(
                        unanswered(&after.doc()).is_empty(),
                        "{action:?} left a match unanswered at seed {seed}:\n{}",
                        after.render_document()
                    );
                }
            }
        }
    }
}
