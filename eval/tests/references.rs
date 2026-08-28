use nothing_action::script::replay_script;
use nothing_core::exp::{Exp, Id, Op, Side};
use nothing_core::names::NameTable;
use nothing_core::render::render;
use nothing_core::ty::Ty;
use nothing_core::typing::is_well_typed;
use nothing_eval::dynamic::Dyn;
use nothing_eval::fixpoint::{Binders, fix};
use nothing_eval::step::{HoleKind, eval};

const FACTORIAL: &str = include_str!("../../bench/fixtures/factorial.actions");
const LIST_MAP: &str = include_str!("../../bench/fixtures/list_map.actions");
const RECORD: &str = include_str!("../../bench/fixtures/record.actions");
const STATE_MACHINE: &str = include_str!("../../bench/fixtures/state_machine.actions");
const NESTED_CONDITIONAL: &str = include_str!("../../bench/fixtures/nested_conditional.actions");

fn reference(name: &str, actions: &str) -> (Exp, NameTable) {
    let state = replay_script(actions).unwrap_or_else(|e| panic!("{name}: {e}"));
    let exp = state.exp();
    assert!(is_well_typed(&exp), "{name} is not well-typed");
    (exp, state.names.clone())
}

fn fill_first_hole(exp: &Exp, with: &Exp) -> Option<Exp> {
    match exp {
        Exp::EmptyHole(_) => Some(with.clone()),
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) => None,
        Exp::Lam(id, ty, body) => Some(Exp::lam(*id, ty.clone(), fill_first_hole(body, with)?)),
        Exp::Proj(side, body) => Some(Exp::proj(*side, fill_first_hole(body, with)?)),
        Exp::NonEmptyHole(h, body) => Some(Exp::non_empty_hole(*h, fill_first_hole(body, with)?)),
        Exp::Ap(a, b) => two(a, b, with, &|a, b| Exp::ap(a, b)),
        Exp::BinOp(op, a, b) => two(a, b, with, &|a, b| Exp::bin_op(*op, a, b)),
        Exp::Let(id, a, b) => two(a, b, with, &|a, b| Exp::let_(*id, a, b)),
        Exp::Pair(a, b) => two(a, b, with, &|a, b| Exp::pair(a, b)),
        Exp::If(c, t, e) => match fill_first_hole(c, with) {
            Some(c) => Some(Exp::if_(c, (**t).clone(), (**e).clone())),
            None => match fill_first_hole(t, with) {
                Some(t) => Some(Exp::if_((**c).clone(), t, (**e).clone())),
                None => Some(Exp::if_(
                    (**c).clone(),
                    (**t).clone(),
                    fill_first_hole(e, with)?,
                )),
            },
        },
    }
}

fn two(a: &Exp, b: &Exp, with: &Exp, build: &dyn Fn(Exp, Exp) -> Exp) -> Option<Exp> {
    match fill_first_hole(a, with) {
        Some(a) => Some(build(a, b.clone())),
        None => Some(build(a.clone(), fill_first_hole(b, with)?)),
    }
}

fn lam_binder(exp: &Exp) -> Id {
    match exp {
        Exp::Lam(id, _, _) => *id,
        other => panic!("expected a lambda, got {other:?}"),
    }
}

fn recursive_factorial() -> (Exp, NameTable) {
    let (fixture, mut names) = reference("factorial", FACTORIAL);
    let n = lam_binder(&fixture);
    let fac = Id::fresh();
    let call = Exp::ap(
        Exp::var(fac),
        Exp::bin_op(Op::Sub, Exp::var(n), Exp::num(1)),
    );
    let body = fill_first_hole(&fixture, &call).expect("the fixture has one hole");
    let binders = Binders::fresh();
    binders.name(&mut names);
    names.set(fac, "factorial");
    (fix(binders, Exp::lam(fac, Ty::Hole, body)), names)
}

#[test]
fn reference_one_factorial_computes() {
    let (factorial, names) = recursive_factorial();
    assert!(is_well_typed(&factorial));

    for (input, expected) in [(0, 1), (1, 1), (2, 2), (3, 6), (5, 120), (12, 479_001_600)] {
        let applied = Exp::ap(factorial.clone(), Exp::num(input));
        assert!(is_well_typed(&applied));
        assert_eq!(
            eval(&applied).num(),
            Some(expected),
            "factorial({input}) — {}",
            render(&applied, &names)
        );
    }
}

#[test]
fn reference_one_factorial_with_its_hole_still_open_blocks_on_that_hole() {
    let (fixture, names) = reference("factorial", FACTORIAL);
    let n = lam_binder(&fixture);

    let applied = Exp::ap(fixture, Exp::num(5));
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
    let (map, names) = reference("list_map", LIST_MAP);
    assert!(eval(&map).is_value(), "a curried function is a value");

    let k = Id::fresh();
    let double = Exp::lam(k, Ty::Num, Exp::bin_op(Op::Mul, Exp::var(k), Exp::num(2)));

    let applied = Exp::ap(Exp::ap(map, double), Exp::pair(Exp::num(3), Exp::num(4)));
    assert!(is_well_typed(&applied));

    let outcome = eval(&applied);
    assert!(outcome.is_value(), "{outcome:?}");
    assert_eq!(
        nothing_eval::dynamic::render(outcome.dyn_result(), &names),
        "(6, 8)"
    );
}

#[test]
fn reference_three_record_constructs_and_accesses() {
    let (record, names) = reference("record", RECORD);

    let constructor = match &record {
        Exp::Let(_, bound, _) => (**bound).clone(),
        other => panic!("expected a let, got {other:?}"),
    };
    let built = Exp::ap(Exp::ap(constructor, Exp::num(3)), Exp::num(4));
    let outcome = eval(&built);
    assert_eq!(
        nothing_eval::dynamic::render(outcome.dyn_result(), &names),
        "(3, 4)",
        "the two-field constructor"
    );

    let accessed = Exp::ap(record.clone(), Exp::pair(Exp::num(3), Exp::num(4)));
    assert!(is_well_typed(&accessed));
    assert_eq!(eval(&accessed).num(), Some(3), "the accessor");

    let direct = Exp::proj(Side::R, Exp::pair(Exp::num(3), Exp::num(4)));
    assert_eq!(eval(&direct).num(), Some(4));
}

#[test]
fn reference_four_state_machine_transitions() {
    let (machine, _) = reference("state_machine", STATE_MACHINE);
    assert!(eval(&machine).is_value());

    for (state, next) in [(0, 1), (1, 2), (2, 0), (7, 0)] {
        let applied = Exp::ap(machine.clone(), Exp::num(state));
        assert!(is_well_typed(&applied));
        assert_eq!(eval(&applied).num(), Some(next), "transition({state})");
    }
}

#[test]
fn reference_five_nested_conditional_classifies() {
    let (classify, _) = reference("nested_conditional", NESTED_CONDITIONAL);
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
        let (exp, _) = reference(name, actions);
        assert!(!eval(&exp).is_out_of_fuel(), "{name} did not settle");
    }
}
