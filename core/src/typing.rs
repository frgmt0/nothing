use crate::ctx::Ctx;
use crate::exp::{Exp, Id, Op, Side};
use crate::stack::on_deep_stack;
use crate::ty::{
    Ty, is_consistent, matched_arrow, matched_cmd, matched_list, matched_prod, matched_record,
    matched_record_fields, matched_variant, unit, variant_constructors,
};

pub fn fields_are_distinct(ids: &[Id]) -> bool {
    ids.iter().enumerate().all(|(i, id)| !ids[..i].contains(id))
}

pub fn is_comparable(ty: &Ty) -> bool {
    matches!(ty, Ty::Num | Ty::Bool | Ty::Str | Ty::Hole)
}

pub fn operand_ty(ctx: &Ctx, op: Op, lhs: &Exp, rhs: &Exp) -> Option<Ty> {
    match op {
        Op::Add | Op::Sub | Op::Mul | Op::Lt => Some(Ty::Num),
        Op::Concat => Some(Ty::Str),
        Op::Eq => {
            let left = syn(ctx, lhs)?;
            let right = syn(ctx, rhs)?;
            if !is_comparable(&left) || !is_comparable(&right) {
                return None;
            }
            Some(if left == Ty::Hole { right } else { left })
        }
    }
}

pub fn operand_expectation(ctx: &Ctx, op: Op, sibling: &Exp) -> Ty {
    match op {
        Op::Add | Op::Sub | Op::Mul | Op::Lt => Ty::Num,
        Op::Concat => Ty::Str,
        Op::Eq => match syn(ctx, sibling) {
            Some(ty) if is_comparable(&ty) => ty,
            _ => Ty::Hole,
        },
    }
}

pub fn result_ty(op: Op) -> Ty {
    match op {
        Op::Add | Op::Sub | Op::Mul => Ty::Num,
        Op::Concat => Ty::Str,
        Op::Lt | Op::Eq => Ty::Bool,
    }
}

pub fn join(a: &Ty, b: &Ty) -> Option<Ty> {
    match (a, b) {
        (Ty::Hole, t) | (t, Ty::Hole) => Some(t.clone()),
        (Ty::Num, Ty::Num) => Some(Ty::Num),
        (Ty::Bool, Ty::Bool) => Some(Ty::Bool),
        (Ty::Str, Ty::Str) => Some(Ty::Str),
        (Ty::Arrow(a1, a2), Ty::Arrow(b1, b2)) => {
            Some(Ty::Arrow(Box::new(join(a1, b1)?), Box::new(join(a2, b2)?)))
        }
        (Ty::Prod(a1, a2), Ty::Prod(b1, b2)) => {
            Some(Ty::Prod(Box::new(join(a1, b1)?), Box::new(join(a2, b2)?)))
        }
        (Ty::List(a), Ty::List(b)) => Some(Ty::List(Box::new(join(a, b)?))),
        (Ty::Cmd(a), Ty::Cmd(b)) => Some(Ty::Cmd(Box::new(join(a, b)?))),
        (Ty::Record(a), Ty::Record(b)) if a.len() == b.len() => {
            let mut fields = Vec::with_capacity(a.len());
            for (id, left) in a {
                let (_, right) = b.iter().find(|(other, _)| other == id)?;
                fields.push((*id, join(left, right)?));
            }
            Some(Ty::Record(fields))
        }
        (Ty::Variant(a), Ty::Variant(b)) => {
            let mut ctors = Vec::with_capacity(a.len() + b.len());
            for (id, left) in a {
                match b.iter().find(|(other, _)| other == id) {
                    Some((_, right)) => ctors.push((*id, join(left, right)?)),
                    None => ctors.push((*id, left.clone())),
                }
            }
            for (id, right) in b {
                if !a.iter().any(|(other, _)| other == id) {
                    ctors.push((*id, right.clone()));
                }
            }
            Some(Ty::Variant(ctors))
        }
        _ => None,
    }
}

pub fn arm_payload_ty(scrutinee_ty: &Ty, ctor: Id) -> Ty {
    matched_variant(scrutinee_ty, ctor).unwrap_or(Ty::Hole)
}

fn arms_cover(scrutinee_ty: &Ty, arms: &[(Id, Id, Exp)]) -> bool {
    let ids: Vec<Id> = arms.iter().map(|(ctor, _, _)| *ctor).collect();
    if !fields_are_distinct(&ids) {
        return false;
    }
    match variant_constructors(scrutinee_ty) {
        Some(required) => required.iter().all(|ctor| ids.contains(ctor)),
        None => false,
    }
}

pub fn step_ty(elem: &Ty, acc: &Ty) -> Ty {
    Ty::Arrow(
        Box::new(elem.clone()),
        Box::new(Ty::Arrow(Box::new(acc.clone()), Box::new(acc.clone()))),
    )
}

fn syn_cons(ctx: &Ctx, head: &Exp, tail: &Exp) -> Option<Ty> {
    let tail_ty = syn_node(ctx, tail)?;
    let from_tail = matched_list(&tail_ty)?;
    let head_ty = syn_node(ctx, head)?;
    let elem = join(&head_ty, &from_tail)?;
    if ana_node(ctx, tail, &Ty::List(Box::new(elem.clone()))) {
        Some(Ty::List(Box::new(elem)))
    } else {
        None
    }
}

fn syn_fold(ctx: &Ctx, list: &Exp, init: &Exp, step: &Exp) -> Option<Ty> {
    let list_ty = syn_node(ctx, list)?;
    let elem = matched_list(&list_ty)?;
    let acc = syn_node(ctx, init)?;
    if ana_node(ctx, step, &step_ty(&elem, &acc)) {
        Some(acc)
    } else {
        None
    }
}

fn syn_record(ctx: &Ctx, fields: &[(Id, Exp)]) -> Option<Ty> {
    let ids: Vec<Id> = fields.iter().map(|(id, _)| *id).collect();
    if !fields_are_distinct(&ids) {
        return None;
    }
    let mut tys = Vec::with_capacity(fields.len());
    for (id, e) in fields {
        tys.push((*id, syn_node(ctx, e)?));
    }
    Some(Ty::Record(tys))
}

fn syn_match(ctx: &Ctx, scrutinee: &Exp, arms: &[(Id, Id, Exp)]) -> Option<Ty> {
    let scrutinee_ty = syn_node(ctx, scrutinee)?;
    if !arms_cover(&scrutinee_ty, arms) {
        return None;
    }
    let mut result = Ty::Hole;
    for (ctor, binder, body) in arms {
        let payload = arm_payload_ty(&scrutinee_ty, *ctor);
        let body_ty = syn_node(&ctx.extend(*binder, payload), body)?;
        result = join(&result, &body_ty)?;
    }
    Some(result)
}

fn syn_cmd_bind(ctx: &Ctx, command: &Exp, id: Id, body: &Exp) -> Option<Ty> {
    let command_ty = syn_node(ctx, command)?;
    let yielded = matched_cmd(&command_ty)?;
    let body_ty = syn_node(&ctx.extend(id, yielded), body)?;
    let result = matched_cmd(&body_ty)?;
    Some(Ty::Cmd(Box::new(result)))
}

fn ana_cmd_bind(ctx: &Ctx, command: &Exp, id: Id, body: &Exp, ty: &Ty) -> bool {
    if matched_cmd(ty).is_none() {
        return false;
    }
    let Some(command_ty) = syn_node(ctx, command) else {
        return false;
    };
    let Some(yielded) = matched_cmd(&command_ty) else {
        return false;
    };
    ana_node(&ctx.extend(id, yielded), body, ty)
}

pub fn syn(ctx: &Ctx, exp: &Exp) -> Option<Ty> {
    on_deep_stack(|| syn_node(ctx, exp))
}

fn syn_node(ctx: &Ctx, exp: &Exp) -> Option<Ty> {
    match exp {
        Exp::Var(id) => ctx.lookup(id),

        Exp::Lam(id, ann, body) => {
            let body_ty = syn_node(&ctx.extend(*id, ann.clone()), body)?;
            Some(Ty::Arrow(Box::new(ann.clone()), Box::new(body_ty)))
        }

        Exp::Ap(fun, arg) => {
            let fun_ty = syn_node(ctx, fun)?;
            let (in_ty, out_ty) = matched_arrow(&fun_ty)?;
            if ana_node(ctx, arg, &in_ty) {
                Some(out_ty)
            } else {
                None
            }
        }

        Exp::Num(_) => Some(Ty::Num),
        Exp::Bool(_) => Some(Ty::Bool),
        Exp::Str(_) => Some(Ty::Str),

        Exp::BinOp(op, lhs, rhs) => {
            let operand = operand_ty(ctx, *op, lhs, rhs)?;
            if ana_node(ctx, lhs, &operand) && ana_node(ctx, rhs, &operand) {
                Some(result_ty(*op))
            } else {
                None
            }
        }

        Exp::If(cond, then, else_) => {
            if !ana_node(ctx, cond, &Ty::Bool) {
                return None;
            }
            let then_ty = syn_node(ctx, then)?;
            let else_ty = syn_node(ctx, else_)?;
            join(&then_ty, &else_ty)
        }

        Exp::Let(id, bound, body) => {
            let bound_ty = syn_node(ctx, bound)?;
            syn_node(&ctx.extend(*id, bound_ty), body)
        }

        Exp::Pair(fst, snd) => {
            let fst_ty = syn_node(ctx, fst)?;
            let snd_ty = syn_node(ctx, snd)?;
            Some(Ty::Prod(Box::new(fst_ty), Box::new(snd_ty)))
        }

        Exp::Proj(side, e) => {
            let e_ty = syn_node(ctx, e)?;
            let (l, r) = matched_prod(&e_ty)?;
            Some(match side {
                Side::L => l,
                Side::R => r,
            })
        }

        Exp::Nil => Some(Ty::List(Box::new(Ty::Hole))),

        Exp::Cons(head, tail) => syn_cons(ctx, head, tail),

        Exp::Fold(list, init, step) => syn_fold(ctx, list, init, step),

        Exp::Record(fields) => syn_record(ctx, fields),

        Exp::Field(subject, field) => {
            let subject_ty = syn_node(ctx, subject)?;
            matched_record(&subject_ty, *field)
        }

        Exp::Inj(ctor, payload) => {
            let payload_ty = syn_node(ctx, payload)?;
            Some(Ty::Variant(vec![(*ctor, payload_ty)]))
        }

        Exp::Match(scrutinee, arms) => syn_match(ctx, scrutinee, arms),

        Exp::Print(text) => {
            if ana_node(ctx, text, &Ty::Str) {
                Some(Ty::Cmd(Box::new(unit())))
            } else {
                None
            }
        }

        Exp::Readline => Some(Ty::Cmd(Box::new(Ty::Str))),

        Exp::CmdPure(value) => {
            let value_ty = syn_node(ctx, value)?;
            Some(Ty::Cmd(Box::new(value_ty)))
        }

        Exp::CmdBind(command, id, body) => syn_cmd_bind(ctx, command, *id, body),

        Exp::EmptyHole(_) => Some(Ty::Hole),

        Exp::NonEmptyHole(_, inner) => {
            syn_node(ctx, inner)?;
            Some(Ty::Hole)
        }
    }
}

pub fn ana(ctx: &Ctx, exp: &Exp, ty: &Ty) -> bool {
    on_deep_stack(|| ana_node(ctx, exp, ty))
}

fn ana_node(ctx: &Ctx, exp: &Exp, ty: &Ty) -> bool {
    match exp {
        Exp::Lam(id, ann, body) => match matched_arrow(ty) {
            Some((in_ty, out_ty)) => {
                is_consistent(ann, &in_ty) && ana_node(&ctx.extend(*id, ann.clone()), body, &out_ty)
            }
            None => false,
        },

        Exp::If(cond, then, else_) => {
            ana_node(ctx, cond, &Ty::Bool) && ana_node(ctx, then, ty) && ana_node(ctx, else_, ty)
        }

        Exp::Let(id, bound, body) => match syn_node(ctx, bound) {
            Some(bound_ty) => ana_node(&ctx.extend(*id, bound_ty), body, ty),
            None => false,
        },

        Exp::Pair(fst, snd) => match matched_prod(ty) {
            Some((l, r)) => ana_node(ctx, fst, &l) && ana_node(ctx, snd, &r),
            None => false,
        },

        Exp::Cons(head, tail) => match matched_list(ty) {
            Some(elem) => {
                let refined = match syn_node(ctx, head) {
                    Some(head_ty) => match join(&head_ty, &elem) {
                        Some(joined) => joined,
                        None => return false,
                    },
                    None => return false,
                };
                ana_node(ctx, head, &refined) && ana_node(ctx, tail, &Ty::List(Box::new(refined)))
            }
            None => false,
        },

        Exp::Fold(list, init, step) => {
            let Some(list_ty) = syn_node(ctx, list) else {
                return false;
            };
            let Some(elem) = matched_list(&list_ty) else {
                return false;
            };
            ana_node(ctx, init, ty) && ana_node(ctx, step, &step_ty(&elem, ty))
        }

        Exp::Record(fields) => {
            let ids: Vec<Id> = fields.iter().map(|(id, _)| *id).collect();
            if !fields_are_distinct(&ids) {
                return false;
            }
            match matched_record_fields(ty, &ids) {
                Some(expected) => fields
                    .iter()
                    .zip(expected.iter())
                    .all(|((_, e), want)| ana_node(ctx, e, want)),
                None => false,
            }
        }

        Exp::Inj(ctor, payload) => match ty {
            Ty::Hole => syn_node(ctx, payload).is_some(),
            Ty::Variant(ctors) => match ctors.iter().find(|(id, _)| id == ctor) {
                Some((_, want)) => ana_node(ctx, payload, want),
                None => syn_node(ctx, payload).is_some(),
            },
            _ => false,
        },

        Exp::Match(scrutinee, arms) => {
            let Some(scrutinee_ty) = syn_node(ctx, scrutinee) else {
                return false;
            };
            if !arms_cover(&scrutinee_ty, arms) {
                return false;
            }
            arms.iter().all(|(ctor, binder, body)| {
                let payload = arm_payload_ty(&scrutinee_ty, *ctor);
                ana_node(&ctx.extend(*binder, payload), body, ty)
            })
        }

        Exp::CmdPure(value) => match matched_cmd(ty) {
            Some(yielded) => ana_node(ctx, value, &yielded),
            None => false,
        },

        Exp::CmdBind(command, id, body) => ana_cmd_bind(ctx, command, *id, body, ty),

        _ => match syn_node(ctx, exp) {
            Some(syn_ty) => is_consistent(&syn_ty, ty),
            None => false,
        },
    }
}

pub fn is_well_typed(exp: &Exp) -> bool {
    syn(&Ctx::empty(), exp).is_some()
}

pub fn is_well_typed_in(ctx: &Ctx, exp: &Exp) -> bool {
    syn(ctx, exp).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exp::{HoleId, Id};

    fn arrow(a: Ty, b: Ty) -> Ty {
        Ty::Arrow(Box::new(a), Box::new(b))
    }

    fn prod(a: Ty, b: Ty) -> Ty {
        Ty::Prod(Box::new(a), Box::new(b))
    }

    fn x() -> Id {
        Id::from_u128(0)
    }

    fn h(n: u128) -> HoleId {
        HoleId::from_u128(n)
    }

    #[test]
    fn syn_var() {
        let ctx = Ctx::empty().extend(x(), Ty::Bool);
        assert_eq!(syn(&ctx, &Exp::var(x())), Some(Ty::Bool));

        assert_eq!(syn(&Ctx::empty(), &Exp::var(x())), None);
    }

    #[test]
    fn syn_lam_via_annotation() {
        let e = Exp::lam(x(), Ty::Num, Exp::var(x()));
        assert_eq!(syn(&Ctx::empty(), &e), Some(arrow(Ty::Num, Ty::Num)));

        let e = Exp::lam(x(), Ty::Hole, Exp::var(x()));
        assert_eq!(syn(&Ctx::empty(), &e), Some(arrow(Ty::Hole, Ty::Hole)));
    }

    #[test]
    fn syn_ap() {
        let f = Exp::lam(x(), Ty::Num, Exp::var(x()));
        let e = Exp::ap(f, Exp::num(1));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Num));

        let ctx = Ctx::empty().extend(x(), Ty::Hole);
        let e = Exp::ap(Exp::var(x()), Exp::num(1));
        assert_eq!(syn(&ctx, &e), Some(Ty::Hole));

        let f = Exp::lam(x(), Ty::Num, Exp::var(x()));
        let e = Exp::ap(f, Exp::bool_(true));
        assert_eq!(syn(&Ctx::empty(), &e), None);

        let e = Exp::ap(Exp::num(1), Exp::num(2));
        assert_eq!(syn(&Ctx::empty(), &e), None);
    }

    #[test]
    fn syn_num() {
        assert_eq!(syn(&Ctx::empty(), &Exp::num(42)), Some(Ty::Num));
    }

    #[test]
    fn syn_bool() {
        assert_eq!(syn(&Ctx::empty(), &Exp::bool_(false)), Some(Ty::Bool));
    }

    #[test]
    fn syn_str() {
        assert_eq!(syn(&Ctx::empty(), &Exp::str_("hello")), Some(Ty::Str));
        assert_eq!(syn(&Ctx::empty(), &Exp::str_("")), Some(Ty::Str));
    }

    #[test]
    fn syn_concat_joins_two_strings() {
        let e = Exp::bin_op(Op::Concat, Exp::str_("a"), Exp::str_("b"));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Str));

        let e = Exp::bin_op(Op::Concat, Exp::str_("a"), Exp::empty_hole(h(0)));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Str));

        let e = Exp::bin_op(Op::Concat, Exp::str_("a"), Exp::num(1));
        assert_eq!(syn(&Ctx::empty(), &e), None);
    }

    #[test]
    fn eq_compares_at_whichever_base_type_its_operands_have() {
        for (lhs, rhs) in [
            (Exp::num(1), Exp::num(2)),
            (Exp::bool_(true), Exp::bool_(false)),
            (Exp::str_("a"), Exp::str_("b")),
            (Exp::str_("a"), Exp::empty_hole(h(0))),
            (Exp::empty_hole(h(0)), Exp::str_("a")),
            (Exp::empty_hole(h(0)), Exp::empty_hole(h(1))),
        ] {
            let e = Exp::bin_op(Op::Eq, lhs.clone(), rhs.clone());
            assert_eq!(
                syn(&Ctx::empty(), &e),
                Some(Ty::Bool),
                "{lhs:?} == {rhs:?} should compare"
            );
        }
    }

    #[test]
    fn eq_never_compares_across_two_different_base_types() {
        for (lhs, rhs) in [
            (Exp::num(1), Exp::bool_(true)),
            (Exp::num(1), Exp::str_("a")),
            (Exp::str_("a"), Exp::bool_(true)),
            (Exp::bool_(true), Exp::num(1)),
        ] {
            let e = Exp::bin_op(Op::Eq, lhs.clone(), rhs.clone());
            assert_eq!(syn(&Ctx::empty(), &e), None, "{lhs:?} == {rhs:?}");
        }
    }

    #[test]
    fn eq_declines_types_with_no_equality() {
        let f = Exp::lam(x(), Ty::Num, Exp::var(x()));
        let e = Exp::bin_op(Op::Eq, f.clone(), f);
        assert_eq!(syn(&Ctx::empty(), &e), None);

        let p = Exp::pair(Exp::num(1), Exp::num(2));
        let e = Exp::bin_op(Op::Eq, p.clone(), p);
        assert_eq!(syn(&Ctx::empty(), &e), None);
    }

    #[test]
    fn the_expectation_at_an_eq_operand_comes_from_the_other_one() {
        let ctx = Ctx::empty();
        assert_eq!(operand_expectation(&ctx, Op::Eq, &Exp::str_("a")), Ty::Str);
        assert_eq!(operand_expectation(&ctx, Op::Eq, &Exp::num(1)), Ty::Num);
        assert_eq!(
            operand_expectation(&ctx, Op::Eq, &Exp::empty_hole(h(0))),
            Ty::Hole
        );
        assert_eq!(
            operand_expectation(&ctx, Op::Concat, &Exp::empty_hole(h(0))),
            Ty::Str
        );
        assert_eq!(operand_expectation(&ctx, Op::Add, &Exp::str_("a")), Ty::Num);
    }

    #[test]
    fn syn_bin_op() {
        let e = Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Num));

        let e = Exp::bin_op(Op::Lt, Exp::num(1), Exp::num(2));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Bool));

        let e = Exp::bin_op(Op::Mul, Exp::num(1), Exp::empty_hole(h(0)));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Num));

        let e = Exp::bin_op(Op::Sub, Exp::num(1), Exp::bool_(true));
        assert_eq!(syn(&Ctx::empty(), &e), None);
    }

    #[test]
    fn syn_let() {
        let e = Exp::let_(
            x(),
            Exp::num(1),
            Exp::bin_op(Op::Add, Exp::var(x()), Exp::num(1)),
        );
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Num));

        let e = Exp::let_(x(), Exp::bool_(true), Exp::var(x()));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Bool));

        assert_eq!(syn(&Ctx::empty(), &Exp::var(x())), None);
    }

    #[test]
    fn syn_pair() {
        let e = Exp::pair(Exp::num(1), Exp::bool_(true));
        assert_eq!(syn(&Ctx::empty(), &e), Some(prod(Ty::Num, Ty::Bool)));
    }

    #[test]
    fn syn_proj() {
        let p = Exp::pair(Exp::num(1), Exp::bool_(true));
        assert_eq!(
            syn(&Ctx::empty(), &Exp::proj(Side::L, p.clone())),
            Some(Ty::Num)
        );
        assert_eq!(syn(&Ctx::empty(), &Exp::proj(Side::R, p)), Some(Ty::Bool));

        let ctx = Ctx::empty().extend(x(), Ty::Hole);
        assert_eq!(
            syn(&ctx, &Exp::proj(Side::R, Exp::var(x()))),
            Some(Ty::Hole)
        );

        assert_eq!(syn(&Ctx::empty(), &Exp::proj(Side::L, Exp::num(1))), None);
    }

    #[test]
    fn syn_if() {
        let e = Exp::if_(Exp::bool_(true), Exp::num(1), Exp::num(2));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Num));

        let e = Exp::if_(Exp::bool_(true), Exp::empty_hole(h(0)), Exp::num(2));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Num));

        let e = Exp::if_(Exp::bool_(true), Exp::num(1), Exp::bool_(true));
        assert_eq!(syn(&Ctx::empty(), &e), None);

        let e = Exp::if_(Exp::num(0), Exp::num(1), Exp::num(2));
        assert_eq!(syn(&Ctx::empty(), &e), None);
    }

    #[test]
    fn syn_empty_hole() {
        assert_eq!(syn(&Ctx::empty(), &Exp::empty_hole(h(7))), Some(Ty::Hole));
    }

    #[test]
    fn syn_non_empty_hole() {
        let e = Exp::non_empty_hole(h(1), Exp::bool_(true));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Hole));

        let e = Exp::non_empty_hole(h(1), Exp::var(x()));
        assert_eq!(syn(&Ctx::empty(), &e), None);

        let ctx = Ctx::empty().extend(x(), Ty::Num);
        let e = Exp::non_empty_hole(h(1), Exp::var(x()));
        assert_eq!(syn(&ctx, &e), Some(Ty::Hole));
    }

    #[test]
    fn ana_lam_against_arrow_bool_and_hole() {
        let lam = Exp::lam(x(), Ty::Num, Exp::var(x()));
        let ctx = Ctx::empty();

        assert!(ana(&ctx, &lam, &arrow(Ty::Num, Ty::Num)));
        assert!(!ana(&ctx, &lam, &Ty::Bool));
        assert!(ana(&ctx, &lam, &Ty::Hole));
    }

    #[test]
    fn ana_hole_annotated_lam_against_arrow_bool_and_hole() {
        let lam = Exp::lam(x(), Ty::Hole, Exp::var(x()));
        let ctx = Ctx::empty();

        assert!(ana(&ctx, &lam, &arrow(Ty::Num, Ty::Num)));
        assert!(!ana(&ctx, &lam, &Ty::Bool));
        assert!(ana(&ctx, &lam, &Ty::Hole));
    }

    #[test]
    fn ana_lam_annotation_must_be_consistent_with_input() {
        let lam = Exp::lam(x(), Ty::Bool, Exp::num(1));
        assert!(!ana(&Ctx::empty(), &lam, &arrow(Ty::Num, Ty::Num)));

        assert!(ana(&Ctx::empty(), &lam, &arrow(Ty::Bool, Ty::Num)));

        assert!(ana(&Ctx::empty(), &lam, &arrow(Ty::Hole, Ty::Num)));
    }

    #[test]
    fn ana_lam_body_is_checked_against_the_output_side() {
        let lam = Exp::lam(x(), Ty::Num, Exp::var(x()));
        assert!(!ana(&Ctx::empty(), &lam, &arrow(Ty::Num, Ty::Bool)));
    }

    #[test]
    fn ana_if_pushes_expected_type_into_both_branches() {
        let ctx = Ctx::empty();

        let e = Exp::if_(Exp::bool_(true), Exp::num(1), Exp::num(2));
        assert!(ana(&ctx, &e, &Ty::Num));
        assert!(ana(&ctx, &e, &Ty::Hole));
        assert!(!ana(&ctx, &e, &Ty::Bool));

        let e = Exp::if_(
            Exp::bool_(true),
            Exp::lam(x(), Ty::Hole, Exp::var(x())),
            Exp::lam(x(), Ty::Num, Exp::var(x())),
        );
        assert!(ana(&ctx, &e, &arrow(Ty::Num, Ty::Num)));

        let e = Exp::if_(Exp::num(0), Exp::num(1), Exp::num(2));
        assert!(!ana(&ctx, &e, &Ty::Num));
    }

    #[test]
    fn ana_subsumption_uses_consistency_not_equality() {
        let ctx = Ctx::empty();

        assert!(ana(&ctx, &Exp::empty_hole(h(0)), &Ty::Num));
        assert!(ana(&ctx, &Exp::empty_hole(h(0)), &arrow(Ty::Num, Ty::Bool)));

        assert!(ana(
            &ctx,
            &Exp::non_empty_hole(h(1), Exp::bool_(true)),
            &Ty::Num
        ));
        assert!(!ana(
            &ctx,
            &Exp::non_empty_hole(h(1), Exp::var(x())),
            &Ty::Num
        ));

        assert!(!ana(&ctx, &Exp::num(1), &Ty::Bool));
        assert!(ana(&ctx, &Exp::num(1), &Ty::Num));
    }

    #[test]
    fn ana_pair_against_product_and_hole() {
        let ctx = Ctx::empty();
        let p = Exp::pair(Exp::num(1), Exp::bool_(true));
        assert!(ana(&ctx, &p, &prod(Ty::Num, Ty::Bool)));
        assert!(ana(&ctx, &p, &prod(Ty::Hole, Ty::Bool)));
        assert!(ana(&ctx, &p, &Ty::Hole));
        assert!(!ana(&ctx, &p, &prod(Ty::Bool, Ty::Bool)));
        assert!(!ana(&ctx, &p, &Ty::Num));
    }

    #[test]
    fn ana_let_propagates_expected_type_to_body() {
        let ctx = Ctx::empty();

        let y = Id::from_u128(9);
        let e = Exp::let_(x(), Exp::num(1), Exp::lam(y, Ty::Hole, Exp::var(y)));
        assert!(ana(&ctx, &e, &arrow(Ty::Num, Ty::Num)));
        assert!(!ana(&ctx, &e, &Ty::Bool));
    }

    #[test]
    fn join_prefers_the_more_precise_type() {
        assert_eq!(join(&Ty::Hole, &Ty::Num), Some(Ty::Num));
        assert_eq!(join(&Ty::Num, &Ty::Hole), Some(Ty::Num));
        assert_eq!(join(&Ty::Num, &Ty::Num), Some(Ty::Num));
        assert_eq!(
            join(&arrow(Ty::Hole, Ty::Num), &arrow(Ty::Bool, Ty::Hole)),
            Some(arrow(Ty::Bool, Ty::Num))
        );
        assert_eq!(
            join(&prod(Ty::Hole, Ty::Num), &prod(Ty::Bool, Ty::Hole)),
            Some(prod(Ty::Bool, Ty::Num))
        );
        assert_eq!(join(&Ty::Num, &Ty::Bool), None);
    }

    #[test]
    fn is_well_typed_on_a_program_with_both_hole_kinds() {
        let prog = Exp::let_(
            x(),
            Exp::num(1),
            Exp::pair(
                Exp::bin_op(Op::Add, Exp::var(x()), Exp::empty_hole(h(0))),
                Exp::non_empty_hole(h(1), Exp::bool_(true)),
            ),
        );

        assert!(is_well_typed(&prog));
        assert_eq!(
            syn(&Ctx::empty(), &prog),
            Some(prod(Ty::Num, Ty::Hole)),
            "the program's synthesised type"
        );
    }

    #[test]
    fn is_well_typed_rejects_a_program_that_does_not_synthesise() {
        assert!(!is_well_typed(&Exp::var(x())));

        assert!(!is_well_typed(&Exp::bin_op(
            Op::Add,
            Exp::num(1),
            Exp::bool_(true)
        )));
    }

    #[test]
    fn is_well_typed_runs_in_the_empty_context() {
        let body = Exp::var(x());
        assert!(!is_well_typed(&body));
        assert!(is_well_typed(&Exp::let_(x(), Exp::num(1), body)));
    }

    fn fx() -> Id {
        Id::from_u128(0xf1)
    }

    fn fy() -> Id {
        Id::from_u128(0xf2)
    }

    fn point() -> Exp {
        Exp::record([(fx(), Exp::num(1)), (fy(), Exp::str_("here"))])
    }

    #[test]
    fn a_record_synthesises_a_record_type_field_by_field() {
        assert_eq!(
            syn(&Ctx::empty(), &point()),
            Some(crate::ty::record([(fx(), Ty::Num), (fy(), Ty::Str)]))
        );
        assert_eq!(
            syn(&Ctx::empty(), &Exp::record([])),
            Some(crate::ty::record([]))
        );
    }

    #[test]
    fn the_same_field_twice_in_one_record_is_ill_formed() {
        let doubled = Exp::record([(fx(), Exp::num(1)), (fx(), Exp::num(2))]);
        assert_eq!(syn(&Ctx::empty(), &doubled), None);
        assert!(!ana(&Ctx::empty(), &doubled, &Ty::Hole));
    }

    #[test]
    fn a_record_analyses_against_a_record_type_whatever_order_it_is_written_in() {
        let want = crate::ty::record([(fy(), Ty::Str), (fx(), Ty::Num)]);
        assert!(ana(&Ctx::empty(), &point(), &want));
        assert!(ana(&Ctx::empty(), &point(), &Ty::Hole));

        let wrong = crate::ty::record([(fx(), Ty::Bool), (fy(), Ty::Str)]);
        assert!(!ana(&Ctx::empty(), &point(), &wrong));
        assert!(!ana(&Ctx::empty(), &point(), &Ty::Num));
    }

    #[test]
    fn projecting_a_field_reads_the_records_own_type() {
        assert_eq!(
            syn(&Ctx::empty(), &Exp::field(point(), fx())),
            Some(Ty::Num)
        );
        assert_eq!(
            syn(&Ctx::empty(), &Exp::field(point(), fy())),
            Some(Ty::Str)
        );
        assert_eq!(
            syn(&Ctx::empty(), &Exp::field(point(), Id::from_u128(0xf9))),
            None,
            "a field the record does not have has no type"
        );
    }

    #[test]
    fn projecting_a_field_of_an_unknown_record_fails_open() {
        let p = Id::from_u128(0xa1);
        let f = Exp::lam(p, Ty::Hole, Exp::field(Exp::var(p), fx()));
        assert_eq!(
            syn(&Ctx::empty(), &f),
            Some(arrow(Ty::Hole, Ty::Hole)),
            "a function over records is writable without a record annotation"
        );
        assert!(is_well_typed(&f));

        let hole = Exp::field(Exp::empty_hole(h(3)), fx());
        assert_eq!(syn(&Ctx::empty(), &hole), Some(Ty::Hole));
    }

    #[test]
    fn a_record_with_a_hole_in_one_field_still_projects_the_others() {
        let partial = Exp::record([(fx(), Exp::num(1)), (fy(), Exp::empty_hole(h(4)))]);
        assert_eq!(
            syn(&Ctx::empty(), &Exp::field(partial.clone(), fx())),
            Some(Ty::Num)
        );
        assert_eq!(
            syn(&Ctx::empty(), &Exp::field(partial, fy())),
            Some(Ty::Hole)
        );
    }

    #[test]
    fn a_record_joins_field_wise_across_the_branches_of_an_if() {
        let then = Exp::record([(fx(), Exp::num(1)), (fy(), Exp::empty_hole(h(5)))]);
        let else_ = Exp::record([(fx(), Exp::empty_hole(h(6))), (fy(), Exp::str_("x"))]);
        assert_eq!(
            syn(&Ctx::empty(), &Exp::if_(Exp::bool_(true), then, else_)),
            Some(crate::ty::record([(fx(), Ty::Num), (fy(), Ty::Str)]))
        );

        let mismatched = Exp::if_(
            Exp::bool_(true),
            Exp::record([(fx(), Exp::num(1))]),
            Exp::record([(fx(), Exp::bool_(true))]),
        );
        assert_eq!(syn(&Ctx::empty(), &mismatched), None);
    }

    fn red() -> Id {
        Id::from_u128(0xc1)
    }

    fn green() -> Id {
        Id::from_u128(0xc2)
    }

    fn payload(n: u128) -> Id {
        Id::from_u128(0xb0 + n)
    }

    fn two_coloured() -> Exp {
        Exp::if_(
            Exp::bool_(true),
            Exp::inj(red(), Exp::unit()),
            Exp::inj(green(), Exp::num(1)),
        )
    }

    #[test]
    fn an_injection_synthesises_the_one_case_it_knows_about() {
        assert_eq!(
            syn(&Ctx::empty(), &Exp::inj(red(), Exp::unit())),
            Some(crate::ty::variant([(red(), crate::ty::unit())]))
        );
        assert_eq!(
            syn(&Ctx::empty(), &Exp::inj(red(), Exp::num(1))),
            Some(crate::ty::variant([(red(), Ty::Num)]))
        );
    }

    #[test]
    fn joining_two_injections_is_the_union_of_their_cases() {
        assert_eq!(
            syn(&Ctx::empty(), &two_coloured()),
            Some(crate::ty::variant([
                (red(), crate::ty::unit()),
                (green(), Ty::Num)
            ])),
            "a sum accepts whatever either branch produces"
        );

        let disagreeing_payloads = Exp::if_(
            Exp::bool_(true),
            Exp::inj(red(), Exp::num(1)),
            Exp::inj(red(), Exp::bool_(true)),
        );
        assert_eq!(syn(&Ctx::empty(), &disagreeing_payloads), None);
    }

    #[test]
    fn an_injection_analyses_against_any_variant_that_has_its_case() {
        let colour = crate::ty::variant([(red(), crate::ty::unit()), (green(), Ty::Num)]);
        assert!(ana(&Ctx::empty(), &Exp::inj(red(), Exp::unit()), &colour));
        assert!(ana(&Ctx::empty(), &Exp::inj(green(), Exp::num(1)), &colour));
        assert!(ana(&Ctx::empty(), &Exp::inj(red(), Exp::unit()), &Ty::Hole));

        assert!(
            !ana(&Ctx::empty(), &Exp::inj(green(), Exp::unit()), &colour),
            "the case is there but the payload is not"
        );
        assert!(
            ana(
                &Ctx::empty(),
                &Exp::inj(Id::from_u128(0xc9), Exp::unit()),
                &colour
            ),
            "a case the expected variant has never heard of widens it rather than failing"
        );
        assert!(!ana(&Ctx::empty(), &Exp::inj(red(), Exp::unit()), &Ty::Num));
        assert!(!ana(
            &Ctx::empty(),
            &Exp::inj(red(), Exp::var(Id::from_u128(0xdead))),
            &Ty::Hole
        ));
        assert!(
            !ana(
                &Ctx::empty(),
                &Exp::inj(green(), Exp::unit()),
                &crate::ty::variant([(green(), Ty::Num)])
            ),
            "the analytic rule reads the payload the variant declares, not any payload"
        );
    }

    #[test]
    fn a_match_must_answer_for_every_constructor_its_scrutinee_can_produce() {
        let complete = Exp::match_(
            two_coloured(),
            [
                (red(), payload(0), Exp::num(0)),
                (green(), payload(1), Exp::var(payload(1))),
            ],
        );
        assert_eq!(syn(&Ctx::empty(), &complete), Some(Ty::Num));

        let missing = Exp::match_(two_coloured(), [(red(), payload(0), Exp::num(0))]);
        assert_eq!(
            syn(&Ctx::empty(), &missing),
            None,
            "a missing arm is not a program"
        );
        assert!(!ana(&Ctx::empty(), &missing, &Ty::Num));

        let doubled = Exp::match_(
            two_coloured(),
            [
                (red(), payload(0), Exp::num(0)),
                (red(), payload(1), Exp::num(1)),
                (green(), payload(2), Exp::num(2)),
            ],
        );
        assert_eq!(syn(&Ctx::empty(), &doubled), None);
    }

    #[test]
    fn a_dead_arm_is_legal_and_binds_a_payload_of_unknown_type() {
        let with_spare = Exp::match_(
            Exp::inj(red(), Exp::unit()),
            [
                (red(), payload(0), Exp::num(0)),
                (green(), payload(1), Exp::num(1)),
            ],
        );
        assert_eq!(
            syn(&Ctx::empty(), &with_spare),
            Some(Ty::Num),
            "an arm the scrutinee cannot reach is how a case is prepared"
        );

        let uses_the_spare = Exp::match_(
            Exp::inj(red(), Exp::unit()),
            [
                (red(), payload(0), Exp::num(0)),
                (green(), payload(1), Exp::var(payload(1))),
            ],
        );
        assert_eq!(
            syn(&Ctx::empty(), &uses_the_spare),
            Some(Ty::Num),
            "the dead arm's binder has the unknown type, and the join keeps the precise one"
        );
    }

    #[test]
    fn a_match_on_an_unknown_scrutinee_needs_no_arms_at_all() {
        let c = Id::from_u128(0xa7);
        let f = Exp::lam(c, Ty::Hole, Exp::match_(Exp::var(c), []));
        assert_eq!(
            syn(&Ctx::empty(), &f),
            Some(Ty::Arrow(Box::new(Ty::Hole), Box::new(Ty::Hole)))
        );
        assert!(is_well_typed(&f));

        let one_arm = Exp::lam(
            c,
            Ty::Hole,
            Exp::match_(Exp::var(c), [(red(), payload(0), Exp::num(1))]),
        );
        assert_eq!(
            syn(&Ctx::empty(), &one_arm),
            Some(Ty::Arrow(Box::new(Ty::Hole), Box::new(Ty::Num)))
        );
    }

    #[test]
    fn a_match_on_something_that_is_not_a_sum_has_no_type() {
        let e = Exp::match_(Exp::num(1), [(red(), payload(0), Exp::num(0))]);
        assert_eq!(syn(&Ctx::empty(), &e), None);
        assert_eq!(
            syn(&Ctx::empty(), &Exp::match_(Exp::num(1), [])),
            None,
            "even with no arms, a number is not something to case-split"
        );
    }

    #[test]
    fn a_match_analyses_by_pushing_the_expectation_into_every_arm() {
        let e = Exp::match_(
            two_coloured(),
            [
                (red(), payload(0), Exp::empty_hole(h(0))),
                (green(), payload(1), Exp::num(2)),
            ],
        );
        assert!(ana(&Ctx::empty(), &e, &Ty::Num));
        assert!(ana(&Ctx::empty(), &e, &Ty::Hole));
        assert!(!ana(&Ctx::empty(), &e, &Ty::Bool));
    }

    fn cmd(result: Ty) -> Ty {
        Ty::Cmd(Box::new(result))
    }

    #[test]
    fn the_three_leaf_commands_synthesise_what_they_yield() {
        assert_eq!(
            syn(&Ctx::empty(), &Exp::print(Exp::str_("hi"))),
            Some(cmd(crate::ty::unit())),
            "print yields the empty record, not a unit type of its own"
        );
        assert_eq!(
            syn(&Ctx::empty(), &Exp::print(Exp::num(1))),
            None,
            "print analyses its payload against Str"
        );
        assert_eq!(
            syn(&Ctx::empty(), &Exp::print(Exp::empty_hole(h(0)))),
            Some(cmd(crate::ty::unit())),
            "a hole is consistent with Str, so print of a hole still has a type"
        );
        assert_eq!(syn(&Ctx::empty(), &Exp::readline()), Some(cmd(Ty::Str)));
        assert_eq!(
            syn(&Ctx::empty(), &Exp::cmd_pure(Exp::num(1))),
            Some(cmd(Ty::Num))
        );
    }

    #[test]
    fn a_bind_puts_what_the_command_yields_into_scope_for_the_body() {
        let e = Exp::cmd_bind(Exp::readline(), x(), Exp::cmd_pure(Exp::var(x())));
        assert_eq!(
            syn(&Ctx::empty(), &e),
            Some(cmd(Ty::Str)),
            "the binder is a Str because readline yields one"
        );

        let mismatched = Exp::cmd_bind(
            Exp::readline(),
            x(),
            Exp::print(Exp::bin_op(crate::exp::Op::Add, Exp::var(x()), Exp::num(1))),
        );
        assert_eq!(
            syn(&Ctx::empty(), &mismatched),
            None,
            "and adding one to it is not something the rule lets through"
        );

        assert_eq!(
            syn(
                &Ctx::empty(),
                &Exp::cmd_bind(Exp::num(1), x(), Exp::readline())
            ),
            None,
            "the thing bound must be a command"
        );
        assert_eq!(
            syn(
                &Ctx::empty(),
                &Exp::cmd_bind(Exp::readline(), x(), Exp::num(1))
            ),
            None,
            "and so must the body"
        );
    }

    #[test]
    fn a_hole_in_a_bind_still_has_a_type_because_matched_cmd_fails_open() {
        let e = Exp::cmd_bind(
            Exp::empty_hole(h(0)),
            x(),
            Exp::print(Exp::empty_hole(h(1))),
        );
        assert_eq!(
            syn(&Ctx::empty(), &e),
            Some(cmd(crate::ty::unit())),
            "an unwritten command is consistent with every command"
        );
        assert!(ana(&Ctx::empty(), &e, &cmd(crate::ty::unit())));
        assert!(ana(&Ctx::empty(), &e, &Ty::Hole));
        assert!(!ana(&Ctx::empty(), &e, &Ty::Num));
    }

    #[test]
    fn analysis_pushes_the_expected_command_type_through_pure_and_bind() {
        assert!(ana(
            &Ctx::empty(),
            &Exp::cmd_pure(Exp::empty_hole(h(0))),
            &cmd(Ty::Num)
        ));
        assert!(!ana(
            &Ctx::empty(),
            &Exp::cmd_pure(Exp::bool_(true)),
            &cmd(Ty::Num)
        ));

        let e = Exp::cmd_bind(Exp::readline(), x(), Exp::cmd_pure(Exp::empty_hole(h(1))));
        assert!(
            ana(&Ctx::empty(), &e, &cmd(Ty::Num)),
            "the expectation reaches the hole at the end of the chain"
        );
        assert!(
            !ana(&Ctx::empty(), &e, &Ty::Num),
            "a command is not its result"
        );
    }
}
