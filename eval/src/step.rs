use nothing_core::exp::{Exp, HoleId, Id, Op, Side};

use crate::dynamic::{Dyn, Env, elaborate, is_value, subst};

pub const DEFAULT_FUEL: usize = 200_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HoleKind {
    Empty,
    NonEmpty,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Blocked {
    pub hole: HoleId,
    pub kind: HoleKind,
    pub env: Env,
}

impl Blocked {
    pub fn bindings(&self) -> Vec<(Id, Dyn)> {
        let mut out: Vec<(Id, Dyn)> = self.env.iter().map(|(id, d)| (*id, d.clone())).collect();
        out.sort_by_key(|(id, _)| *id);
        out
    }

    pub fn known(&self) -> Vec<(Id, Dyn)> {
        self.bindings()
            .into_iter()
            .filter(|(id, d)| *d != Dyn::Var(*id))
            .collect()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Outcome {
    Value(Dyn),
    Indeterminate { result: Dyn, blocked: Vec<Blocked> },
    OutOfFuel { partial: Dyn, steps: usize },
}

impl Outcome {
    pub fn is_value(&self) -> bool {
        matches!(self, Outcome::Value(_))
    }

    pub fn is_indeterminate(&self) -> bool {
        matches!(self, Outcome::Indeterminate { .. })
    }

    pub fn is_out_of_fuel(&self) -> bool {
        matches!(self, Outcome::OutOfFuel { .. })
    }

    pub fn is_stuck(&self) -> bool {
        matches!(self, Outcome::Indeterminate { blocked, .. } if blocked.is_empty())
    }

    pub fn dyn_result(&self) -> &Dyn {
        match self {
            Outcome::Value(d)
            | Outcome::Indeterminate { result: d, .. }
            | Outcome::OutOfFuel { partial: d, .. } => d,
        }
    }

    pub fn blocked(&self) -> &[Blocked] {
        match self {
            Outcome::Indeterminate { blocked, .. } => blocked,
            _ => &[],
        }
    }

    pub fn num(&self) -> Option<i64> {
        match self {
            Outcome::Value(Dyn::Num(n)) => Some(*n),
            _ => None,
        }
    }

    pub fn bool(&self) -> Option<bool> {
        match self {
            Outcome::Value(Dyn::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    pub fn to_exp(&self) -> Exp {
        crate::dynamic::to_exp(self.dyn_result())
    }
}

pub fn step(d: &Dyn) -> Option<Dyn> {
    match d {
        Dyn::Var(_) | Dyn::Num(_) | Dyn::Bool(_) | Dyn::Lam(..) | Dyn::EmptyHole(..) => None,

        Dyn::Ap(fun, arg) => {
            if let Some(fun) = step(fun) {
                return Some(Dyn::Ap(Box::new(fun), arg.clone()));
            }
            if let Some(arg) = step(arg) {
                return Some(Dyn::Ap(fun.clone(), Box::new(arg)));
            }
            match fun.as_ref() {
                Dyn::Lam(id, _, body) => Some(subst(*id, arg, body)),
                _ => None,
            }
        }

        Dyn::BinOp(op, lhs, rhs) => {
            if let Some(lhs) = step(lhs) {
                return Some(Dyn::BinOp(*op, Box::new(lhs), rhs.clone()));
            }
            if let Some(rhs) = step(rhs) {
                return Some(Dyn::BinOp(*op, lhs.clone(), Box::new(rhs)));
            }
            match (lhs.as_ref(), rhs.as_ref()) {
                (Dyn::Num(a), Dyn::Num(b)) => apply_op(*op, *a, *b),
                _ => None,
            }
        }

        Dyn::If(cond, then, else_) => {
            if let Some(cond) = step(cond) {
                return Some(Dyn::If(Box::new(cond), then.clone(), else_.clone()));
            }
            match cond.as_ref() {
                Dyn::Bool(true) => Some(then.as_ref().clone()),
                Dyn::Bool(false) => Some(else_.as_ref().clone()),
                _ => None,
            }
        }

        Dyn::Let(id, bound, body) => {
            if let Some(bound) = step(bound) {
                return Some(Dyn::Let(*id, Box::new(bound), body.clone()));
            }
            Some(subst(*id, bound, body))
        }

        Dyn::Pair(fst, snd) => {
            if let Some(fst) = step(fst) {
                return Some(Dyn::Pair(Box::new(fst), snd.clone()));
            }
            step(snd).map(|snd| Dyn::Pair(fst.clone(), Box::new(snd)))
        }

        Dyn::Proj(side, inner) => {
            if let Some(inner) = step(inner) {
                return Some(Dyn::Proj(*side, Box::new(inner)));
            }
            match inner.as_ref() {
                Dyn::Pair(fst, snd) => Some(match side {
                    Side::L => fst.as_ref().clone(),
                    Side::R => snd.as_ref().clone(),
                }),
                _ => None,
            }
        }

        Dyn::NonEmptyHole(h, env, inner) => {
            step(inner).map(|inner| Dyn::NonEmptyHole(*h, env.clone(), Box::new(inner)))
        }
    }
}

fn apply_op(op: Op, a: i64, b: i64) -> Option<Dyn> {
    Some(match op {
        Op::Add => Dyn::Num(a.checked_add(b)?),
        Op::Sub => Dyn::Num(a.checked_sub(b)?),
        Op::Mul => Dyn::Num(a.checked_mul(b)?),
        Op::Lt => Dyn::Bool(a < b),
        Op::Eq => Dyn::Bool(a == b),
    })
}

pub fn eval(exp: &Exp) -> Outcome {
    eval_with_fuel(exp, DEFAULT_FUEL)
}

pub fn eval_with_fuel(exp: &Exp, fuel: usize) -> Outcome {
    run(elaborate(exp), fuel)
}

pub fn run(start: Dyn, fuel: usize) -> Outcome {
    let mut d = start;
    let mut steps = 0;
    while steps < fuel {
        match step(&d) {
            None => return settle(d),
            Some(next) => {
                d = next;
                steps += 1;
            }
        }
    }
    match step(&d) {
        None => settle(d),
        Some(_) => Outcome::OutOfFuel { partial: d, steps },
    }
}

fn settle(d: Dyn) -> Outcome {
    if is_value(&d) {
        return Outcome::Value(d);
    }
    let blocked = blocked_holes(&d);
    Outcome::Indeterminate { result: d, blocked }
}

pub fn blocked_holes(d: &Dyn) -> Vec<Blocked> {
    let mut out = Vec::new();
    collect(d, &mut out);
    out
}

fn collect(d: &Dyn, out: &mut Vec<Blocked>) {
    match d {
        Dyn::EmptyHole(h, env) => out.push(Blocked {
            hole: *h,
            kind: HoleKind::Empty,
            env: env.clone(),
        }),
        Dyn::NonEmptyHole(h, env, inner) => {
            out.push(Blocked {
                hole: *h,
                kind: HoleKind::NonEmpty,
                env: env.clone(),
            });
            collect(inner, out);
        }
        Dyn::Ap(a, b) | Dyn::BinOp(_, a, b) | Dyn::Pair(a, b) => {
            collect(a, out);
            collect(b, out);
        }
        Dyn::Let(_, bound, _) => collect(bound, out),
        Dyn::If(cond, _, _) => collect(cond, out),
        Dyn::Proj(_, inner) => collect(inner, out),

        Dyn::Var(_) | Dyn::Num(_) | Dyn::Bool(_) | Dyn::Lam(..) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamic::render;
    use nothing_core::examples;
    use nothing_core::names::NameTable;
    use nothing_core::ty::Ty;
    use nothing_core::typing::is_well_typed;

    fn x() -> Id {
        Id::from_u128(1)
    }

    fn y() -> Id {
        Id::from_u128(2)
    }

    fn h(n: u128) -> HoleId {
        HoleId::from_u128(n)
    }

    fn names() -> NameTable {
        let mut names = examples::names();
        names.set(x(), "x");
        names.set(y(), "y");
        names
    }

    #[test]
    fn literals_are_already_values() {
        assert_eq!(eval(&Exp::num(7)).num(), Some(7));
        assert_eq!(eval(&Exp::bool_(true)).bool(), Some(true));
    }

    #[test]
    fn arithmetic_and_comparison_reduce() {
        let e = Exp::bin_op(
            Op::Add,
            Exp::num(1),
            Exp::bin_op(Op::Mul, Exp::num(2), Exp::num(3)),
        );
        assert_eq!(eval(&e).num(), Some(7));
        assert_eq!(
            eval(&Exp::bin_op(Op::Lt, Exp::num(1), Exp::num(2))).bool(),
            Some(true)
        );
        assert_eq!(
            eval(&Exp::bin_op(Op::Eq, Exp::num(2), Exp::num(2))).bool(),
            Some(true)
        );
        assert_eq!(
            eval(&Exp::bin_op(Op::Sub, Exp::num(2), Exp::num(5))).num(),
            Some(-3)
        );
    }

    #[test]
    fn an_overflowing_operation_gets_stuck_rather_than_panicking() {
        let e = Exp::bin_op(Op::Mul, Exp::num(i64::MAX), Exp::num(2));
        let outcome = eval(&e);
        assert!(outcome.is_indeterminate());
        assert!(outcome.is_stuck(), "no hole is to blame: {outcome:?}");
    }

    #[test]
    fn a_conditional_evaluates_only_the_branch_it_takes() {
        let e = Exp::if_(
            Exp::bin_op(Op::Lt, Exp::num(1), Exp::num(2)),
            Exp::num(10),
            Exp::empty_hole(h(0)),
        );
        assert_eq!(eval(&e).num(), Some(10), "the hole is never needed");
    }

    #[test]
    fn application_is_call_by_value_and_left_to_right() {
        let e = examples::increment_applied();
        assert_eq!(eval(&e).num(), Some(42));
    }

    #[test]
    fn let_binds_then_runs_the_body() {
        assert_eq!(eval(&examples::let_identity()).num(), Some(1));
        assert_eq!(eval(&examples::square_and_compare()).bool(), Some(true));
    }

    #[test]
    fn pairs_and_projections_reduce() {
        assert_eq!(eval(&examples::pair_and_project()).num(), Some(1));
        let e = Exp::proj(Side::R, Exp::pair(Exp::num(1), Exp::num(2)));
        assert_eq!(eval(&e).num(), Some(2));
    }

    #[test]
    fn a_lambda_is_a_value_and_evaluation_does_not_enter_it() {
        let e = Exp::lam(x(), Ty::Num, Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2)));
        let outcome = eval(&e);
        assert!(outcome.is_value());
        assert_eq!(render(outcome.dyn_result(), &names()), "λx:Num. 1 + 2");
    }

    #[test]
    fn shadowed_binders_with_the_same_display_name_evaluate_by_identity() {
        let mut table = NameTable::new();
        table.set(x(), "x");
        table.set(y(), "x");

        let e = Exp::ap(
            Exp::ap(
                Exp::lam(
                    x(),
                    Ty::Num,
                    Exp::lam(
                        y(),
                        Ty::Num,
                        Exp::bin_op(Op::Sub, Exp::var(x()), Exp::var(y())),
                    ),
                ),
                Exp::num(10),
            ),
            Exp::num(4),
        );
        assert_eq!(
            nothing_core::render::render(&e, &table),
            "(λx:Num. λx:Num. x - x) 10 4"
        );
        assert_eq!(
            eval(&e).num(),
            Some(6),
            "identity, not display name, decides"
        );
    }

    #[test]
    fn one_plus_a_hole_reports_the_hole_instead_of_panicking() {
        let e = examples::add_with_empty_hole();
        assert!(is_well_typed(&e));
        let outcome = eval(&e);
        assert!(outcome.is_indeterminate(), "{outcome:?}");
        assert_eq!(render(outcome.dyn_result(), &names()), "1 + ⦇⦈");
        assert_eq!(outcome.blocked().len(), 1);
        assert_eq!(outcome.blocked()[0].hole, examples::hole(0));
        assert_eq!(outcome.blocked()[0].kind, HoleKind::Empty);
    }

    #[test]
    fn a_quarantined_expression_blocks_too_and_keeps_its_contents() {
        let outcome = eval(&examples::add_with_non_empty_hole());
        assert!(outcome.is_indeterminate());
        assert_eq!(render(outcome.dyn_result(), &names()), "1 + ⦇true⦈");
        assert_eq!(outcome.blocked().len(), 1);
        assert_eq!(outcome.blocked()[0].kind, HoleKind::NonEmpty);
    }

    #[test]
    fn evaluation_continues_around_a_hole_it_does_not_need() {
        let e = Exp::pair(
            Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2)),
            Exp::bin_op(Op::Mul, Exp::num(2), Exp::empty_hole(h(0))),
        );
        let outcome = eval(&e);
        assert!(outcome.is_indeterminate());
        assert_eq!(
            render(outcome.dyn_result(), &names()),
            "(3, 2 * ⦇⦈)",
            "the half that could run, ran"
        );
    }

    #[test]
    fn a_hole_inside_a_lambda_applied_to_five_carries_the_binding() {
        let e = Exp::ap(Exp::lam(x(), Ty::Num, Exp::empty_hole(h(0))), Exp::num(5));
        assert!(is_well_typed(&e));
        let outcome = eval(&e);
        assert!(outcome.is_indeterminate(), "{outcome:?}");

        let blocked = &outcome.blocked()[0];
        assert_eq!(blocked.hole, h(0));
        assert_eq!(
            blocked.known(),
            vec![(x(), Dyn::Num(5))],
            "the environment at the point of blocking"
        );
        assert_eq!(
            blocked
                .known()
                .iter()
                .map(|(id, d)| format!("{} = {}", names().display(*id), render(d, &names())))
                .collect::<Vec<_>>(),
            vec!["x = 5".to_string()]
        );
    }

    #[test]
    fn every_binder_in_scope_at_the_hole_is_captured() {
        let e = Exp::ap(
            Exp::ap(
                Exp::lam(x(), Ty::Num, Exp::lam(y(), Ty::Bool, Exp::empty_hole(h(0)))),
                Exp::num(5),
            ),
            Exp::bool_(true),
        );
        let outcome = eval(&e);
        let blocked = &outcome.blocked()[0];
        assert_eq!(
            blocked.known(),
            vec![(x(), Dyn::Num(5)), (y(), Dyn::Bool(true))]
        );
    }

    #[test]
    fn an_unapplied_lambda_keeps_its_holes_environment_unresolved() {
        let e = Exp::lam(x(), Ty::Num, Exp::empty_hole(h(0)));
        let outcome = eval(&e);

        assert!(outcome.is_value(), "an unapplied lambda is a value");

        let inner = match outcome.dyn_result() {
            Dyn::Lam(_, _, body) => (**body).clone(),
            other => panic!("expected a lambda, got {other:?}"),
        };
        match inner {
            Dyn::EmptyHole(_, env) => {
                assert_eq!(env.get(&x()), Some(&Dyn::Var(x())));
            }
            other => panic!("expected a hole, got {other:?}"),
        }
    }

    #[test]
    fn a_let_bound_hole_flows_into_every_use() {
        let e = Exp::let_(
            x(),
            Exp::empty_hole(h(0)),
            Exp::bin_op(Op::Add, Exp::var(x()), Exp::num(1)),
        );
        let outcome = eval(&e);
        assert_eq!(render(outcome.dyn_result(), &names()), "⦇⦈ + 1");
        assert_eq!(outcome.blocked()[0].hole, h(0));
    }

    #[test]
    fn a_hole_in_the_scrutinee_leaves_both_branches_unevaluated() {
        let e = Exp::if_(
            Exp::empty_hole(h(0)),
            Exp::bin_op(Op::Add, Exp::num(1), Exp::num(1)),
            Exp::num(0),
        );
        let outcome = eval(&e);
        assert_eq!(
            render(outcome.dyn_result(), &names()),
            "if ⦇⦈ then 1 + 1 else 0"
        );
        assert_eq!(
            outcome.blocked().len(),
            1,
            "only the scrutinee's hole is in the way"
        );
    }

    #[test]
    fn a_runaway_program_runs_out_of_fuel_rather_than_hanging() {
        let omega = Exp::lam(x(), Ty::Hole, Exp::ap(Exp::var(x()), Exp::var(x())));
        let e = Exp::ap(omega.clone(), omega);
        assert!(is_well_typed(&e));

        let outcome = eval_with_fuel(&e, 100);
        assert!(outcome.is_out_of_fuel(), "{outcome:?}");
        assert!(!outcome.is_value());
        assert!(!outcome.is_indeterminate());
        match outcome {
            Outcome::OutOfFuel { steps, .. } => assert_eq!(steps, 100),
            other => panic!("expected exhaustion, got {other:?}"),
        }
    }

    #[test]
    fn fuel_is_not_spent_on_a_program_that_finishes() {
        let outcome = eval_with_fuel(&examples::increment_applied(), 100);
        assert_eq!(outcome.num(), Some(42));
    }

    #[test]
    fn every_step_of_a_run_is_one_small_step() {
        let e = Exp::bin_op(
            Op::Add,
            Exp::bin_op(Op::Mul, Exp::num(2), Exp::num(3)),
            Exp::num(4),
        );
        let mut d = elaborate(&e);
        let mut seen = vec![render(&d, &names())];
        while let Some(next) = step(&d) {
            d = next;
            seen.push(render(&d, &names()));
        }
        assert_eq!(seen, vec!["2 * 3 + 4", "6 + 4", "10"]);
    }

    #[test]
    fn a_stuck_application_of_a_number_is_indeterminate_not_a_panic() {
        let f = Id::from_u128(9);
        let e = Exp::ap(
            Exp::lam(f, Ty::Hole, Exp::ap(Exp::var(f), Exp::num(1))),
            Exp::num(2),
        );
        assert!(is_well_typed(&e), "gradual typing lets this through");
        let outcome = eval(&e);
        assert!(outcome.is_indeterminate());
        assert!(outcome.is_stuck());
        assert_eq!(render(outcome.dyn_result(), &names()), "2 1");
    }
}
