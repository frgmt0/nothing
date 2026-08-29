use nothing_core::exp::{Exp, HoleId, Id, Op};
use nothing_core::stack::on_deep_stack;
use nothing_core::ty::Ty;
use nothing_core::typing::is_well_typed;
use nothing_eval::dynamic::{Dyn, elaborate, is_value, size, subst, to_exp};
use nothing_eval::perform::{Io, Recorded, perform_in};
use nothing_eval::step::{Defs, Outcome, blocked_holes, eval, eval_with_fuel, run, step};

const CI_STACK_BYTES: usize = 2 * 1024 * 1024;

fn on_a_ci_sized_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(CI_STACK_BYTES)
        .spawn(work)
        .expect("spawn the small-stack thread a CI runner would give a test")
        .join()
        .expect("the small-stack thread finished without overflowing")
}

fn x() -> Id {
    Id::from_u128(1)
}

fn y() -> Id {
    Id::from_u128(2)
}

fn adder() -> Exp {
    Exp::lam(
        x(),
        Ty::Num,
        Exp::lam(
            y(),
            Ty::Num,
            Exp::bin_op(Op::Add, Exp::var(x()), Exp::var(y())),
        ),
    )
}

fn long_list(n: i64) -> Exp {
    Exp::list((0..n).map(Exp::num))
}

fn list_ending_in_a_hole(n: i64, hole: HoleId) -> Exp {
    let mut list = Exp::empty_hole(hole);
    for value in (0..n).rev() {
        list = Exp::cons(Exp::num(value), list);
    }
    list
}

fn nested_lets(depth: u128) -> Exp {
    let mut body = Exp::var(Id::from_u128(depth));
    for level in (1..=depth).rev() {
        let bound = if level == 1 {
            Exp::num(1)
        } else {
            Exp::bin_op(Op::Add, Exp::var(Id::from_u128(level - 1)), Exp::num(1))
        };
        body = Exp::let_(Id::from_u128(level), bound, body);
    }
    body
}

fn nested_binds(depth: u128) -> Exp {
    let mut body = Exp::print(Exp::str_("last"));
    for level in (1..=depth).rev() {
        body = Exp::cmd_bind(Exp::print(Exp::str_("line")), Id::from_u128(level), body);
    }
    body
}

#[test]
fn folding_a_long_list_on_a_ci_sized_stack_does_not_overflow() {
    on_a_ci_sized_stack(|| {
        let sum = Exp::fold(long_list(400), Exp::num(0), adder());
        assert!(is_well_typed(&sum));
        assert!(eval_with_fuel(&sum, 100).is_out_of_fuel());
        assert_eq!(eval_with_fuel(&sum, 100_000).num(), Some((0..400).sum()));
    });
}

#[test]
fn a_long_list_literal_settles_as_a_value_on_a_ci_sized_stack() {
    on_a_ci_sized_stack(|| {
        let list = long_list(5_000);
        let outcome = run(elaborate(&list), 1_000);
        assert!(outcome.is_value(), "a list of literals is already a value");
        assert_eq!(size(outcome.dyn_result()), 10_001);
        assert!(blocked_holes(outcome.dyn_result()).is_empty());
    });
}

#[test]
fn a_fifty_thousand_element_list_literal_runs_to_a_value() {
    on_a_ci_sized_stack(|| {
        let (valued, nodes, holes, projects_back) = on_deep_stack(|| {
            let list = long_list(50_000);
            let outcome = run(elaborate(&list), 1_000);
            let projects_back = to_exp(outcome.dyn_result()) == list;
            (
                outcome.is_value() && is_value(outcome.dyn_result()),
                size(outcome.dyn_result()),
                blocked_holes(outcome.dyn_result()).len(),
                projects_back,
            )
        });
        assert!(valued, "fifty thousand cells are still a value");
        assert_eq!(nodes, 100_001);
        assert_eq!(holes, 0);
        assert!(projects_back, "the residual projects back to the program");
    });
}

#[test]
fn a_hole_at_the_end_of_a_long_list_is_the_one_thing_reported() {
    on_a_ci_sized_stack(|| {
        let outcome = eval(&list_ending_in_a_hole(5_000, HoleId::from_u128(9)));
        assert!(outcome.is_indeterminate(), "the tail is still a hole");
        assert_eq!(outcome.blocked().len(), 1);
        assert_eq!(outcome.blocked()[0].hole, HoleId::from_u128(9));
    });
}

#[test]
fn a_hole_at_the_end_of_a_fifty_thousand_element_list_is_still_the_one_thing_reported() {
    on_a_ci_sized_stack(|| {
        let (indeterminate, blocked) = on_deep_stack(|| {
            let outcome = eval(&list_ending_in_a_hole(50_000, HoleId::from_u128(9)));
            (
                outcome.is_indeterminate(),
                outcome.blocked().iter().map(|b| b.hole).collect::<Vec<_>>(),
            )
        });
        assert!(indeterminate);
        assert_eq!(blocked, vec![HoleId::from_u128(9)]);
    });
}

#[test]
fn a_deep_chain_of_lets_evaluates_to_its_last_binding() {
    on_a_ci_sized_stack(|| {
        let program = nested_lets(3_000);
        assert!(is_well_typed(&program));
        assert_eq!(eval(&program).num(), Some(3_000));
    });
}

#[test]
fn a_deep_chain_of_binds_performs_every_command_in_order() {
    on_a_ci_sized_stack(|| {
        let program = nested_binds(2_000);
        assert!(is_well_typed(&program));
        let mut io = Recorded::default();
        let performance = perform_in(
            &Defs::new(),
            elaborate(&program),
            1_000_000,
            &mut io as &mut (dyn Io + Send),
        );
        assert!(performance.outcome.is_value(), "{:?}", performance.outcome);
        assert_eq!(io.written.len(), 2_001);
        assert_eq!(io.written.last().map(String::as_str), Some("last"));
    });
}

#[test]
fn substituting_through_a_very_long_spine_rewrites_every_cell() {
    on_a_ci_sized_stack(|| {
        let (valued, nodes, settled) = on_deep_stack(|| {
            let mut list = Exp::Nil;
            for _ in 0..50_000 {
                list = Exp::cons(Exp::var(x()), list);
            }
            let substituted = subst(x(), &Dyn::Num(7), &elaborate(&list));
            (
                is_value(&substituted),
                size(&substituted),
                step(&substituted).is_none(),
            )
        });
        assert!(valued, "every cell became a literal");
        assert_eq!(nodes, 100_001);
        assert!(settled, "a value has nowhere left to go");
    });
}

#[test]
fn a_deeply_nested_residual_still_reports_its_outcome() {
    on_a_ci_sized_stack(|| {
        let mut sum = Exp::empty_hole(HoleId::from_u128(3));
        for value in 0..5_000i64 {
            sum = Exp::bin_op(Op::Add, Exp::num(value), sum);
        }
        let outcome = eval(&sum);
        let Outcome::Indeterminate { result, blocked } = &outcome else {
            panic!("expected an indeterminate outcome, got {outcome:?}");
        };
        assert_eq!(blocked.len(), 1);
        assert_eq!(size(result), 10_001);
    });
}
