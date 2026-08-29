use nothing_action::script::replay_script;
use nothing_core::doc::{Def, Doc};
use nothing_core::exp::{Exp, HoleId, Id, Op, Side};
use nothing_core::names::NameTable;
use nothing_core::render::render;
use nothing_core::ty::Ty;
use nothing_core::typing::is_well_typed;
use nothing_eval::dynamic::Dyn;
use nothing_eval::fixpoint::{Binders, fix};
use nothing_eval::step::{HoleKind, Outcome, eval, eval_doc};

const FACTORIAL: &str = include_str!("../../bench/fixtures/factorial.actions");
const LIST_MAP: &str = include_str!("../../bench/fixtures/list_map.actions");
const RECORD: &str = include_str!("../../bench/fixtures/record.actions");
const STATE_MACHINE: &str = include_str!("../../bench/fixtures/state_machine.actions");
const NESTED_CONDITIONAL: &str = include_str!("../../bench/fixtures/nested_conditional.actions");

fn reference(name: &str, actions: &str) -> (Doc, NameTable) {
    let state = replay_script(actions).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert!(state.is_well_typed(), "{name} is not a well-typed document");
    (state.doc(), state.names.clone())
}

fn definition(doc: &Doc, names: &NameTable, name: &str) -> Def {
    doc.defs()
        .iter()
        .find(|def| names.get(def.id) == Some(name))
        .cloned()
        .unwrap_or_else(|| panic!("no definition named `{name}` in {}", doc.render(names)))
}

fn main_of(doc: &Doc, names: &NameTable) -> Def {
    definition(doc, names, "main")
}

fn call(doc: &Doc, exp: Exp) -> Outcome {
    let caller = Id::from_u128(0xca11);
    let mut defs = doc.defs().to_vec();
    defs.push(Def::new(caller, Ty::Hole, exp));
    eval_doc(&Doc::new(defs).expect("the caller id is fresh"), caller)
}

fn drop_calls_to(exp: &Exp, target: Id, next: &mut u128) -> Exp {
    match exp {
        Exp::Ap(f, _) if matches!(**f, Exp::Var(id) if id == target) => {
            *next += 1;
            Exp::empty_hole(HoleId::from_u128(*next))
        }
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::Str(_) | Exp::Nil | Exp::EmptyHole(_) => {
            exp.clone()
        }
        Exp::Lam(id, ty, body) => Exp::lam(*id, ty.clone(), drop_calls_to(body, target, next)),
        Exp::Proj(side, body) => Exp::proj(*side, drop_calls_to(body, target, next)),
        Exp::NonEmptyHole(h, body) => Exp::non_empty_hole(*h, drop_calls_to(body, target, next)),
        Exp::Ap(a, b) => Exp::ap(
            drop_calls_to(a, target, next),
            drop_calls_to(b, target, next),
        ),
        Exp::BinOp(op, a, b) => Exp::bin_op(
            *op,
            drop_calls_to(a, target, next),
            drop_calls_to(b, target, next),
        ),
        Exp::Let(id, a, b) => Exp::let_(
            *id,
            drop_calls_to(a, target, next),
            drop_calls_to(b, target, next),
        ),
        Exp::Pair(a, b) => Exp::pair(
            drop_calls_to(a, target, next),
            drop_calls_to(b, target, next),
        ),
        Exp::Cons(a, b) => Exp::cons(
            drop_calls_to(a, target, next),
            drop_calls_to(b, target, next),
        ),
        Exp::Fold(a, b, c) => Exp::fold(
            drop_calls_to(a, target, next),
            drop_calls_to(b, target, next),
            drop_calls_to(c, target, next),
        ),
        Exp::If(c, t, e) => Exp::if_(
            drop_calls_to(c, target, next),
            drop_calls_to(t, target, next),
            drop_calls_to(e, target, next),
        ),
    }
}

fn lam_binder(exp: &Exp) -> Id {
    match exp {
        Exp::Lam(id, _, _) => *id,
        other => panic!("expected a lambda, got {other:?}"),
    }
}

#[test]
fn reference_one_factorial_computes() {
    let (doc, names) = reference("factorial", FACTORIAL);
    let main = main_of(&doc, &names);

    for (input, expected) in [(0, 1), (1, 1), (2, 2), (3, 6), (5, 120), (12, 479_001_600)] {
        let outcome = call(&doc, Exp::ap(Exp::var(main.id), Exp::num(input)));
        assert_eq!(
            outcome.num(),
            Some(expected),
            "factorial({input}) — {}",
            doc.render(&names)
        );
    }
}

#[test]
fn the_z_combinator_still_computes_factorial_beside_the_definition() {
    let (doc, mut names) = reference("factorial", FACTORIAL);
    let main = main_of(&doc, &names);

    let binders = Binders::fresh();
    binders.name(&mut names);
    let factorial = fix(binders, Exp::lam(main.id, Ty::Hole, main.body.clone()));
    assert!(
        is_well_typed(&factorial),
        "the combinator form is still well-typed on its own"
    );

    for (input, expected) in [(0, 1), (1, 1), (2, 2), (3, 6), (5, 120), (12, 479_001_600)] {
        let applied = Exp::ap(factorial.clone(), Exp::num(input));
        assert!(is_well_typed(&applied));
        assert_eq!(
            eval(&applied).num(),
            Some(expected),
            "factorial({input}) — {}",
            render(&applied, &names)
        );
        assert_eq!(
            call(&doc, Exp::ap(Exp::var(main.id), Exp::num(input))).num(),
            eval(&applied).num(),
            "the definition and the combinator disagree at {input}"
        );
    }
}

#[test]
fn factorial_with_its_recursive_call_removed_blocks_on_that_hole() {
    let (doc, names) = reference("factorial", FACTORIAL);
    let main = main_of(&doc, &names);

    let mut next = 0;
    let holey = drop_calls_to(&main.body, main.id, &mut next);
    assert_eq!(next, 1, "factorial calls itself exactly once");
    let n = lam_binder(&holey);

    let applied = Exp::ap(holey, Exp::num(5));
    assert!(is_well_typed(&applied));

    let outcome = eval(&applied);
    assert!(
        outcome.is_indeterminate(),
        "a missing recursive call is a hole, not a crash: {outcome:?}"
    );
    assert_eq!(
        nothing_eval::dynamic::render(outcome.dyn_result(), &names),
        "5 * ⦇⦈",
        "everything that could run, ran"
    );

    let blocked = &outcome.blocked()[0];
    assert_eq!(blocked.kind, HoleKind::Empty);
    assert_eq!(
        blocked.known(),
        vec![(n, Dyn::Num(5))],
        "and the editor can say what was in scope when it stopped"
    );
}

#[test]
fn reference_two_list_map_maps() {
    let (doc, names) = reference("list_map", LIST_MAP);
    let map = main_of(&doc, &names).body;
    assert!(eval(&map).is_value(), "a curried function is a value");

    let k = Id::fresh();
    let double = Exp::lam(k, Ty::Num, Exp::bin_op(Op::Mul, Exp::var(k), Exp::num(2)));

    let list = Exp::cons(
        Exp::num(3),
        Exp::cons(Exp::num(4), Exp::cons(Exp::num(5), Exp::Nil)),
    );
    let applied = Exp::ap(Exp::ap(map, double), list);
    assert!(is_well_typed(&applied));

    let outcome = eval(&applied);
    assert!(outcome.is_value(), "{outcome:?}");
    assert_eq!(
        nothing_eval::dynamic::render(outcome.dyn_result(), &names),
        "6 :: 8 :: 10 :: nil",
        "the fold rebuilt the whole spine"
    );
}

#[test]
fn reference_three_record_constructs_and_accesses() {
    let (doc, names) = reference("record", RECORD);
    assert_eq!(doc.len(), 2, "the record reference is two definitions now");
    let mk = definition(&doc, &names, "mk");
    let main = main_of(&doc, &names);

    let built = call(
        &doc,
        Exp::ap(Exp::ap(Exp::var(mk.id), Exp::num(3)), Exp::num(4)),
    );
    assert_eq!(
        nothing_eval::dynamic::render(built.dyn_result(), &names),
        "(3, 4)",
        "the two-field constructor, called by name across definitions"
    );

    let accessed = call(
        &doc,
        Exp::ap(Exp::var(main.id), Exp::pair(Exp::num(3), Exp::num(4))),
    );
    assert_eq!(accessed.num(), Some(3), "the accessor");

    let both = call(
        &doc,
        Exp::ap(
            Exp::var(main.id),
            Exp::ap(Exp::ap(Exp::var(mk.id), Exp::num(3)), Exp::num(4)),
        ),
    );
    assert_eq!(
        both.num(),
        Some(3),
        "and one definition composed with the other"
    );

    let direct = Exp::proj(Side::R, Exp::pair(Exp::num(3), Exp::num(4)));
    assert_eq!(eval(&direct).num(), Some(4));
}

#[test]
fn reference_four_state_machine_transitions() {
    let (doc, names) = reference("state_machine", STATE_MACHINE);
    let machine = main_of(&doc, &names).body;
    assert!(eval(&machine).is_value());

    for (state, next) in [(0, 1), (1, 2), (2, 0), (7, 0)] {
        let applied = Exp::ap(machine.clone(), Exp::num(state));
        assert!(is_well_typed(&applied));
        assert_eq!(eval(&applied).num(), Some(next), "transition({state})");
    }
}

#[test]
fn reference_five_nested_conditional_classifies() {
    let (doc, names) = reference("nested_conditional", NESTED_CONDITIONAL);
    let classify = main_of(&doc, &names).body;
    assert!(eval(&classify).is_value());

    for (input, expected) in [
        (-3, 0),
        (0, 0),
        (1, 1),
        (10, 1),
        (11, 2),
        (100, 2),
        (101, 3),
    ] {
        let applied = Exp::ap(classify.clone(), Exp::num(input));
        assert!(is_well_typed(&applied));
        assert_eq!(eval(&applied).num(), Some(expected), "classify({input})");
    }
}

#[test]
fn every_reference_program_evaluates_without_fuel_exhaustion() {
    for (name, actions) in [
        ("factorial", FACTORIAL),
        ("list_map", LIST_MAP),
        ("record", RECORD),
        ("state_machine", STATE_MACHINE),
        ("nested_conditional", NESTED_CONDITIONAL),
    ] {
        let (doc, names) = reference(name, actions);
        for def in doc.defs() {
            let outcome = eval_doc(&doc, def.id);
            assert!(
                !outcome.is_out_of_fuel(),
                "{name}'s definition {} did not settle",
                names.get(def.id).unwrap_or("?")
            );
        }
    }
}
