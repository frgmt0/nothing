
use nothing_action::generate;
use nothing_core::exp::Exp;
use nothing_core::typing::is_well_typed;
use nothing_eval::dynamic::{self, Dyn};
use nothing_eval::step::{Outcome, eval_with_fuel};
use proptest::prelude::*;

const FUEL: usize = 2_000;

fn holes(exp: &Exp) -> usize {
    match exp {
        Exp::EmptyHole(_) => 1,
        Exp::NonEmptyHole(_, inner) => 1 + holes(inner),
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) => 0,
        Exp::Lam(_, _, b) | Exp::Proj(_, b) => holes(b),
        Exp::Ap(a, b) | Exp::BinOp(_, a, b) | Exp::Let(_, a, b) | Exp::Pair(a, b) => {
            holes(a) + holes(b)
        }
        Exp::If(c, t, e) => holes(c) + holes(t) + holes(e),
    }
}

fn free_vars(d: &Dyn, bound: &mut Vec<nothing_core::exp::Id>, out: &mut Vec<nothing_core::exp::Id>) {
    match d {
        Dyn::Var(id) => {
            if !bound.contains(id) {
                out.push(*id);
            }
        }
        Dyn::Lam(id, _, body) => {
            bound.push(*id);
            free_vars(body, bound, out);
            bound.pop();
        }
        Dyn::Let(id, boundto, body) => {
            free_vars(boundto, bound, out);
            bound.push(*id);
            free_vars(body, bound, out);
            bound.pop();
        }
        Dyn::Ap(a, b) | Dyn::BinOp(_, a, b) | Dyn::Pair(a, b) => {
            free_vars(a, bound, out);
            free_vars(b, bound, out);
        }
        Dyn::If(c, t, e) => {
            free_vars(c, bound, out);
            free_vars(t, bound, out);
            free_vars(e, bound, out);
        }
        Dyn::Proj(_, inner) | Dyn::NonEmptyHole(_, _, inner) => free_vars(inner, bound, out),
        Dyn::Num(_) | Dyn::Bool(_) => {}
        Dyn::EmptyHole(_, env) => {
            for (id, value) in env.iter() {
                if value != &Dyn::Var(*id) {
                    free_vars(value, bound, out);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2_000))]

    #[test]
    fn evaluating_a_well_typed_program_always_settles_into_one_of_three_outcomes(
        seed in any::<u64>()
    ) {
        let e = generate::well_typed_exp(seed);
        prop_assert!(is_well_typed(&e));

        let outcome = eval_with_fuel(&e, FUEL);
        prop_assert_eq!(
            usize::from(outcome.is_value())
                + usize::from(outcome.is_indeterminate())
                + usize::from(outcome.is_out_of_fuel()),
            1,
            "{:?}",
            outcome
        );
    }

    #[test]
    fn a_value_is_hole_free_and_an_indeterminate_result_is_not_a_value(seed in any::<u64>()) {
        let e = generate::well_typed_exp(seed);
        match eval_with_fuel(&e, FUEL) {
            Outcome::Value(result) => {
                prop_assert!(dynamic::is_value(&result));
            }
            Outcome::Indeterminate { result, blocked } => {
                prop_assert!(!dynamic::is_value(&result));
                prop_assert!(
                    blocked.len() <= holes(&dynamic::to_exp(&result)),
                    "more holes were blamed than are left in the residual"
                );
            }
            Outcome::OutOfFuel { .. } => {}
        }
    }

    #[test]
    fn a_hole_free_program_never_blames_a_hole(seed in any::<u64>()) {
        let e = generate::well_typed_exp(seed);
        if holes(&e) == 0 {
            let outcome = eval_with_fuel(&e, FUEL);
            prop_assert!(outcome.blocked().is_empty(), "{outcome:?}");
            prop_assert_eq!(holes(&outcome.to_exp()), 0, "evaluation invented a hole");
        }
    }

    #[test]
    fn a_closed_program_stays_closed_all_the_way_down(seed in any::<u64>()) {
        let e = generate::well_typed_exp(seed);
        let outcome = eval_with_fuel(&e, FUEL);

        let mut bound = Vec::new();
        let mut free = Vec::new();
        free_vars(outcome.dyn_result(), &mut bound, &mut free);
        prop_assert!(
            free.is_empty(),
            "substitution leaked {free:?} out of its binder"
        );
    }

    #[test]
    fn evaluation_is_deterministic(seed in any::<u64>()) {
        let e = generate::well_typed_exp(seed);
        prop_assert_eq!(eval_with_fuel(&e, FUEL), eval_with_fuel(&e, FUEL));
    }
}
