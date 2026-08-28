
use std::fmt::Write as _;

use crate::exp::{Exp, Id, Op, Side};
use crate::names::NameTable;

pub type Prec = u8;

pub const PREC_BINDER: Prec = 0;
pub const PREC_CMP: Prec = 1;
pub const PREC_ADD: Prec = 2;
pub const PREC_MUL: Prec = 3;
pub const PREC_APP: Prec = 4;
pub const PREC_ATOM: Prec = 5;

pub fn op_prec(op: Op) -> Prec {
    match op {
        Op::Add | Op::Sub => PREC_ADD,
        Op::Mul => PREC_MUL,
        Op::Lt | Op::Eq => PREC_CMP,
    }
}

pub fn op_str(op: Op) -> &'static str {
    match op {
        Op::Add => "+",
        Op::Sub => "-",
        Op::Mul => "*",
        Op::Lt => "<",
        Op::Eq => "==",
    }
}

pub fn render_id(id: Id, names: &NameTable) -> String {
    names.display(id)
}

pub fn render(exp: &Exp, names: &NameTable) -> String {
    let mut out = String::new();
    fmt_prec(exp, PREC_BINDER, names, &mut out);
    out
}

pub fn render_prec(exp: &Exp, min_prec: Prec, names: &NameTable) -> String {
    let mut out = String::new();
    fmt_prec(exp, min_prec, names, &mut out);
    out
}

fn fmt_prec(exp: &Exp, min_prec: Prec, names: &NameTable, out: &mut String) {
    let own_prec = prec_of(exp);
    let needs_parens = own_prec < min_prec;
    if needs_parens {
        out.push('(');
    }
    match exp {
        Exp::Var(id) => {
            out.push_str(&render_id(*id, names));
        }
        Exp::Num(n) => {
            write!(out, "{n}").unwrap();
        }
        Exp::Bool(b) => {
            write!(out, "{b}").unwrap();
        }
        Exp::EmptyHole(_) => {
            out.push_str("⦇⦈");
        }
        Exp::NonEmptyHole(_, e) => {
            out.push('⦇');


            fmt_prec(e, PREC_BINDER, names, out);
            out.push('⦈');
        }
        Exp::Pair(a, b) => {

            out.push('(');
            fmt_prec(a, PREC_BINDER, names, out);
            out.push_str(", ");
            fmt_prec(b, PREC_BINDER, names, out);
            out.push(')');
        }
        Exp::Proj(side, e) => {
            out.push_str(match side {
                Side::L => "fst ",
                Side::R => "snd ",
            });


            fmt_prec(e, PREC_ATOM, names, out);
        }
        Exp::Ap(f, a) => {


            fmt_prec(f, PREC_APP, names, out);
            out.push(' ');
            fmt_prec(a, PREC_ATOM, names, out);
        }
        Exp::BinOp(op, l, r) => {
            let p = op_prec(*op);


            fmt_prec(l, p, names, out);
            write!(out, " {} ", op_str(*op)).unwrap();
            fmt_prec(r, p + 1, names, out);
        }
        Exp::If(c, t, e) => {
            out.push_str("if ");


            fmt_prec(c, PREC_CMP, names, out);
            out.push_str(" then ");
            fmt_prec(t, PREC_CMP, names, out);
            out.push_str(" else ");


            fmt_prec(e, PREC_BINDER, names, out);
        }
        Exp::Let(id, bound, body) => {
            write!(out, "let {} = ", render_id(*id, names)).unwrap();
            fmt_prec(bound, PREC_CMP, names, out);
            out.push_str(" in ");

            fmt_prec(body, PREC_BINDER, names, out);
        }
        Exp::Lam(id, ty, body) => {
            write!(out, "λ{}:{}. ", render_id(*id, names), ty).unwrap();

            fmt_prec(body, PREC_BINDER, names, out);
        }
    }
    if needs_parens {
        out.push(')');
    }
}

fn prec_of(exp: &Exp) -> Prec {
    match exp {
        Exp::Var(_)
        | Exp::Num(_)
        | Exp::Bool(_)
        | Exp::EmptyHole(_)
        | Exp::NonEmptyHole(_, _)
        | Exp::Pair(_, _) => PREC_ATOM,
        Exp::Ap(_, _) | Exp::Proj(_, _) => PREC_APP,
        Exp::BinOp(op, _, _) => op_prec(*op),
        Exp::If(_, _, _) | Exp::Let(_, _, _) | Exp::Lam(_, _, _) => PREC_BINDER,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exp::{HoleId, Id, Op, Side};
    use crate::ty::Ty;

    fn x(n: u128) -> Id {
        Id::from_u128(n)
    }

    fn names() -> NameTable {
        let mut names = crate::examples::names();
        for n in 0..4u128 {
            names.set(x(n), format!("x{n}"));
        }
        names
    }


    #[test]
    fn mul_binds_tighter_than_add_no_parens_needed() {

        let e = Exp::bin_op(
            Op::Add,
            Exp::num(1),
            Exp::bin_op(Op::Mul, Exp::num(2), Exp::num(3)),
        );
        assert_eq!(render(&e, &names()), "1 + 2 * 3");
    }

    #[test]
    fn add_under_mul_needs_parens() {

        let e = Exp::bin_op(
            Op::Mul,
            Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2)),
            Exp::num(3),
        );
        assert_eq!(render(&e, &names()), "(1 + 2) * 3");
    }

    #[test]
    fn left_associative_chain_no_parens() {

        let e = Exp::bin_op(
            Op::Add,
            Exp::bin_op(Op::Sub, Exp::num(1), Exp::num(2)),
            Exp::num(3),
        );
        assert_eq!(render(&e, &names()), "1 - 2 + 3");
    }

    #[test]
    fn right_nested_same_precedence_needs_parens() {

        let e = Exp::bin_op(
            Op::Sub,
            Exp::num(1),
            Exp::bin_op(Op::Add, Exp::num(2), Exp::num(3)),
        );
        assert_eq!(render(&e, &names()), "1 - (2 + 3)");
    }

    #[test]
    fn application_argument_needs_parens_around_nested_application() {
        let f = x(0);
        let g = x(1);
        let x = x(2);

        let e = Exp::ap(Exp::var(f), Exp::ap(Exp::var(g), Exp::var(x)));
        assert_eq!(render(&e, &names()), "x0 (x1 x2)");
    }

    #[test]
    fn application_chain_left_no_parens() {
        let f = x(0);
        let a = x(1);
        let b = x(2);

        let e = Exp::ap(Exp::ap(Exp::var(f), Exp::var(a)), Exp::var(b));
        assert_eq!(render(&e, &names()), "x0 x1 x2");
    }

    #[test]
    fn proj_wraps_non_atomic_operand() {
        let f = x(0);
        let x = x(1);
        let e = Exp::proj(Side::L, Exp::ap(Exp::var(f), Exp::var(x)));
        assert_eq!(render(&e, &names()), "fst (x0 x1)");
    }

    #[test]
    fn nested_lambda_in_binop_gets_parens() {
        let x = x(0);


        let e = Exp::bin_op(
            Op::Add,
            Exp::num(1),
            Exp::lam(x, Ty::Num, Exp::var(x)),
        );
        assert_eq!(render(&e, &names()), "1 + (λx0:Num. x0)");
    }

    #[test]
    fn let_and_if_extend_rightward_without_trailing_parens() {
        let x = x(0);

        let e = Exp::lam(
            x,
            Ty::Num,
            Exp::let_(
                x,
                Exp::var(x),
                Exp::if_(
                    Exp::bin_op(Op::Lt, Exp::var(x), Exp::num(1)),
                    Exp::num(1),
                    Exp::var(x),
                ),
            ),
        );
        assert_eq!(
            render(&e, &names()),
            "λx0:Num. let x0 = x0 in if x0 < 1 then 1 else x0"
        );
    }


    #[test]
    fn the_projection_takes_every_name_from_the_table() {
        let e = Exp::let_(x(0), Exp::num(1), Exp::var(x(0)));

        let mut mine = NameTable::new();
        mine.set(x(0), "total");
        assert_eq!(render(&e, &mine), "let total = 1 in total");

        let mut theirs = NameTable::new();
        theirs.set(x(0), "sum");
        assert_eq!(render(&e, &theirs), "let sum = 1 in sum");

        assert_eq!(
            e,
            Exp::let_(x(0), Exp::num(1), Exp::var(x(0))),
            "the tree the two projections read is the same tree"
        );
    }

    #[test]
    fn a_binder_the_table_does_not_name_still_renders_and_stays_stable() {
        let e = Exp::lam(x(0), Ty::Num, Exp::var(x(0)));
        let silent = NameTable::new();

        let text = render(&e, &silent);
        assert_eq!(text, render(&e, &silent), "the fallback is deterministic");
        assert!(!text.contains("x0"), "no name was invented from thin air");
        assert_eq!(
            text,
            format!("λ_{0}:Num. _{0}", x(0).short()),
            "an unnamed binder falls back to its identity"
        );
    }

    #[test]
    fn empty_hole_renders_bare_brackets() {
        assert_eq!(render(&Exp::empty_hole(HoleId::from_u128(0)), &names()), "⦇⦈");
    }

    #[test]
    fn non_empty_hole_wraps_contents_unparenthesised() {
        let e = Exp::non_empty_hole(HoleId::from_u128(0), Exp::bool_(true));
        assert_eq!(render(&e, &names()), "⦇true⦈");
    }

    #[test]
    fn hole_contents_never_get_extra_parens_from_context() {

        let e = Exp::bin_op(
            Op::Add,
            Exp::num(1),
            Exp::non_empty_hole(HoleId::from_u128(0), Exp::bool_(true)),
        );
        assert_eq!(render(&e, &names()), "1 + ⦇true⦈");
    }


    #[test]
    fn pair_and_types_render() {
        let e = Exp::pair(Exp::num(1), Exp::bool_(true));
        assert_eq!(render(&e, &names()), "(1, true)");
    }

    #[test]
    fn lambda_with_hole_annotation_renders_question_mark() {
        let x = x(0);
        let e = Exp::lam(x, Ty::Hole, Exp::var(x));
        assert_eq!(render(&e, &names()), "λx0:?. x0");
    }


    use crate::examples::*;

    #[test]
    fn all_ten_examples_render_legibly() {


        let examples: Vec<Exp> = vec![
            let_identity(),
            increment_applied(),
            clamp_to_one(),
            pair_and_project(),
            pair_with_empty_hole(),
            add_with_empty_hole(),
            square_and_compare(),
            identity_hole_annotated_applied(),
            add_with_non_empty_hole(),
            if_over_pairs_with_hole(),
        ];
        assert_eq!(examples.len(), 10);
        for e in &examples {
            let s = render(e, &names());
            assert!(!s.is_empty());
        }
    }

    #[test]
    fn snapshot_let_identity() {
        assert_eq!(render(&let_identity(), &names()), "let x0 = 1 in x0");
    }

    #[test]
    fn snapshot_increment_applied() {
        assert_eq!(
            render(&increment_applied(), &names()),
            "(λx0:Num. x0 + 1) 41"
        );
    }

    #[test]
    fn snapshot_clamp_to_one() {
        assert_eq!(
            render(&clamp_to_one(), &names()),
            "λx0:Num. if x0 < 1 then 1 else x0"
        );
    }

    #[test]
    fn snapshot_pair_and_project() {
        assert_eq!(
            render(&pair_and_project(), &names()),
            "let x0 = (1, true) in fst x0"
        );
    }

    #[test]
    fn snapshot_pair_with_empty_hole() {
        assert_eq!(render(&pair_with_empty_hole(), &names()), "(⦇⦈, 2)");
    }

    #[test]
    fn snapshot_add_with_empty_hole() {
        assert_eq!(render(&add_with_empty_hole(), &names()), "1 + ⦇⦈");
    }

    #[test]
    fn snapshot_square_and_compare() {


        assert_eq!(
            render(&square_and_compare(), &names()),
            "let x0 = (λx1:Num. x1 * x1) in x0 5 == 25"
        );
    }

    #[test]
    fn snapshot_identity_hole_annotated_applied() {
        assert_eq!(
            render(&identity_hole_annotated_applied(), &names()),
            "(λx0:?. x0) true"
        );
    }

    #[test]
    fn snapshot_add_with_non_empty_hole() {
        assert_eq!(render(&add_with_non_empty_hole(), &names()), "1 + ⦇true⦈");
    }

    #[test]
    fn snapshot_if_over_pairs_with_hole() {
        assert_eq!(
            render(&if_over_pairs_with_hole(), &names()),
            "if true then (1, 2) else (⦇⦈, 4)"
        );
    }
}