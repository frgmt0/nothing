use nothing_core::doc::Doc;
use nothing_core::exp::{Exp, HoleId, Id, Op, Side};

use crate::dynamic::{Dyn, Env, elaborate, is_value, subst};

pub type Defs = im::HashMap<Id, Dyn>;

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

    pub fn str(&self) -> Option<&str> {
        match self {
            Outcome::Value(Dyn::Str(text)) => Some(text),
            _ => None,
        }
    }

    pub fn to_exp(&self) -> Exp {
        crate::dynamic::to_exp(self.dyn_result())
    }
}

pub fn step(d: &Dyn) -> Option<Dyn> {
    step_in(&Defs::new(), d)
}

pub fn step_in(defs: &Defs, d: &Dyn) -> Option<Dyn> {
    let step = |d: &Dyn| step_in(defs, d);
    match d {
        Dyn::Var(id) => defs.get(id).cloned(),

        Dyn::Num(_) | Dyn::Bool(_) | Dyn::Str(_) | Dyn::Lam(..) | Dyn::Nil | Dyn::EmptyHole(..) => {
            None
        }

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
                (Dyn::Num(a), Dyn::Num(b)) => apply_num_op(*op, *a, *b),
                (Dyn::Str(a), Dyn::Str(b)) => apply_str_op(*op, a, b),
                (Dyn::Bool(a), Dyn::Bool(b)) => apply_bool_op(*op, *a, *b),
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

        Dyn::Cons(head, tail) => {
            if let Some(head) = step(head) {
                return Some(Dyn::Cons(Box::new(head), tail.clone()));
            }
            step(tail).map(|tail| Dyn::Cons(head.clone(), Box::new(tail)))
        }

        Dyn::Fold(list, init, folder) => step_fold(defs, list, init, folder),

        Dyn::Record(fields) => step_record(defs, fields),

        Dyn::Field(subject, id) => step_field(defs, subject, *id),

        Dyn::NonEmptyHole(h, env, inner) => {
            step(inner).map(|inner| Dyn::NonEmptyHole(*h, env.clone(), Box::new(inner)))
        }
    }
}

fn step_fold(defs: &Defs, list: &Dyn, init: &Dyn, folder: &Dyn) -> Option<Dyn> {
    if let Some(list) = step_in(defs, list) {
        return Some(Dyn::Fold(
            Box::new(list),
            Box::new(init.clone()),
            Box::new(folder.clone()),
        ));
    }
    match list {
        Dyn::Nil => Some(init.clone()),
        Dyn::Cons(head, tail) => Some(Dyn::Ap(
            Box::new(Dyn::Ap(Box::new(folder.clone()), head.clone())),
            Box::new(Dyn::Fold(
                tail.clone(),
                Box::new(init.clone()),
                Box::new(folder.clone()),
            )),
        )),
        _ => {
            if let Some(init) = step_in(defs, init) {
                return Some(Dyn::Fold(
                    Box::new(list.clone()),
                    Box::new(init),
                    Box::new(folder.clone()),
                ));
            }
            step_in(defs, folder).map(|folder| {
                Dyn::Fold(
                    Box::new(list.clone()),
                    Box::new(init.clone()),
                    Box::new(folder),
                )
            })
        }
    }
}

fn step_field(defs: &Defs, subject: &Dyn, field: Id) -> Option<Dyn> {
    if let Some(stepped) = step_in(defs, subject) {
        return Some(Dyn::Field(Box::new(stepped), field));
    }
    match subject {
        Dyn::Record(fields) => fields
            .iter()
            .find(|(id, _)| *id == field)
            .map(|(_, value)| value.clone()),
        _ => None,
    }
}

fn step_record(defs: &Defs, fields: &[(Id, Dyn)]) -> Option<Dyn> {
    for (index, (_, value)) in fields.iter().enumerate() {
        if let Some(value) = step_in(defs, value) {
            let mut next = fields.to_vec();
            next[index].1 = value;
            return Some(Dyn::Record(next));
        }
    }
    None
}

fn apply_num_op(op: Op, a: i64, b: i64) -> Option<Dyn> {
    Some(match op {
        Op::Add => Dyn::Num(a.checked_add(b)?),
        Op::Sub => Dyn::Num(a.checked_sub(b)?),
        Op::Mul => Dyn::Num(a.checked_mul(b)?),
        Op::Lt => Dyn::Bool(a < b),
        Op::Eq => Dyn::Bool(a == b),
        Op::Concat => return None,
    })
}

fn apply_str_op(op: Op, a: &str, b: &str) -> Option<Dyn> {
    match op {
        Op::Concat => Some(Dyn::Str(format!("{a}{b}"))),
        Op::Eq => Some(Dyn::Bool(a == b)),
        Op::Add | Op::Sub | Op::Mul | Op::Lt => None,
    }
}

fn apply_bool_op(op: Op, a: bool, b: bool) -> Option<Dyn> {
    match op {
        Op::Eq => Some(Dyn::Bool(a == b)),
        Op::Add | Op::Sub | Op::Mul | Op::Lt | Op::Concat => None,
    }
}

pub fn defs_of(doc: &Doc) -> Defs {
    doc.defs()
        .iter()
        .map(|def| (def.id, elaborate(&def.body)))
        .collect()
}

pub fn eval(exp: &Exp) -> Outcome {
    eval_with_fuel(exp, DEFAULT_FUEL)
}

pub fn eval_with_fuel(exp: &Exp, fuel: usize) -> Outcome {
    run(elaborate(exp), fuel)
}

pub fn eval_doc(doc: &Doc, main: Id) -> Outcome {
    eval_doc_with_fuel(doc, main, DEFAULT_FUEL)
}

pub fn eval_doc_with_fuel(doc: &Doc, main: Id, fuel: usize) -> Outcome {
    let defs = defs_of(doc);
    let start = match doc.get(main) {
        Some(def) => elaborate(&def.body),
        None => Dyn::Var(main),
    };
    run_in(&defs, start, fuel)
}

pub fn run(start: Dyn, fuel: usize) -> Outcome {
    run_in(&Defs::new(), start, fuel)
}

pub fn run_in(defs: &Defs, start: Dyn, fuel: usize) -> Outcome {
    let mut d = start;
    let mut steps = 0;
    while steps < fuel {
        match step_in(defs, &d) {
            None => return settle(d),
            Some(next) => {
                d = next;
                steps += 1;
            }
        }
    }
    match step_in(defs, &d) {
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
        Dyn::Ap(a, b) | Dyn::BinOp(_, a, b) | Dyn::Pair(a, b) | Dyn::Cons(a, b) => {
            collect(a, out);
            collect(b, out);
        }
        Dyn::Let(_, bound, _) => collect(bound, out),
        Dyn::If(cond, _, _) => collect(cond, out),
        Dyn::Fold(list, init, folder) => {
            collect(list, out);
            collect(init, out);
            collect(folder, out);
        }
        Dyn::Proj(_, inner) | Dyn::Field(inner, _) => collect(inner, out),
        Dyn::Record(fields) => {
            for (_, value) in fields {
                collect(value, out);
            }
        }

        Dyn::Var(_) | Dyn::Num(_) | Dyn::Bool(_) | Dyn::Str(_) | Dyn::Nil | Dyn::Lam(..) => {}
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
        assert_eq!(eval(&Exp::str_("hi")).str(), Some("hi"));
    }

    #[test]
    fn concatenation_and_string_equality_reduce() {
        let e = Exp::bin_op(
            Op::Concat,
            Exp::str_("hello, "),
            Exp::bin_op(Op::Concat, Exp::str_("wor"), Exp::str_("ld")),
        );
        assert!(is_well_typed(&e));
        assert_eq!(eval(&e).str(), Some("hello, world"));

        assert_eq!(
            eval(&Exp::bin_op(Op::Eq, Exp::str_("a"), Exp::str_("a"))).bool(),
            Some(true)
        );
        assert_eq!(
            eval(&Exp::bin_op(Op::Eq, Exp::str_("a"), Exp::str_("b"))).bool(),
            Some(false)
        );
        assert_eq!(
            eval(&Exp::bin_op(Op::Eq, Exp::bool_(true), Exp::bool_(true))).bool(),
            Some(true)
        );
    }

    #[test]
    fn a_string_around_a_hole_stops_at_the_hole() {
        let e = Exp::bin_op(Op::Concat, Exp::str_("hi "), Exp::empty_hole(h(1)));
        assert!(is_well_typed(&e));
        let outcome = eval(&e);
        assert!(outcome.is_indeterminate());
        assert_eq!(outcome.blocked().len(), 1);
        assert_eq!(render(outcome.dyn_result(), &names()), "\"hi \" ++ ⦇⦈");
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
    fn a_list_with_a_hole_in_its_tail_is_an_indeterminate_list_that_reports_the_hole() {
        let e = Exp::cons(Exp::num(1), Exp::cons(Exp::num(2), Exp::empty_hole(h(0))));
        assert!(is_well_typed(&e));
        let outcome = eval(&e);
        assert!(outcome.is_indeterminate(), "{outcome:?}");
        assert_eq!(render(outcome.dyn_result(), &names()), "1 :: 2 :: ⦇⦈");
        assert_eq!(outcome.blocked().len(), 1);
        assert_eq!(outcome.blocked()[0].hole, h(0));
        assert_eq!(outcome.blocked()[0].kind, HoleKind::Empty);
    }

    #[test]
    fn a_finished_list_is_a_value_and_a_fold_over_it_is_a_number() {
        let xs = Exp::list([Exp::num(1), Exp::num(2), Exp::num(3)]);
        assert!(eval(&xs).is_value(), "a list of literals is a value");
        assert_eq!(
            render(eval(&xs).dyn_result(), &names()),
            "1 :: 2 :: 3 :: nil"
        );

        let sum = Exp::fold(
            xs.clone(),
            Exp::num(0),
            Exp::lam(
                x(),
                Ty::Num,
                Exp::lam(
                    y(),
                    Ty::Num,
                    Exp::bin_op(Op::Add, Exp::var(x()), Exp::var(y())),
                ),
            ),
        );
        assert!(is_well_typed(&sum));
        assert_eq!(eval(&sum).num(), Some(6));

        let empty = Exp::fold(
            Exp::nil(),
            Exp::num(41),
            Exp::lam(
                x(),
                Ty::Num,
                Exp::lam(
                    y(),
                    Ty::Num,
                    Exp::bin_op(Op::Add, Exp::var(x()), Exp::var(y())),
                ),
            ),
        );
        assert_eq!(eval(&empty).num(), Some(41), "folding nothing is the seed");
    }

    #[test]
    fn a_fold_runs_until_it_reaches_the_hole_in_the_list() {
        let xs = Exp::cons(Exp::num(1), Exp::cons(Exp::num(2), Exp::empty_hole(h(3))));
        let sum = Exp::fold(
            xs,
            Exp::num(0),
            Exp::lam(
                x(),
                Ty::Num,
                Exp::lam(
                    y(),
                    Ty::Num,
                    Exp::bin_op(Op::Add, Exp::var(x()), Exp::var(y())),
                ),
            ),
        );
        assert!(is_well_typed(&sum));
        let outcome = eval(&sum);
        assert!(outcome.is_indeterminate(), "{outcome:?}");
        assert_eq!(
            outcome.blocked().len(),
            1,
            "the fold stopped at the one hole it needed"
        );
        assert_eq!(outcome.blocked()[0].hole, h(3));
        assert!(
            render(outcome.dyn_result(), &names()).starts_with("1 + (2 + fold ⦇⦈ 0"),
            "the elements it did reach were folded in: {}",
            render(outcome.dyn_result(), &names())
        );
    }

    #[test]
    fn folding_a_long_enough_list_runs_out_of_fuel_rather_than_hanging() {
        let long = Exp::list((0..400).map(Exp::num));
        let sum = Exp::fold(
            long,
            Exp::num(0),
            Exp::lam(
                x(),
                Ty::Num,
                Exp::lam(
                    y(),
                    Ty::Num,
                    Exp::bin_op(Op::Add, Exp::var(x()), Exp::var(y())),
                ),
            ),
        );
        assert!(is_well_typed(&sum));
        assert!(
            eval_with_fuel(&sum, 100).is_out_of_fuel(),
            "fuel must still guard a fold over a long list"
        );
        assert_eq!(
            eval_with_fuel(&sum, 100_000).num(),
            Some((0..400).sum()),
            "with enough fuel it finishes"
        );
    }

    #[test]
    fn a_record_of_values_is_a_value_and_a_projection_picks_a_field_out_of_it() {
        let point = Exp::record([(x(), Exp::num(1)), (y(), Exp::num(2))]);
        assert!(is_well_typed(&point));
        let outcome = eval(&point);
        assert!(outcome.is_value());
        assert_eq!(render(outcome.dyn_result(), &names()), "{x = 1, y = 2}");

        assert_eq!(eval(&Exp::field(point.clone(), x())).num(), Some(1));
        assert_eq!(eval(&Exp::field(point, y())).num(), Some(2));
    }

    #[test]
    fn a_records_fields_reduce_where_they_stand() {
        let e = Exp::record([
            (x(), Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2))),
            (y(), Exp::bin_op(Op::Concat, Exp::str_("a"), Exp::str_("b"))),
        ]);
        assert!(is_well_typed(&e));
        assert_eq!(
            render(eval(&e).dyn_result(), &names()),
            "{x = 3, y = \"ab\"}"
        );
    }

    #[test]
    fn projecting_a_filled_field_of_a_half_written_record_still_answers() {
        let half = Exp::record([(x(), Exp::num(1)), (y(), Exp::empty_hole(h(0)))]);
        assert!(is_well_typed(&half));

        let filled = eval(&Exp::field(half.clone(), x()));
        assert_eq!(
            filled.num(),
            Some(1),
            "the hole in the other field was never needed"
        );

        let holed = eval(&Exp::field(half.clone(), y()));
        assert!(holed.is_indeterminate(), "{holed:?}");
        assert_eq!(holed.blocked().len(), 1);
        assert_eq!(holed.blocked()[0].hole, h(0));

        let whole = eval(&half);
        assert!(whole.is_indeterminate(), "a record with a hole is not done");
        assert_eq!(render(whole.dyn_result(), &names()), "{x = 1, y = ⦇⦈}");
    }

    #[test]
    fn projecting_a_field_of_something_that_is_not_a_record_gets_stuck_rather_than_panicking() {
        let e = Exp::field(Exp::empty_hole(h(0)), x());
        assert!(is_well_typed(&e));
        let outcome = eval(&e);
        assert!(outcome.is_indeterminate());
        assert_eq!(render(outcome.dyn_result(), &names()), "⦇⦈.x");
        assert_eq!(outcome.blocked().len(), 1);
    }

    #[test]
    fn a_record_flows_through_a_lambda_and_a_let() {
        let f = Id::from_u128(11);
        let e = Exp::let_(
            f,
            Exp::record([(x(), Exp::num(3)), (y(), Exp::num(4))]),
            Exp::bin_op(
                Op::Add,
                Exp::field(Exp::var(f), x()),
                Exp::field(Exp::var(f), y()),
            ),
        );
        assert!(is_well_typed(&e));
        assert_eq!(eval(&e).num(), Some(7));
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
