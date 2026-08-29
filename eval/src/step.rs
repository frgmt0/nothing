use nothing_core::doc::Doc;
use nothing_core::exp::{Exp, HoleId, Id, Op, Side};
use nothing_core::stack::on_deep_stack;

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
    on_deep_stack(|| step_walk(defs, d))
}

fn step_walk(defs: &Defs, d: &Dyn) -> Option<Dyn> {
    let mut parents: Vec<&Dyn> = Vec::new();
    let mut cur = d;
    loop {
        match cur {
            Dyn::Ap(fun, arg) => {
                if let Some(stepped) = step_walk(defs, fun) {
                    let node = Dyn::Ap(Box::new(stepped), arg.clone());
                    return Some(rebuild(node, &parents));
                }
                parents.push(cur);
                cur = arg;
            }
            Dyn::BinOp(op, lhs, rhs) => {
                if let Some(stepped) = step_walk(defs, lhs) {
                    let node = Dyn::BinOp(*op, Box::new(stepped), rhs.clone());
                    return Some(rebuild(node, &parents));
                }
                parents.push(cur);
                cur = rhs;
            }
            Dyn::Pair(fst, snd) => {
                if let Some(stepped) = step_walk(defs, fst) {
                    let node = Dyn::Pair(Box::new(stepped), snd.clone());
                    return Some(rebuild(node, &parents));
                }
                parents.push(cur);
                cur = snd;
            }
            Dyn::Cons(head, tail) => {
                if let Some(stepped) = step_walk(defs, head) {
                    let node = Dyn::Cons(Box::new(stepped), tail.clone());
                    return Some(rebuild(node, &parents));
                }
                parents.push(cur);
                cur = tail;
            }
            Dyn::Let(_, bound, _) => {
                parents.push(cur);
                cur = bound;
            }
            Dyn::CmdBind(command, _, _) => {
                parents.push(cur);
                cur = command;
            }
            Dyn::If(cond, _, _) => {
                parents.push(cur);
                cur = cond;
            }
            Dyn::Match(scrutinee, _) => {
                parents.push(cur);
                cur = scrutinee;
            }
            Dyn::Fold(list, _, _) => {
                parents.push(cur);
                cur = list;
            }
            Dyn::Proj(_, inner)
            | Dyn::Field(inner, _)
            | Dyn::Inj(_, inner)
            | Dyn::Print(inner)
            | Dyn::CmdPure(inner)
            | Dyn::NonEmptyHole(_, _, inner) => {
                parents.push(cur);
                cur = inner;
            }
            Dyn::Var(_)
            | Dyn::Num(_)
            | Dyn::Bool(_)
            | Dyn::Str(_)
            | Dyn::Lam(..)
            | Dyn::Nil
            | Dyn::Readline
            | Dyn::EmptyHole(..)
            | Dyn::Record(_) => break,
        }
    }

    if let Some(node) = step_leaf(defs, cur) {
        return Some(rebuild(node, &parents));
    }

    while let Some(parent) = parents.pop() {
        if let Some(node) = reduce_here(defs, parent) {
            return Some(rebuild(node, &parents));
        }
    }
    None
}

fn step_leaf(defs: &Defs, d: &Dyn) -> Option<Dyn> {
    match d {
        Dyn::Var(id) => defs.get(id).cloned(),
        Dyn::Record(fields) => {
            for (index, (_, value)) in fields.iter().enumerate() {
                if let Some(value) = step_walk(defs, value) {
                    let mut next = fields.to_vec();
                    next[index].1 = value;
                    return Some(Dyn::Record(next));
                }
            }
            None
        }
        _ => None,
    }
}

fn reduce_here(defs: &Defs, parent: &Dyn) -> Option<Dyn> {
    match parent {
        Dyn::Ap(fun, arg) => match fun.as_ref() {
            Dyn::Lam(id, _, body) => Some(subst(*id, arg, body)),
            _ => None,
        },
        Dyn::BinOp(op, lhs, rhs) => match (lhs.as_ref(), rhs.as_ref()) {
            (Dyn::Num(a), Dyn::Num(b)) => apply_num_op(*op, *a, *b),
            (Dyn::Str(a), Dyn::Str(b)) => apply_str_op(*op, a, b),
            (Dyn::Bool(a), Dyn::Bool(b)) => apply_bool_op(*op, *a, *b),
            _ => None,
        },
        Dyn::Let(id, bound, body) => Some(subst(*id, bound, body)),
        Dyn::If(cond, then, else_) => match cond.as_ref() {
            Dyn::Bool(true) => Some(then.as_ref().clone()),
            Dyn::Bool(false) => Some(else_.as_ref().clone()),
            _ => None,
        },
        Dyn::Match(scrutinee, arms) => match scrutinee.as_ref() {
            Dyn::Inj(ctor, payload) => arms
                .iter()
                .find(|(id, _, _)| id == ctor)
                .map(|(_, binder, body)| subst(*binder, payload, body)),
            _ => None,
        },
        Dyn::Proj(side, inner) => match inner.as_ref() {
            Dyn::Pair(fst, snd) => Some(match side {
                Side::L => fst.as_ref().clone(),
                Side::R => snd.as_ref().clone(),
            }),
            _ => None,
        },
        Dyn::Field(subject, field) => match subject.as_ref() {
            Dyn::Record(fields) => fields
                .iter()
                .find(|(id, _)| id == field)
                .map(|(_, value)| value.clone()),
            _ => None,
        },
        Dyn::Fold(list, init, folder) => reduce_fold(defs, list, init, folder),
        Dyn::Pair(..)
        | Dyn::Cons(..)
        | Dyn::Inj(..)
        | Dyn::Print(_)
        | Dyn::CmdPure(_)
        | Dyn::CmdBind(..)
        | Dyn::NonEmptyHole(..) => None,
        _ => None,
    }
}

fn reduce_fold(defs: &Defs, list: &Dyn, init: &Dyn, folder: &Dyn) -> Option<Dyn> {
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
            if let Some(init) = step_walk(defs, init) {
                return Some(Dyn::Fold(
                    Box::new(list.clone()),
                    Box::new(init),
                    Box::new(folder.clone()),
                ));
            }
            step_walk(defs, folder).map(|folder| {
                Dyn::Fold(
                    Box::new(list.clone()),
                    Box::new(init.clone()),
                    Box::new(folder),
                )
            })
        }
    }
}

fn rebuild(mut node: Dyn, parents: &[&Dyn]) -> Dyn {
    for parent in parents.iter().rev() {
        node = rebuild_one(parent, node);
    }
    node
}

fn rebuild_one(parent: &Dyn, child: Dyn) -> Dyn {
    match parent {
        Dyn::Ap(fun, _) => Dyn::Ap(fun.clone(), Box::new(child)),
        Dyn::BinOp(op, lhs, _) => Dyn::BinOp(*op, lhs.clone(), Box::new(child)),
        Dyn::Pair(fst, _) => Dyn::Pair(fst.clone(), Box::new(child)),
        Dyn::Cons(head, _) => Dyn::Cons(head.clone(), Box::new(child)),
        Dyn::Let(id, _, body) => Dyn::Let(*id, Box::new(child), body.clone()),
        Dyn::CmdBind(_, id, body) => Dyn::CmdBind(Box::new(child), *id, body.clone()),
        Dyn::If(_, then, else_) => Dyn::If(Box::new(child), then.clone(), else_.clone()),
        Dyn::Match(_, arms) => Dyn::Match(Box::new(child), arms.clone()),
        Dyn::Fold(_, init, folder) => Dyn::Fold(Box::new(child), init.clone(), folder.clone()),
        Dyn::Proj(side, _) => Dyn::Proj(*side, Box::new(child)),
        Dyn::Field(_, id) => Dyn::Field(Box::new(child), *id),
        Dyn::Inj(ctor, _) => Dyn::Inj(*ctor, Box::new(child)),
        Dyn::Print(_) => Dyn::Print(Box::new(child)),
        Dyn::CmdPure(_) => Dyn::CmdPure(Box::new(child)),
        Dyn::NonEmptyHole(h, env, _) => Dyn::NonEmptyHole(*h, env.clone(), Box::new(child)),
        other => other.clone(),
    }
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
    on_deep_stack(|| run(elaborate(exp), fuel))
}

pub fn eval_doc(doc: &Doc, main: Id) -> Outcome {
    eval_doc_with_fuel(doc, main, DEFAULT_FUEL)
}

pub fn eval_doc_with_fuel(doc: &Doc, main: Id, fuel: usize) -> Outcome {
    on_deep_stack(|| {
        let defs = defs_of(doc);
        let start = match doc.get(main) {
            Some(def) => elaborate(&def.body),
            None => Dyn::Var(main),
        };
        run_in(&defs, start, fuel)
    })
}

pub fn run(start: Dyn, fuel: usize) -> Outcome {
    run_in(&Defs::new(), start, fuel)
}

pub fn run_in(defs: &Defs, start: Dyn, fuel: usize) -> Outcome {
    run_in_counted(defs, start, fuel).0
}

pub fn run_in_counted(defs: &Defs, start: Dyn, fuel: usize) -> (Outcome, usize) {
    on_deep_stack(|| run_counted(defs, start, fuel))
}

pub(crate) fn run_counted(defs: &Defs, start: Dyn, fuel: usize) -> (Outcome, usize) {
    let mut d = start;
    let mut steps = 0;
    while steps < fuel {
        match step_walk(defs, &d) {
            None => return (settle(d), steps),
            Some(next) => {
                d = next;
                steps += 1;
            }
        }
    }
    match step_walk(defs, &d) {
        None => (settle(d), steps),
        Some(_) => (Outcome::OutOfFuel { partial: d, steps }, steps),
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
    let mut pending = vec![d];
    while let Some(cur) = pending.pop() {
        match cur {
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
                pending.push(inner);
            }
            Dyn::Ap(a, b) | Dyn::BinOp(_, a, b) | Dyn::Pair(a, b) | Dyn::Cons(a, b) => {
                pending.push(b);
                pending.push(a);
            }
            Dyn::Let(_, bound, _) => pending.push(bound),
            Dyn::If(cond, _, _) => pending.push(cond),
            Dyn::Fold(list, init, folder) => {
                pending.push(folder);
                pending.push(init);
                pending.push(list);
            }
            Dyn::Proj(_, inner)
            | Dyn::Field(inner, _)
            | Dyn::Inj(_, inner)
            | Dyn::Print(inner)
            | Dyn::CmdPure(inner) => pending.push(inner),
            Dyn::CmdBind(command, _, _) => pending.push(command),
            Dyn::Match(scrutinee, _) => pending.push(scrutinee),
            Dyn::Record(fields) => {
                for (_, value) in fields.iter().rev() {
                    pending.push(value);
                }
            }

            Dyn::Var(_)
            | Dyn::Num(_)
            | Dyn::Bool(_)
            | Dyn::Str(_)
            | Dyn::Nil
            | Dyn::Readline
            | Dyn::Lam(..) => {}
        }
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

    fn red() -> Id {
        Id::from_u128(31)
    }

    fn green() -> Id {
        Id::from_u128(32)
    }

    #[test]
    fn a_match_reduces_by_constructor_and_binds_the_payload() {
        let e = Exp::match_(
            Exp::inj(green(), Exp::num(4)),
            [
                (
                    red(),
                    x(),
                    Exp::bin_op(Op::Mul, Exp::var(x()), Exp::num(10)),
                ),
                (
                    green(),
                    y(),
                    Exp::bin_op(Op::Add, Exp::var(y()), Exp::num(1)),
                ),
            ],
        );
        assert!(is_well_typed(&e));
        assert_eq!(
            eval(&e).num(),
            Some(5),
            "the taken arm is the one that runs"
        );
    }

    #[test]
    fn a_hole_in_an_arm_that_is_not_taken_does_not_block() {
        let e = Exp::match_(
            Exp::inj(green(), Exp::num(4)),
            [
                (red(), x(), Exp::empty_hole(h(7))),
                (green(), y(), Exp::var(y())),
            ],
        );
        assert!(is_well_typed(&e));
        let out = eval(&e);
        assert_eq!(out.num(), Some(4));
        assert!(
            out.blocked().is_empty(),
            "an arm nothing takes is not a reason to stop"
        );
    }

    #[test]
    fn a_match_on_an_indeterminate_scrutinee_is_indeterminate() {
        let e = Exp::match_(
            Exp::empty_hole(h(1)),
            [(red(), x(), Exp::var(x())), (green(), y(), Exp::var(y()))],
        );
        assert!(is_well_typed(&e));
        let out = eval(&e);
        assert!(out.is_indeterminate(), "{out:?}");
        assert_eq!(
            out.blocked().len(),
            1,
            "the scrutinee's hole is the one thing standing in the way"
        );
        assert_eq!(out.blocked()[0].hole, h(1));
        assert_eq!(
            render(out.dyn_result(), &names()),
            render(&elaborate(&e), &names()),
            "and the residual is the match itself, arms and all"
        );
    }

    #[test]
    fn an_injection_of_a_value_is_a_value_and_its_payload_reduces_first() {
        let e = Exp::inj(red(), Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2)));
        let out = eval(&e);
        assert!(out.is_value(), "{out:?}");
        assert_eq!(out.to_exp(), Exp::inj(red(), Exp::num(3)));

        let stuck = Exp::inj(red(), Exp::empty_hole(h(3)));
        assert!(!eval(&stuck).is_value());
    }

    #[test]
    fn a_match_with_no_arm_for_the_case_it_gets_is_stuck_rather_than_wrong() {
        let d = elaborate(&Exp::match_(
            Exp::inj(green(), Exp::num(1)),
            [(red(), x(), Exp::var(x()))],
        ));
        assert_eq!(
            step(&d),
            None,
            "there is no arm to take and no answer to give"
        );
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
    fn a_finished_command_is_a_value_and_pure_evaluation_does_not_perform_it() {
        let program = Exp::cmd_bind(
            Exp::print(Exp::bin_op(
                Op::Concat,
                Exp::str_("hello, "),
                Exp::str_("world"),
            )),
            x(),
            Exp::cmd_bind(Exp::readline(), y(), Exp::print(Exp::var(y()))),
        );
        assert!(is_well_typed(&program));

        let outcome = eval(&program);
        assert!(
            outcome.is_value(),
            "a bind chain with nothing left to compute is a value: {:?}",
            outcome
        );
        assert_eq!(
            render(outcome.dyn_result(), &names()),
            "bind x <- print \"hello, world\" in bind y <- readline in print y",
            "the only reduction that happened is inside print's argument — the command \
             itself was not performed"
        );
        assert!(crate::dynamic::is_value(outcome.dyn_result()));
    }

    #[test]
    fn a_command_whose_argument_is_blocked_is_indeterminate_rather_than_a_value() {
        let program = Exp::cmd_bind(
            Exp::print(Exp::empty_hole(h(4))),
            x(),
            Exp::cmd_pure(Exp::num(1)),
        );
        let outcome = eval(&program);
        assert!(outcome.is_indeterminate());
        assert_eq!(outcome.blocked().len(), 1);
        assert_eq!(outcome.blocked()[0].hole, h(4));

        let unwritten = Exp::cmd_bind(Exp::empty_hole(h(5)), x(), Exp::cmd_pure(Exp::num(1)));
        let outcome = eval(&unwritten);
        assert!(
            outcome.is_indeterminate(),
            "a bind whose command is a hole is not a finished command"
        );
        assert_eq!(outcome.blocked()[0].hole, h(5));
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
