use crate::ctx::Ctx;
use crate::exp::{Exp, Op, Side};
use crate::ty::{Ty, is_consistent, matched_arrow, matched_prod};

fn operand_ty(op: Op) -> Ty {
    match op {
        Op::Add | Op::Sub | Op::Mul | Op::Lt | Op::Eq => Ty::Num,
    }
}

fn result_ty(op: Op) -> Ty {
    match op {
        Op::Add | Op::Sub | Op::Mul => Ty::Num,
        Op::Lt | Op::Eq => Ty::Bool,
    }
}

pub fn join(a: &Ty, b: &Ty) -> Option<Ty> {
    match (a, b) {
        (Ty::Hole, t) | (t, Ty::Hole) => Some(t.clone()),
        (Ty::Num, Ty::Num) => Some(Ty::Num),
        (Ty::Bool, Ty::Bool) => Some(Ty::Bool),
        (Ty::Arrow(a1, a2), Ty::Arrow(b1, b2)) => {
            Some(Ty::Arrow(Box::new(join(a1, b1)?), Box::new(join(a2, b2)?)))
        }
        (Ty::Prod(a1, a2), Ty::Prod(b1, b2)) => {
            Some(Ty::Prod(Box::new(join(a1, b1)?), Box::new(join(a2, b2)?)))
        }
        _ => None,
    }
}

pub fn syn(ctx: &Ctx, exp: &Exp) -> Option<Ty> {
    match exp {
        Exp::Var(id) => ctx.lookup(id),

        Exp::Lam(id, ann, body) => {
            let body_ty = syn(&ctx.extend(*id, ann.clone()), body)?;
            Some(Ty::Arrow(Box::new(ann.clone()), Box::new(body_ty)))
        }

        Exp::Ap(fun, arg) => {
            let fun_ty = syn(ctx, fun)?;
            let (in_ty, out_ty) = matched_arrow(&fun_ty)?;
            if ana(ctx, arg, &in_ty) {
                Some(out_ty)
            } else {
                None
            }
        }

        Exp::Num(_) => Some(Ty::Num),
        Exp::Bool(_) => Some(Ty::Bool),

        Exp::BinOp(op, lhs, rhs) => {
            let operand = operand_ty(*op);
            if ana(ctx, lhs, &operand) && ana(ctx, rhs, &operand) {
                Some(result_ty(*op))
            } else {
                None
            }
        }

        Exp::If(cond, then, else_) => {
            if !ana(ctx, cond, &Ty::Bool) {
                return None;
            }
            let then_ty = syn(ctx, then)?;
            let else_ty = syn(ctx, else_)?;
            join(&then_ty, &else_ty)
        }

        Exp::Let(id, bound, body) => {
            let bound_ty = syn(ctx, bound)?;
            syn(&ctx.extend(*id, bound_ty), body)
        }

        Exp::Pair(fst, snd) => {
            let fst_ty = syn(ctx, fst)?;
            let snd_ty = syn(ctx, snd)?;
            Some(Ty::Prod(Box::new(fst_ty), Box::new(snd_ty)))
        }

        Exp::Proj(side, e) => {
            let e_ty = syn(ctx, e)?;
            let (l, r) = matched_prod(&e_ty)?;
            Some(match side {
                Side::L => l,
                Side::R => r,
            })
        }

        Exp::EmptyHole(_) => Some(Ty::Hole),

        Exp::NonEmptyHole(_, inner) => {
            syn(ctx, inner)?;
            Some(Ty::Hole)
        }
    }
}

pub fn ana(ctx: &Ctx, exp: &Exp, ty: &Ty) -> bool {
    match exp {
        Exp::Lam(id, ann, body) => match matched_arrow(ty) {
            Some((in_ty, out_ty)) => {
                is_consistent(ann, &in_ty) && ana(&ctx.extend(*id, ann.clone()), body, &out_ty)
            }
            None => false,
        },

        Exp::If(cond, then, else_) => {
            ana(ctx, cond, &Ty::Bool) && ana(ctx, then, ty) && ana(ctx, else_, ty)
        }

        Exp::Let(id, bound, body) => match syn(ctx, bound) {
            Some(bound_ty) => ana(&ctx.extend(*id, bound_ty), body, ty),
            None => false,
        },

        Exp::Pair(fst, snd) => match matched_prod(ty) {
            Some((l, r)) => ana(ctx, fst, &l) && ana(ctx, snd, &r),
            None => false,
        },

        _ => match syn(ctx, exp) {
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
}
