//! Bidirectional typing (Phase 1): synthesis, analysis, and the
//! well-typedness invariant.
//!
//! The two judgments are mutually recursive and split the work the way the
//! editor needs it split:
//!
//! - [`syn`] runs *bottom-up*. Given a context and an expression, it works
//!   out what type that expression has on its own. It is partial: some
//!   forms carry no information of their own and simply have no synthesised
//!   type.
//! - [`ana`] runs *top-down*. Given a context, an expression, and an
//!   expected type, it checks the expression against that type. The
//!   expected type is the thing the editor cares about most — it is what
//!   powers completion and error recovery at the cursor.
//!
//! Every rule that cannot be decided top-down falls back to *subsumption*:
//! analysing `e` against `τ` succeeds when `e` synthesises some `τ'` that is
//! *consistent* with `τ` (see [`crate::ty::is_consistent`]). Consistency,
//! not equality, is what lets a program containing holes stay well-typed.

use crate::ctx::Ctx;
use crate::exp::{Exp, Op, Side};
use crate::ty::{Ty, is_consistent, matched_arrow, matched_prod};

/// The type both operands of `op` are analysed against.
fn operand_ty(op: Op) -> Ty {
    match op {
        Op::Add | Op::Sub | Op::Mul | Op::Lt | Op::Eq => Ty::Num,
    }
}

/// The type `op` produces: arithmetic yields `Num`, comparison yields
/// `Bool`.
fn result_ty(op: Op) -> Ty {
    match op {
        Op::Add | Op::Sub | Op::Mul => Ty::Num,
        Op::Lt | Op::Eq => Ty::Bool,
    }
}

/// The join (least upper bound under *precision*) of two consistent types:
/// the most specific type that both are consistent with.
///
/// `join(?, τ) = τ` — a hole carries no information, so the other side
/// wins. Structural types join componentwise. Types that are not consistent
/// have no join.
///
/// This is what lets an `if` synthesise: each branch synthesises a type, and
/// the conditional as a whole synthesises the join of the two. Without it a
/// conditional would only ever be checkable, never inferable, and an
/// annotated lambda whose body is a conditional would have no synthesised
/// type at all.
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

/// Synthesis: what type does `exp` have on its own, in `ctx`?
///
/// `None` means "no synthesised type", which is *not* the same as "type
/// error" for every form — but at the top level, in the empty context, it is
/// exactly the failure of the well-typedness invariant (see
/// [`is_well_typed`]).
///
/// Both hole forms synthesise [`Ty::Hole`]: an empty hole because nothing
/// has been written there yet, a non-empty hole because its contents do not
/// fit the surrounding context and so contribute no usable type outward.
/// A non-empty hole's contents must still synthesise *something* in
/// context — the whole point of the non-empty hole is that it quarantines
/// an expression that is well-typed on its own.
pub fn syn(ctx: &Ctx, exp: &Exp) -> Option<Ty> {
    match exp {
        Exp::Var(id) => ctx.lookup(id),

        // An annotated lambda synthesises: the annotation gives the input
        // type, the body's synthesised type gives the output type.
        Exp::Lam(id, ann, body) => {
            let body_ty = syn(&ctx.extend(*id, ann.clone()), body)?;
            Some(Ty::Arrow(Box::new(ann.clone()), Box::new(body_ty)))
        }

        // Application: synthesise the function, force it into an arrow
        // shape (which works even when its type is still a hole), then
        // *analyse* the argument against the input side.
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

        // The scrutinee is checked against `Bool`; the branches synthesise
        // and the conditional takes their join.
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
            // The contents are well-typed on their own — that is what makes
            // this a *non-empty* hole rather than a broken tree — but their
            // type does not escape the hole.
            syn(ctx, inner)?;
            Some(Ty::Hole)
        }
    }
}

/// Analysis: does `exp` check against the expected type `ty`, in `ctx`?
///
/// Three forms are handled specially because they genuinely need the
/// expected type pushed inward — a lambda (to type its parameter and its
/// body), a conditional (to type both branches), and a let (to type its
/// body). Everything else falls back to subsumption: synthesise, then ask
/// whether the synthesised type is *consistent* with the expected one.
pub fn ana(ctx: &Ctx, exp: &Exp, ty: &Ty) -> bool {
    match exp {
        // The expected type is forced into an arrow shape; the annotation
        // need only be *consistent* with the input side, and the body is
        // analysed against the output side.
        Exp::Lam(id, ann, body) => match matched_arrow(ty) {
            Some((in_ty, out_ty)) => {
                is_consistent(ann, &in_ty) && ana(&ctx.extend(*id, ann.clone()), body, &out_ty)
            }
            None => false,
        },

        // Both branches are analysed against the expected type, so a
        // conditional checks even when neither branch synthesises.
        Exp::If(cond, then, else_) => {
            ana(ctx, cond, &Ty::Bool) && ana(ctx, then, ty) && ana(ctx, else_, ty)
        }

        // The bound expression synthesises; the body inherits the expected
        // type under the extended context.
        Exp::Let(id, bound, body) => match syn(ctx, bound) {
            Some(bound_ty) => ana(&ctx.extend(*id, bound_ty), body, ty),
            None => false,
        },

        // A pair against a product shape checks componentwise. This is
        // strictly more permissive than subsumption would be, because a
        // component may itself be a form that only analyses (a lambda, a
        // conditional).
        Exp::Pair(fst, snd) => match matched_prod(ty) {
            Some((l, r)) => ana(ctx, fst, &l) && ana(ctx, snd, &r),
            None => false,
        },

        // Subsumption: everything else synthesises, and the synthesised
        // type must be consistent with what was expected.
        _ => match syn(ctx, exp) {
            Some(syn_ty) => is_consistent(&syn_ty, ty),
            None => false,
        },
    }
}

/// The well-typedness invariant: a program is well-typed exactly when it
/// synthesises a type in the empty context.
///
/// This is the property every edit action must preserve (Phase 2). Note
/// that it is *not* "contains no holes" — a program full of holes, empty
/// and non-empty alike, is well-typed. Incompleteness is not brokenness.
pub fn is_well_typed(exp: &Exp) -> bool {
    syn(&Ctx::empty(), exp).is_some()
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
        Id::new(0)
    }

    fn h(n: u64) -> HoleId {
        HoleId::new(n)
    }

    // --- syn: one test per variant listed in the spec ---

    #[test]
    fn syn_var() {
        let ctx = Ctx::empty().extend(x(), Ty::Bool);
        assert_eq!(syn(&ctx, &Exp::var(x())), Some(Ty::Bool));
        // An unbound variable synthesises nothing.
        assert_eq!(syn(&Ctx::empty(), &Exp::var(x())), None);
    }

    #[test]
    fn syn_lam_via_annotation() {
        // λx:Num. x  synthesises  Num -> Num
        let e = Exp::lam(x(), Ty::Num, Exp::var(x()));
        assert_eq!(syn(&Ctx::empty(), &e), Some(arrow(Ty::Num, Ty::Num)));

        // λx:?. x  synthesises  ? -> ?
        let e = Exp::lam(x(), Ty::Hole, Exp::var(x()));
        assert_eq!(syn(&Ctx::empty(), &e), Some(arrow(Ty::Hole, Ty::Hole)));
    }

    #[test]
    fn syn_ap() {
        // (λx:Num. x) 1  synthesises  Num
        let f = Exp::lam(x(), Ty::Num, Exp::var(x()));
        let e = Exp::ap(f, Exp::num(1));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Num));

        // Applying something of hole type works via matched_arrow, and the
        // result is a hole.
        let ctx = Ctx::empty().extend(x(), Ty::Hole);
        let e = Exp::ap(Exp::var(x()), Exp::num(1));
        assert_eq!(syn(&ctx, &e), Some(Ty::Hole));

        // A bad argument makes the application fail to synthesise.
        let f = Exp::lam(x(), Ty::Num, Exp::var(x()));
        let e = Exp::ap(f, Exp::bool_(true));
        assert_eq!(syn(&Ctx::empty(), &e), None);

        // Applying a non-arrow, non-hole type fails.
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
        // Arithmetic synthesises Num.
        let e = Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Num));

        // Comparison synthesises Bool from Num operands.
        let e = Exp::bin_op(Op::Lt, Exp::num(1), Exp::num(2));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Bool));

        // Operands are *analysed*, so a hole operand is fine.
        let e = Exp::bin_op(Op::Mul, Exp::num(1), Exp::empty_hole(h(0)));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Num));

        // A Bool operand is not.
        let e = Exp::bin_op(Op::Sub, Exp::num(1), Exp::bool_(true));
        assert_eq!(syn(&Ctx::empty(), &e), None);
    }

    #[test]
    fn syn_let() {
        // let x = 1 in x + 1  synthesises  Num
        let e = Exp::let_(
            x(),
            Exp::num(1),
            Exp::bin_op(Op::Add, Exp::var(x()), Exp::num(1)),
        );
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Num));

        // The binding is visible in the body and has the bound
        // expression's synthesised type.
        let e = Exp::let_(x(), Exp::bool_(true), Exp::var(x()));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Bool));

        // The binding does not escape the let.
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

        // Projecting out of a hole-typed expression works via matched_prod.
        let ctx = Ctx::empty().extend(x(), Ty::Hole);
        assert_eq!(
            syn(&ctx, &Exp::proj(Side::R, Exp::var(x()))),
            Some(Ty::Hole)
        );

        // Projecting out of a Num does not.
        assert_eq!(syn(&Ctx::empty(), &Exp::proj(Side::L, Exp::num(1))), None);
    }

    #[test]
    fn syn_if() {
        // Both branches Num -> Num.
        let e = Exp::if_(Exp::bool_(true), Exp::num(1), Exp::num(2));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Num));

        // A hole branch joins to the other branch's type.
        let e = Exp::if_(Exp::bool_(true), Exp::empty_hole(h(0)), Exp::num(2));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Num));

        // Inconsistent branches have no join.
        let e = Exp::if_(Exp::bool_(true), Exp::num(1), Exp::bool_(true));
        assert_eq!(syn(&Ctx::empty(), &e), None);

        // A non-Bool scrutinee fails.
        let e = Exp::if_(Exp::num(0), Exp::num(1), Exp::num(2));
        assert_eq!(syn(&Ctx::empty(), &e), None);
    }

    #[test]
    fn syn_empty_hole() {
        assert_eq!(syn(&Ctx::empty(), &Exp::empty_hole(h(7))), Some(Ty::Hole));
    }

    #[test]
    fn syn_non_empty_hole() {
        // Contents well-typed on their own: the hole synthesises Hole, and
        // the contents' own type (Bool) does not escape.
        let e = Exp::non_empty_hole(h(1), Exp::bool_(true));
        assert_eq!(syn(&Ctx::empty(), &e), Some(Ty::Hole));

        // Contents that do not typecheck in context are not merely
        // ill-fitting, they are ill-typed — the hole does not launder them.
        let e = Exp::non_empty_hole(h(1), Exp::var(x()));
        assert_eq!(syn(&Ctx::empty(), &e), None);

        // ...but with the variable in scope, it is fine again.
        let ctx = Ctx::empty().extend(x(), Ty::Num);
        let e = Exp::non_empty_hole(h(1), Exp::var(x()));
        assert_eq!(syn(&ctx, &e), Some(Ty::Hole));
    }

    // --- ana ---

    /// The spec's acceptance criterion, with the annotation the grammar
    /// requires: `λx:Num. x` against `Num -> Num` succeeds, against `Bool`
    /// fails, against `?` succeeds.
    #[test]
    fn ana_lam_against_arrow_bool_and_hole() {
        let lam = Exp::lam(x(), Ty::Num, Exp::var(x()));
        let ctx = Ctx::empty();

        assert!(ana(&ctx, &lam, &arrow(Ty::Num, Ty::Num)));
        assert!(!ana(&ctx, &lam, &Ty::Bool));
        assert!(ana(&ctx, &lam, &Ty::Hole));
    }

    /// The same three cases for a hole-annotated lambda, `λx:?. x` — the
    /// annotation is consistent with any input type.
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
        // λx:Bool. 1  does not check against  Num -> Num
        let lam = Exp::lam(x(), Ty::Bool, Exp::num(1));
        assert!(!ana(&Ctx::empty(), &lam, &arrow(Ty::Num, Ty::Num)));

        // ...but it does check against  Bool -> Num
        assert!(ana(&Ctx::empty(), &lam, &arrow(Ty::Bool, Ty::Num)));

        // ...and against  ? -> Num, since ? ~ Bool.
        assert!(ana(&Ctx::empty(), &lam, &arrow(Ty::Hole, Ty::Num)));
    }

    #[test]
    fn ana_lam_body_is_checked_against_the_output_side() {
        // λx:Num. x  does not check against  Num -> Bool
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

        // A conditional whose branches are lambdas checks even though the
        // branches alone would need their annotations to line up exactly.
        let e = Exp::if_(
            Exp::bool_(true),
            Exp::lam(x(), Ty::Hole, Exp::var(x())),
            Exp::lam(x(), Ty::Num, Exp::var(x())),
        );
        assert!(ana(&ctx, &e, &arrow(Ty::Num, Ty::Num)));

        // A non-Bool scrutinee fails regardless of the branches.
        let e = Exp::if_(Exp::num(0), Exp::num(1), Exp::num(2));
        assert!(!ana(&ctx, &e, &Ty::Num));
    }

    #[test]
    fn ana_subsumption_uses_consistency_not_equality() {
        let ctx = Ctx::empty();
        // A hole checks against anything.
        assert!(ana(&ctx, &Exp::empty_hole(h(0)), &Ty::Num));
        assert!(ana(&ctx, &Exp::empty_hole(h(0)), &arrow(Ty::Num, Ty::Bool)));
        // A non-empty hole likewise, provided its contents are well-typed.
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
        // A concrete mismatch still fails.
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
        // let x = 1 in (λy:?. y)  checks against  Num -> Num
        let y = Id::new(9);
        let e = Exp::let_(x(), Exp::num(1), Exp::lam(y, Ty::Hole, Exp::var(y)));
        assert!(ana(&ctx, &e, &arrow(Ty::Num, Ty::Num)));
        assert!(!ana(&ctx, &e, &Ty::Bool));
    }

    // --- join ---

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

    // --- is_well_typed ---

    /// The invariant holds on a hand-built program containing *both* hole
    /// kinds:
    ///
    /// ```text
    /// let x = 1 in ( x + ⦇⦈₀ , ⦇true⦈₁ )
    /// ```
    ///
    /// The empty hole sits in a `Num` position and checks there because
    /// `? ~ Num`; the non-empty hole wraps `true`, which is well-typed on
    /// its own, and synthesises `?` outward. The whole program synthesises
    /// `Num * ?`.
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
        // An unbound variable at the top level.
        assert!(!is_well_typed(&Exp::var(x())));
        // A genuine type error that is *not* quarantined in a hole.
        assert!(!is_well_typed(&Exp::bin_op(
            Op::Add,
            Exp::num(1),
            Exp::bool_(true)
        )));
    }

    #[test]
    fn is_well_typed_runs_in_the_empty_context() {
        // Well-typed only because the let binds x; the same body alone is
        // not well-typed.
        let body = Exp::var(x());
        assert!(!is_well_typed(&body));
        assert!(is_well_typed(&Exp::let_(x(), Exp::num(1), body)));
    }
}
