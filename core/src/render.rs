use std::fmt::Write as _;

use crate::exp::{Exp, Id, Op, Side};
use crate::names::NameTable;
use crate::stack::on_deep_stack;
use crate::ty::Ty;

pub type Prec = u8;

pub const PREC_BINDER: Prec = 0;
pub const PREC_CMP: Prec = 1;
pub const PREC_CONS: Prec = 2;
pub const PREC_ADD: Prec = 3;
pub const PREC_MUL: Prec = 4;
pub const PREC_APP: Prec = 5;
pub const PREC_ATOM: Prec = 6;

pub const CONS_STR: &str = "::";
pub const FIELD_STR: &str = ".";
pub const NIL_STR: &str = "nil";
pub const FOLD_STR: &str = "fold";
pub const INJ_STR: &str = "`";
pub const MATCH_STR: &str = "match";
pub const ARM_STR: &str = "->";
pub const ARM_SEP_STR: &str = "|";
pub const PRINT_STR: &str = "print";
pub const READLINE_STR: &str = "readline";
pub const PURE_STR: &str = "pure";
pub const BIND_STR: &str = "bind";
pub const BIND_ARROW_STR: &str = "<-";

pub fn op_prec(op: Op) -> Prec {
    match op {
        Op::Add | Op::Sub | Op::Concat => PREC_ADD,
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
        Op::Concat => "++",
    }
}

pub fn escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out
}

pub fn quote_str(s: &str) -> String {
    format!("\"{}\"", escape_str(s))
}

pub fn render_id(id: Id, names: &NameTable) -> String {
    names.display(id)
}

pub fn render_ty(ty: &Ty, names: &NameTable) -> String {
    let mut out = String::new();
    fmt_ty(ty, 0, names, &mut out);
    out
}

fn fmt_ty(ty: &Ty, min_prec: u8, names: &NameTable, out: &mut String) {
    match ty {
        Ty::Num | Ty::Bool | Ty::Str | Ty::Hole => write!(out, "{ty}").unwrap(),
        Ty::Arrow(a, b) => {
            let parens = min_prec > 0;
            if parens {
                out.push('(');
            }
            fmt_ty(a, 1, names, out);
            out.push_str(" -> ");
            fmt_ty(b, 0, names, out);
            if parens {
                out.push(')');
            }
        }
        Ty::Prod(a, b) => {
            let parens = min_prec > 1;
            if parens {
                out.push('(');
            }
            fmt_ty(a, 2, names, out);
            out.push_str(" * ");
            fmt_ty(b, 2, names, out);
            if parens {
                out.push(')');
            }
        }
        Ty::List(elem) => {
            let parens = min_prec > 2;
            if parens {
                out.push('(');
            }
            out.push_str("List ");
            fmt_ty(elem, 3, names, out);
            if parens {
                out.push(')');
            }
        }
        Ty::Cmd(result) => {
            let parens = min_prec > 2;
            if parens {
                out.push('(');
            }
            out.push_str("Cmd ");
            fmt_ty(result, 3, names, out);
            if parens {
                out.push(')');
            }
        }
        Ty::Record(fields) => {
            out.push('{');
            for (i, (id, field_ty)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write!(out, "{}: ", render_id(*id, names)).unwrap();
                fmt_ty(field_ty, 0, names, out);
            }
            out.push('}');
        }
        Ty::Variant(ctors) => {
            out.push('[');
            for (i, (id, payload)) in ctors.iter().enumerate() {
                if i > 0 {
                    write!(out, " {ARM_SEP_STR} ").unwrap();
                }
                write!(out, "{}: ", render_id(*id, names)).unwrap();
                fmt_ty(payload, 0, names, out);
            }
            out.push(']');
        }
    }
}

pub fn render(exp: &Exp, names: &NameTable) -> String {
    render_prec(exp, PREC_BINDER, names)
}

pub fn render_prec(exp: &Exp, min_prec: Prec, names: &NameTable) -> String {
    on_deep_stack(|| {
        let mut out = String::new();
        fmt_prec(exp, min_prec, names, &mut out);
        out
    })
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
        Exp::Str(s) => {
            out.push_str(&quote_str(s));
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
        Exp::Nil => {
            out.push_str(NIL_STR);
        }
        Exp::Cons(head, tail) => {
            fmt_prec(head, PREC_CONS + 1, names, out);
            write!(out, " {CONS_STR} ").unwrap();
            fmt_prec(tail, PREC_CONS, names, out);
        }
        Exp::Fold(list, init, step) => {
            out.push_str(FOLD_STR);
            out.push(' ');
            fmt_prec(list, PREC_ATOM, names, out);
            out.push(' ');
            fmt_prec(init, PREC_ATOM, names, out);
            out.push(' ');
            fmt_prec(step, PREC_ATOM, names, out);
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
            write!(out, "λ{}:{}. ", render_id(*id, names), render_ty(ty, names)).unwrap();

            fmt_prec(body, PREC_BINDER, names, out);
        }
        Exp::Record(fields) => {
            out.push('{');
            for (i, (id, e)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write!(out, "{} = ", render_id(*id, names)).unwrap();
                fmt_prec(e, PREC_BINDER, names, out);
            }
            out.push('}');
        }
        Exp::Field(subject, id) => {
            fmt_prec(subject, PREC_ATOM, names, out);
            out.push_str(FIELD_STR);
            out.push_str(&render_id(*id, names));
        }
        Exp::Inj(ctor, payload) => {
            write!(out, "{INJ_STR}{} ", render_id(*ctor, names)).unwrap();
            fmt_prec(payload, PREC_ATOM, names, out);
        }
        Exp::Print(text) => {
            out.push_str(PRINT_STR);
            out.push(' ');
            fmt_prec(text, PREC_ATOM, names, out);
        }
        Exp::Readline => {
            out.push_str(READLINE_STR);
        }
        Exp::CmdPure(value) => {
            out.push_str(PURE_STR);
            out.push(' ');
            fmt_prec(value, PREC_ATOM, names, out);
        }
        Exp::CmdBind(command, id, body) => {
            write!(
                out,
                "{BIND_STR} {} {BIND_ARROW_STR} ",
                render_id(*id, names)
            )
            .unwrap();
            fmt_prec(command, PREC_CMP, names, out);
            out.push_str(" in ");
            fmt_prec(body, PREC_BINDER, names, out);
        }
        Exp::Match(scrutinee, arms) => {
            write!(out, "{MATCH_STR} ").unwrap();
            fmt_prec(scrutinee, PREC_ATOM, names, out);
            out.push_str(" {");
            for (i, (ctor, binder, body)) in arms.iter().enumerate() {
                if i > 0 {
                    write!(out, " {ARM_SEP_STR}").unwrap();
                }
                write!(
                    out,
                    " {} {} {ARM_STR} ",
                    render_id(*ctor, names),
                    render_id(*binder, names)
                )
                .unwrap();
                fmt_prec(body, PREC_CMP, names, out);
            }
            if arms.is_empty() {
                out.push('}');
            } else {
                out.push_str(" }");
            }
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
        | Exp::Str(_)
        | Exp::EmptyHole(_)
        | Exp::NonEmptyHole(_, _)
        | Exp::Nil
        | Exp::Record(_)
        | Exp::Field(_, _)
        | Exp::Match(..)
        | Exp::Readline
        | Exp::Pair(_, _) => PREC_ATOM,
        Exp::Cons(_, _) => PREC_CONS,
        Exp::Ap(_, _)
        | Exp::Proj(_, _)
        | Exp::Fold(..)
        | Exp::Inj(..)
        | Exp::Print(_)
        | Exp::CmdPure(_) => PREC_APP,
        Exp::BinOp(op, _, _) => op_prec(*op),
        Exp::If(_, _, _) | Exp::Let(_, _, _) | Exp::Lam(_, _, _) | Exp::CmdBind(..) => PREC_BINDER,
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
    fn a_string_renders_between_quotes_with_only_two_escapes() {
        assert_eq!(render(&Exp::str_("hello"), &names()), "\"hello\"");
        assert_eq!(render(&Exp::str_(""), &names()), "\"\"");
        assert_eq!(
            render(&Exp::str_("a \"b\" c"), &names()),
            "\"a \\\"b\\\" c\""
        );
        assert_eq!(render(&Exp::str_("a\\b"), &names()), "\"a\\\\b\"");
        assert_eq!(render(&Exp::str_("1 + 2 ⦇⦈"), &names()), "\"1 + 2 ⦇⦈\"");
    }

    #[test]
    fn concat_renders_and_associates_like_addition() {
        let e = Exp::bin_op(Op::Concat, Exp::str_("a"), Exp::str_("b"));
        assert_eq!(render(&e, &names()), "\"a\" ++ \"b\"");

        let e = Exp::bin_op(
            Op::Mul,
            Exp::bin_op(Op::Concat, Exp::str_("a"), Exp::str_("b")),
            Exp::num(3),
        );
        assert_eq!(render(&e, &names()), "(\"a\" ++ \"b\") * 3");
    }

    #[test]
    fn a_cons_chain_renders_right_associatively_and_ends_in_nil() {
        assert_eq!(render(&Exp::Nil, &names()), "nil");
        assert_eq!(
            render(
                &Exp::list([Exp::num(1), Exp::num(2), Exp::num(3)]),
                &names()
            ),
            "1 :: 2 :: 3 :: nil"
        );
        assert_eq!(
            render(
                &Exp::cons(
                    Exp::num(1),
                    Exp::cons(Exp::num(2), Exp::empty_hole(HoleId::from_u128(0)))
                ),
                &names()
            ),
            "1 :: 2 :: ⦇⦈"
        );
    }

    #[test]
    fn cons_binds_looser_than_arithmetic_and_tighter_than_comparison() {
        let e = Exp::cons(Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2)), Exp::Nil);
        assert_eq!(render(&e, &names()), "1 + 2 :: nil");

        let e = Exp::bin_op(
            Op::Eq,
            Exp::cons(Exp::num(1), Exp::Nil),
            Exp::cons(Exp::num(1), Exp::Nil),
        );
        assert_eq!(render(&e, &names()), "1 :: nil == 1 :: nil");

        let e = Exp::cons(Exp::cons(Exp::num(1), Exp::Nil), Exp::Nil);
        assert_eq!(render(&e, &names()), "(1 :: nil) :: nil");
    }

    #[test]
    fn fold_renders_as_an_application_of_three_atoms() {
        let f = x(0);
        let e = Exp::fold(
            Exp::list([Exp::num(1), Exp::num(2)]),
            Exp::num(0),
            Exp::var(f),
        );
        assert_eq!(render(&e, &names()), "fold (1 :: 2 :: nil) 0 x0");

        let e = Exp::fold(Exp::var(f), Exp::num(0), Exp::var(f));
        assert_eq!(render(&e, &names()), "fold x0 0 x0");

        let e = Exp::cons(Exp::fold(Exp::Nil, Exp::num(0), Exp::var(f)), Exp::Nil);
        assert_eq!(render(&e, &names()), "fold nil 0 x0 :: nil");
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

        let e = Exp::bin_op(Op::Add, Exp::num(1), Exp::lam(x, Ty::Num, Exp::var(x)));
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
        assert_eq!(
            render(&Exp::empty_hole(HoleId::from_u128(0)), &names()),
            "⦇⦈"
        );
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

    fn field_names() -> NameTable {
        let mut names = names();
        names.set(x(10), "x");
        names.set(x(11), "y");
        names
    }

    #[test]
    fn a_record_renders_its_fields_by_display_name() {
        let point = Exp::record([(x(10), Exp::num(1)), (x(11), Exp::num(2))]);
        assert_eq!(render(&point, &field_names()), "{x = 1, y = 2}");
        assert_eq!(render(&Exp::record([]), &field_names()), "{}");
        assert_eq!(
            render(
                &Exp::record([(x(10), Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2)))]),
                &field_names()
            ),
            "{x = 1 + 2}"
        );
    }

    #[test]
    fn a_projection_binds_tighter_than_anything_else() {
        let names = field_names();
        let p = Exp::var(x(0));
        assert_eq!(render(&Exp::field(p.clone(), x(10)), &names), "x0.x");
        assert_eq!(
            render(&Exp::field(Exp::field(p.clone(), x(10)), x(11)), &names),
            "x0.x.y"
        );
        assert_eq!(
            render(&Exp::field(Exp::ap(p.clone(), Exp::num(1)), x(10)), &names),
            "(x0 1).x"
        );
        assert_eq!(
            render(&Exp::ap(p.clone(), Exp::field(p.clone(), x(10))), &names),
            "x0 x0.x"
        );
        assert_eq!(
            render(
                &Exp::bin_op(Op::Add, Exp::field(p, x(10)), Exp::num(1)),
                &names
            ),
            "x0.x + 1"
        );
    }

    fn variant_names() -> NameTable {
        let mut names = field_names();
        names.set(x(20), "Red");
        names.set(x(21), "Green");
        names.set(x(22), "p");
        names.set(x(23), "q");
        names
    }

    #[test]
    fn an_injection_is_a_backtick_a_name_and_one_payload() {
        let names = variant_names();
        assert_eq!(
            render(&Exp::inj(x(20), Exp::unit()), &names),
            "`Red {}",
            "a nullary constructor spells its empty payload out"
        );
        assert_eq!(render(&Exp::inj(x(20), Exp::num(1)), &names), "`Red 1");
        assert_eq!(
            render(
                &Exp::inj(x(20), Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2))),
                &names
            ),
            "`Red (1 + 2)",
            "an injection takes an atom, like an application"
        );
        assert_eq!(
            render(
                &Exp::bin_op(Op::Eq, Exp::inj(x(20), Exp::unit()), Exp::num(1)),
                &names
            ),
            "`Red {} == 1"
        );
    }

    #[test]
    fn a_match_renders_its_arms_between_braces_and_needs_no_parentheses() {
        let names = variant_names();
        let scrutinee = Exp::inj(x(20), Exp::unit());
        assert_eq!(
            render(&Exp::match_(scrutinee.clone(), []), &names),
            "match (`Red {}) {}",
            "a match with nothing to answer for is still a match"
        );
        assert_eq!(
            render(&Exp::match_(Exp::num(1), []), &names),
            "match 1 {}",
            "an atomic scrutinee wears nothing"
        );
        assert_eq!(
            render(
                &Exp::match_(
                    scrutinee.clone(),
                    [(x(20), x(22), Exp::num(1)), (x(21), x(23), Exp::var(x(23))),]
                ),
                &names
            ),
            "match (`Red {}) { Red p -> 1 | Green q -> q }",
            "the scrutinee sits at atom precedence so the brace after it is never ambiguous"
        );
        assert_eq!(
            render(
                &Exp::bin_op(
                    Op::Add,
                    Exp::match_(scrutinee.clone(), [(x(20), x(22), Exp::num(1))]),
                    Exp::num(2)
                ),
                &names
            ),
            "match (`Red {}) { Red p -> 1 } + 2",
            "a match is delimited, so it is an atom and never wears parentheses"
        );
        assert_eq!(
            render(
                &Exp::match_(
                    Exp::if_(Exp::bool_(true), scrutinee.clone(), scrutinee),
                    [(x(20), x(22), Exp::lam(x(23), Ty::Num, Exp::var(x(23))))]
                ),
                &names
            ),
            "match (if true then `Red {} else `Red {}) { Red p -> (λq:Num. q) }",
            "the scrutinee and the arm bodies both parenthesise the binder forms"
        );
    }

    #[test]
    fn a_variant_type_reads_its_constructor_names_from_the_table() {
        let names = variant_names();
        let colour = crate::ty::variant([(x(20), crate::ty::unit()), (x(21), Ty::Num)]);
        assert_eq!(render_ty(&colour, &names), "[Red: {} | Green: Num]");
        assert_eq!(render_ty(&crate::ty::variant([]), &names), "[]");
        assert_eq!(
            render_ty(&Ty::List(Box::new(colour)), &names),
            "List [Red: {} | Green: Num]"
        );
    }

    #[test]
    fn a_record_type_in_an_annotation_reads_its_field_names_from_the_table() {
        let ty = crate::ty::record([(x(10), Ty::Num), (x(11), Ty::Str)]);
        assert_eq!(render_ty(&ty, &field_names()), "{x: Num, y: Str}");
        assert_eq!(
            render(&Exp::lam(x(0), ty, Exp::num(1)), &field_names()),
            "λx0:{x: Num, y: Str}. 1"
        );
        assert_eq!(render_ty(&Ty::Num, &field_names()), "Num");
        assert_eq!(
            render_ty(
                &Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Bool)),
                &field_names()
            ),
            "Num -> Bool"
        );
    }

    #[test]
    fn the_command_forms_read_as_words_and_bracket_like_the_forms_they_copy() {
        let names = names();
        assert_eq!(render(&Exp::readline(), &names), "readline");
        assert_eq!(render(&Exp::print(Exp::str_("hi")), &names), "print \"hi\"");
        assert_eq!(render(&Exp::cmd_pure(Exp::num(1)), &names), "pure 1");
        assert_eq!(
            render(
                &Exp::print(Exp::bin_op(Op::Concat, Exp::str_("a"), Exp::str_("b"))),
                &names
            ),
            "print (\"a\" ++ \"b\")",
            "print takes an atom, exactly as fst does"
        );
        assert_eq!(
            render(
                &Exp::cmd_bind(Exp::readline(), x(0), Exp::print(Exp::var(x(0)))),
                &names
            ),
            "bind x0 <- readline in print x0"
        );
        assert_eq!(
            render(
                &Exp::cmd_bind(
                    Exp::cmd_bind(Exp::readline(), x(0), Exp::cmd_pure(Exp::var(x(0)))),
                    x(1),
                    Exp::print(Exp::var(x(1)))
                ),
                &names
            ),
            "bind x1 <- (bind x0 <- readline in pure x0) in print x1",
            "a bind in the bound position is bracketed and one in the body is not, \
             exactly as a let is"
        );
        assert_eq!(
            render(
                &Exp::bin_op(
                    Op::Add,
                    Exp::num(1),
                    Exp::cmd_bind(Exp::readline(), x(0), Exp::cmd_pure(Exp::num(2)))
                ),
                &names
            ),
            "1 + (bind x0 <- readline in pure 2)"
        );
    }

    #[test]
    fn a_command_type_reads_as_a_prefix_like_a_list() {
        let names = names();
        assert_eq!(render_ty(&crate::ty::cmd(Ty::Str), &names), "Cmd Str");
        assert_eq!(
            render_ty(&crate::ty::cmd(crate::ty::unit()), &names),
            "Cmd {}"
        );
        assert_eq!(
            render_ty(
                &crate::ty::cmd(Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Num))),
                &names
            ),
            "Cmd (Num -> Num)"
        );
        assert_eq!(
            render_ty(
                &Ty::Arrow(Box::new(Ty::Str), Box::new(crate::ty::cmd(Ty::Num))),
                &names
            ),
            "Str -> Cmd Num"
        );
    }
}
