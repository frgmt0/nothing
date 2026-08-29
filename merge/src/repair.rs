use nothing_action::act::Fresh;
use nothing_core::ctx::Ctx;
use nothing_core::exp::{Exp, Op};
use nothing_core::names::NameTable;
use nothing_core::render::render;
use nothing_core::ty::{Ty, matched_arrow, matched_list, matched_prod};
use nothing_core::typing::{
    ana, is_comparable, is_well_typed_in, join, operand_expectation, step_ty, syn,
};

use crate::path::{Path, extend, label};

#[derive(Clone, PartialEq, Debug)]
pub enum RepairKind {
    Quarantined,
    Unbound,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Repair {
    pub kind: RepairKind,
    pub path: Path,
    pub subject: String,
    pub reason: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Repaired {
    pub exp: Exp,
    pub repairs: Vec<Repair>,
}

pub fn repair(exp: &Exp, names: &NameTable) -> Repaired {
    repair_in(&Ctx::empty(), exp, names)
}

pub fn repair_in(base: &Ctx, exp: &Exp, names: &NameTable) -> Repaired {
    if is_well_typed_in(base, exp) {
        return Repaired {
            exp: exp.clone(),
            repairs: Vec::new(),
        };
    }
    let mut state = State {
        fresh: Fresh::from_program(exp),
        names: names.clone(),
        root: exp.clone(),
        repairs: Vec::new(),
    };
    let rebuilt = state.go(base, exp, &[]);
    Repaired {
        exp: rebuilt,
        repairs: state.repairs,
    }
}

struct State {
    fresh: Fresh,
    names: NameTable,
    root: Exp,
    repairs: Vec<Repair>,
}

impl State {
    fn quarantine(&mut self, exp: Exp, path: &[usize], reason: &str) -> Exp {
        self.repairs.push(Repair {
            kind: RepairKind::Quarantined,
            path: path.to_vec(),
            subject: render(&exp, &self.names),
            reason: format!("{reason} at {}", label(&self.root, path)),
        });
        Exp::non_empty_hole(self.fresh.hole(), exp)
    }

    fn vacate(&mut self, exp: &Exp, path: &[usize], reason: &str) -> Exp {
        self.repairs.push(Repair {
            kind: RepairKind::Unbound,
            path: path.to_vec(),
            subject: render(exp, &self.names),
            reason: format!("{reason} at {}", label(&self.root, path)),
        });
        Exp::empty_hole(self.fresh.hole())
    }

    fn ensure_ana(&mut self, ctx: &Ctx, exp: Exp, ty: &Ty, path: &[usize], reason: &str) -> Exp {
        if ana(ctx, &exp, ty) {
            exp
        } else {
            self.quarantine(exp, path, reason)
        }
    }

    fn ensure_comparable(&mut self, ctx: &Ctx, exp: Exp, path: &[usize]) -> Exp {
        if syn(ctx, &exp).is_some_and(|ty| is_comparable(&ty)) {
            exp
        } else {
            self.quarantine(exp, path, "the merge left a value equality cannot compare")
        }
    }

    fn go(&mut self, ctx: &Ctx, exp: &Exp, path: &[usize]) -> Exp {
        match exp {
            Exp::Var(id) => {
                if ctx.lookup(id).is_some() {
                    exp.clone()
                } else {
                    self.vacate(exp, path, "the merge left this variable unbound")
                }
            }
            Exp::Num(_) | Exp::Bool(_) | Exp::Str(_) | Exp::Nil | Exp::EmptyHole(_) => exp.clone(),

            Exp::Lam(id, ann, body) => {
                let inner = ctx.extend(*id, ann.clone());
                let body = self.go(&inner, body, &extend(path, 0));
                Exp::Lam(*id, ann.clone(), Box::new(body))
            }

            Exp::Ap(fun, arg) => {
                let mut fun = self.go(ctx, fun, &extend(path, 0));
                let arg = self.go(ctx, arg, &extend(path, 1));
                let fun_ty = syn(ctx, &fun).unwrap_or(Ty::Hole);
                let (in_ty, _) = match matched_arrow(&fun_ty) {
                    Some(pair) => pair,
                    None => {
                        fun = self.quarantine(
                            fun,
                            &extend(path, 0),
                            "the merge left a non-function in function position",
                        );
                        (Ty::Hole, Ty::Hole)
                    }
                };
                let arg = self.ensure_ana(
                    ctx,
                    arg,
                    &in_ty,
                    &extend(path, 1),
                    "the merge left an argument that does not fit its parameter",
                );
                Exp::Ap(Box::new(fun), Box::new(arg))
            }

            Exp::BinOp(op, lhs, rhs) => {
                let lhs = self.go(ctx, lhs, &extend(path, 0));
                let rhs = self.go(ctx, rhs, &extend(path, 1));
                let (lhs, rhs) = match op {
                    Op::Eq => {
                        let lhs = self.ensure_comparable(ctx, lhs, &extend(path, 0));
                        let rhs = self.ensure_comparable(ctx, rhs, &extend(path, 1));
                        let operand = operand_expectation(ctx, Op::Eq, &lhs);
                        let rhs = self.ensure_ana(
                            ctx,
                            rhs,
                            &operand,
                            &extend(path, 1),
                            "the merge left an equality between two different types",
                        );
                        (lhs, rhs)
                    }
                    _ => {
                        let operand = operand_expectation(ctx, *op, &Exp::Bool(false));
                        let lhs = self.ensure_ana(
                            ctx,
                            lhs,
                            &operand,
                            &extend(path, 0),
                            "the merge left an operand this operator cannot take",
                        );
                        let rhs = self.ensure_ana(
                            ctx,
                            rhs,
                            &operand,
                            &extend(path, 1),
                            "the merge left an operand this operator cannot take",
                        );
                        (lhs, rhs)
                    }
                };
                Exp::BinOp(*op, Box::new(lhs), Box::new(rhs))
            }

            Exp::If(cond, then, else_) => {
                let cond = self.go(ctx, cond, &extend(path, 0));
                let then = self.go(ctx, then, &extend(path, 1));
                let else_ = self.go(ctx, else_, &extend(path, 2));
                let cond = self.ensure_ana(
                    ctx,
                    cond,
                    &Ty::Bool,
                    &extend(path, 0),
                    "the merge left a non-boolean condition",
                );
                let then_ty = syn(ctx, &then).unwrap_or(Ty::Hole);
                let else_ty = syn(ctx, &else_).unwrap_or(Ty::Hole);
                let else_ = if join(&then_ty, &else_ty).is_some() {
                    else_
                } else {
                    self.quarantine(
                        else_,
                        &extend(path, 2),
                        "the merge left branches with incompatible types",
                    )
                };
                Exp::If(Box::new(cond), Box::new(then), Box::new(else_))
            }

            Exp::Let(id, bound, body) => {
                let bound = self.go(ctx, bound, &extend(path, 0));
                let bound_ty = syn(ctx, &bound).unwrap_or(Ty::Hole);
                let inner = ctx.extend(*id, bound_ty);
                let body = self.go(&inner, body, &extend(path, 1));
                Exp::Let(*id, Box::new(bound), Box::new(body))
            }

            Exp::Pair(fst, snd) => {
                let fst = self.go(ctx, fst, &extend(path, 0));
                let snd = self.go(ctx, snd, &extend(path, 1));
                Exp::Pair(Box::new(fst), Box::new(snd))
            }

            Exp::Proj(side, inner) => {
                let mut inner = self.go(ctx, inner, &extend(path, 0));
                let inner_ty = syn(ctx, &inner).unwrap_or(Ty::Hole);
                if matched_prod(&inner_ty).is_none() {
                    inner = self.quarantine(
                        inner,
                        &extend(path, 0),
                        "the merge left a projection over a non-pair",
                    );
                }
                Exp::Proj(*side, Box::new(inner))
            }

            Exp::Cons(head, tail) => {
                let head = self.go(ctx, head, &extend(path, 0));
                let mut tail = self.go(ctx, tail, &extend(path, 1));
                let tail_ty = syn(ctx, &tail).unwrap_or(Ty::Hole);
                let from_tail = match matched_list(&tail_ty) {
                    Some(elem) => elem,
                    None => {
                        tail = self.quarantine(
                            tail,
                            &extend(path, 1),
                            "the merge left a non-list after a `::`",
                        );
                        Ty::Hole
                    }
                };
                let head_ty = syn(ctx, &head).unwrap_or(Ty::Hole);
                let head = if join(&head_ty, &from_tail).is_some() {
                    head
                } else {
                    self.quarantine(
                        head,
                        &extend(path, 0),
                        "the merge left an element the rest of the list cannot hold",
                    )
                };
                Exp::Cons(Box::new(head), Box::new(tail))
            }

            Exp::Fold(list, init, step) => {
                let mut list = self.go(ctx, list, &extend(path, 0));
                let init = self.go(ctx, init, &extend(path, 1));
                let step = self.go(ctx, step, &extend(path, 2));
                let list_ty = syn(ctx, &list).unwrap_or(Ty::Hole);
                let elem = match matched_list(&list_ty) {
                    Some(elem) => elem,
                    None => {
                        list = self.quarantine(
                            list,
                            &extend(path, 0),
                            "the merge left a fold over a non-list",
                        );
                        Ty::Hole
                    }
                };
                let acc = syn(ctx, &init).unwrap_or(Ty::Hole);
                let step = self.ensure_ana(
                    ctx,
                    step,
                    &step_ty(&elem, &acc),
                    &extend(path, 2),
                    "the merge left a fold step that does not match its list",
                );
                Exp::Fold(Box::new(list), Box::new(init), Box::new(step))
            }

            Exp::NonEmptyHole(h, inner) => {
                let inner = self.go(ctx, inner, &extend(path, 0));
                Exp::NonEmptyHole(*h, Box::new(inner))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::exp::{HoleId, Id, Op};
    use nothing_core::typing::is_well_typed;

    #[test]
    fn a_well_typed_program_is_left_exactly_alone() {
        let e = Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2));
        let out = repair(&e, &NameTable::new());
        assert_eq!(out.exp, e);
        assert!(out.repairs.is_empty());
    }

    #[test]
    fn a_type_clash_is_quarantined_rather_than_rejected() {
        let e = Exp::bin_op(Op::Add, Exp::num(1), Exp::bool_(true));
        assert!(!is_well_typed(&e));
        let out = repair(&e, &NameTable::new());
        assert!(is_well_typed(&out.exp));
        assert_eq!(out.repairs.len(), 1);
        assert_eq!(out.repairs[0].kind, RepairKind::Quarantined);
        match &out.exp {
            Exp::BinOp(_, _, rhs) => match &**rhs {
                Exp::NonEmptyHole(_, inner) => assert_eq!(**inner, Exp::bool_(true)),
                other => panic!("expected a non-empty hole, got {other:?}"),
            },
            other => panic!("expected a binop, got {other:?}"),
        }
    }

    #[test]
    fn an_argument_that_no_longer_fits_is_quarantined_not_dropped() {
        let x = Id::from_u128(1);
        let e = Exp::ap(Exp::lam(x, Ty::Num, Exp::var(x)), Exp::bool_(true));
        assert!(!is_well_typed(&e));
        let out = repair(&e, &NameTable::new());
        assert!(is_well_typed(&out.exp));
        assert_eq!(out.repairs.len(), 1);
        assert!(render(&out.exp, &NameTable::new()).contains("⦇true⦈"));
    }

    #[test]
    fn a_merge_that_joins_a_string_to_a_number_quarantines_the_number() {
        let e = Exp::bin_op(Op::Concat, Exp::str_("a"), Exp::num(1));
        assert!(!is_well_typed(&e));
        let out = repair(&e, &NameTable::new());
        assert!(is_well_typed(&out.exp));
        assert_eq!(out.repairs.len(), 1);
        assert_eq!(out.repairs[0].kind, RepairKind::Quarantined);
        assert!(render(&out.exp, &NameTable::new()).contains("⦇1⦈"));
    }

    #[test]
    fn a_merge_that_compares_a_string_to_a_function_quarantines_the_function() {
        let x = Id::from_u128(1);
        let e = Exp::bin_op(Op::Eq, Exp::str_("a"), Exp::lam(x, Ty::Num, Exp::var(x)));
        assert!(!is_well_typed(&e));
        let out = repair(&e, &NameTable::new());
        assert!(is_well_typed(&out.exp));
        assert!(!out.repairs.is_empty());
        assert_eq!(out.repairs[0].kind, RepairKind::Quarantined);
    }

    #[test]
    fn two_strings_compare_without_any_repair_at_all() {
        let e = Exp::bin_op(Op::Eq, Exp::str_("a"), Exp::str_("b"));
        let out = repair(&e, &NameTable::new());
        assert_eq!(out.exp, e);
        assert!(out.repairs.is_empty());
    }

    #[test]
    fn an_unbound_variable_becomes_an_empty_hole() {
        let x = Id::from_u128(1);
        let e = Exp::bin_op(Op::Add, Exp::num(1), Exp::var(x));
        assert!(!is_well_typed(&e));
        let out = repair(&e, &NameTable::new());
        assert!(is_well_typed(&out.exp));
        assert_eq!(out.repairs.len(), 1);
        assert_eq!(out.repairs[0].kind, RepairKind::Unbound);
    }

    #[test]
    fn an_ill_typed_branch_pair_is_repaired_at_the_else_arm() {
        let e = Exp::if_(Exp::bool_(true), Exp::num(1), Exp::bool_(false));
        assert!(!is_well_typed(&e));
        let out = repair(&e, &NameTable::new());
        assert!(is_well_typed(&out.exp));
    }

    #[test]
    fn a_projection_over_a_non_pair_is_repaired() {
        let e = Exp::proj(nothing_core::exp::Side::L, Exp::num(1));
        assert!(!is_well_typed(&e));
        let out = repair(&e, &NameTable::new());
        assert!(is_well_typed(&out.exp));
    }

    #[test]
    fn an_existing_non_empty_hole_survives_repair() {
        let e = Exp::bin_op(
            Op::Add,
            Exp::num(1),
            Exp::non_empty_hole(HoleId::from_u128(9), Exp::bool_(true)),
        );
        let out = repair(&e, &NameTable::new());
        assert_eq!(out.exp, e);
    }
}
