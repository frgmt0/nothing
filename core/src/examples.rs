use crate::exp::{Exp, HoleId, Id, Op, Side};
use crate::names::NameTable;
use crate::ty::Ty;

const EXAMPLE_ID: u128 = 0x6578_616d_706c_6500_0000_0000_0000_0000;

pub const fn binder(n: u128) -> Id {
    Id::from_u128(EXAMPLE_ID | n)
}

pub const fn hole(n: u128) -> HoleId {
    HoleId::from_u128(EXAMPLE_ID | 0xffff_0000 | n)
}

pub fn names() -> NameTable {
    let mut names = NameTable::new();
    for n in 0..4u128 {
        names.set(binder(n), format!("x{n}"));
    }
    names
}

pub fn let_identity() -> Exp {
    let x = binder(0);
    Exp::let_(x, Exp::num(1), Exp::var(x))
}

pub fn increment_applied() -> Exp {
    let x = binder(0);
    Exp::ap(
        Exp::lam(x, Ty::Num, Exp::bin_op(Op::Add, Exp::var(x), Exp::num(1))),
        Exp::num(41),
    )
}

pub fn clamp_to_one() -> Exp {
    let n = binder(0);
    Exp::lam(
        n,
        Ty::Num,
        Exp::if_(
            Exp::bin_op(Op::Lt, Exp::var(n), Exp::num(1)),
            Exp::num(1),
            Exp::var(n),
        ),
    )
}

pub fn pair_and_project() -> Exp {
    let p = binder(0);
    Exp::let_(
        p,
        Exp::pair(Exp::num(1), Exp::bool_(true)),
        Exp::proj(Side::L, Exp::var(p)),
    )
}

pub fn pair_with_empty_hole() -> Exp {
    Exp::pair(Exp::empty_hole(hole(0)), Exp::num(2))
}

pub fn add_with_empty_hole() -> Exp {
    Exp::bin_op(Op::Add, Exp::num(1), Exp::empty_hole(hole(0)))
}

pub fn square_and_compare() -> Exp {
    let f = binder(0);
    let x = binder(1);
    Exp::let_(
        f,
        Exp::lam(x, Ty::Num, Exp::bin_op(Op::Mul, Exp::var(x), Exp::var(x))),
        Exp::bin_op(Op::Eq, Exp::ap(Exp::var(f), Exp::num(5)), Exp::num(25)),
    )
}

pub fn identity_hole_annotated_applied() -> Exp {
    let x = binder(0);
    Exp::ap(Exp::lam(x, Ty::Hole, Exp::var(x)), Exp::bool_(true))
}

pub fn add_with_non_empty_hole() -> Exp {
    Exp::bin_op(
        Op::Add,
        Exp::num(1),
        Exp::non_empty_hole(hole(0), Exp::bool_(true)),
    )
}

pub fn if_over_pairs_with_hole() -> Exp {
    Exp::if_(
        Exp::bool_(true),
        Exp::pair(Exp::num(1), Exp::num(2)),
        Exp::pair(Exp::empty_hole(hole(0)), Exp::num(4)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typing::is_well_typed;

    #[test]
    fn all_ten_examples_are_well_typed() {
        let examples: Vec<(&str, Exp)> = vec![
            ("let_identity", let_identity()),
            ("increment_applied", increment_applied()),
            ("clamp_to_one", clamp_to_one()),
            ("pair_and_project", pair_and_project()),
            ("pair_with_empty_hole", pair_with_empty_hole()),
            ("add_with_empty_hole", add_with_empty_hole()),
            ("square_and_compare", square_and_compare()),
            (
                "identity_hole_annotated_applied",
                identity_hole_annotated_applied(),
            ),
            ("add_with_non_empty_hole", add_with_non_empty_hole()),
            ("if_over_pairs_with_hole", if_over_pairs_with_hole()),
        ];

        assert_eq!(examples.len(), 10, "expected exactly ten example programs");

        for (name, exp) in &examples {
            assert!(
                is_well_typed(exp),
                "expected `{name}` to be well-typed: {exp:?}"
            );
        }
    }

    #[test]
    fn at_least_two_examples_contain_an_empty_hole() {
        fn contains_empty_hole(e: &Exp) -> bool {
            match e {
                Exp::EmptyHole(_) => true,
                Exp::NonEmptyHole(_, inner) => contains_empty_hole(inner),
                Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::Str(_) | Exp::Nil => false,
                Exp::Lam(_, _, body) => contains_empty_hole(body),
                Exp::Ap(f, a) => contains_empty_hole(f) || contains_empty_hole(a),
                Exp::BinOp(_, l, r) => contains_empty_hole(l) || contains_empty_hole(r),
                Exp::If(c, t, e) | Exp::Fold(c, t, e) => {
                    contains_empty_hole(c) || contains_empty_hole(t) || contains_empty_hole(e)
                }
                Exp::Let(_, bound, body) => contains_empty_hole(bound) || contains_empty_hole(body),
                Exp::Pair(l, r) | Exp::Cons(l, r) => {
                    contains_empty_hole(l) || contains_empty_hole(r)
                }
                Exp::Proj(_, e) | Exp::Field(e, _) => contains_empty_hole(e),
                Exp::Record(fields) => fields.iter().any(|(_, e)| contains_empty_hole(e)),
            }
        }

        let count = [
            pair_with_empty_hole(),
            add_with_empty_hole(),
            if_over_pairs_with_hole(),
        ]
        .iter()
        .filter(|e| contains_empty_hole(e))
        .count();

        assert!(
            count >= 2,
            "expected at least two examples with an empty hole"
        );
    }

    #[test]
    fn at_least_one_example_contains_a_non_empty_hole() {
        fn contains_non_empty_hole(e: &Exp) -> bool {
            match e {
                Exp::NonEmptyHole(_, _) => true,
                Exp::EmptyHole(_) => false,
                Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::Str(_) | Exp::Nil => false,
                Exp::Lam(_, _, body) => contains_non_empty_hole(body),
                Exp::Ap(f, a) => contains_non_empty_hole(f) || contains_non_empty_hole(a),
                Exp::BinOp(_, l, r) => contains_non_empty_hole(l) || contains_non_empty_hole(r),
                Exp::If(c, t, e) | Exp::Fold(c, t, e) => {
                    contains_non_empty_hole(c)
                        || contains_non_empty_hole(t)
                        || contains_non_empty_hole(e)
                }
                Exp::Let(_, bound, body) => {
                    contains_non_empty_hole(bound) || contains_non_empty_hole(body)
                }
                Exp::Pair(l, r) | Exp::Cons(l, r) => {
                    contains_non_empty_hole(l) || contains_non_empty_hole(r)
                }
                Exp::Proj(_, e) | Exp::Field(e, _) => contains_non_empty_hole(e),
                Exp::Record(fields) => fields.iter().any(|(_, e)| contains_non_empty_hole(e)),
            }
        }

        assert!(contains_non_empty_hole(&add_with_non_empty_hole()));
    }
}
