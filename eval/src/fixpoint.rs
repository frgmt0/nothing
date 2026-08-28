use nothing_core::exp::{Exp, Id};
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Binders {
    pub f: Id,
    pub x: Id,
    pub v: Id,
}

impl Binders {
    pub fn fresh() -> Binders {
        Binders {
            f: Id::fresh(),
            x: Id::fresh(),
            v: Id::fresh(),
        }
    }

    pub fn name(&self, names: &mut NameTable) {
        names.set(self.f, "step");
        names.set(self.x, "self");
        names.set(self.v, "arg");
    }
}

pub fn z_combinator(b: Binders) -> Exp {
    let half = || {
        Exp::lam(
            b.x,
            Ty::Hole,
            Exp::ap(
                Exp::var(b.f),
                Exp::lam(
                    b.v,
                    Ty::Hole,
                    Exp::ap(Exp::ap(Exp::var(b.x), Exp::var(b.x)), Exp::var(b.v)),
                ),
            ),
        )
    };
    Exp::lam(b.f, Ty::Hole, Exp::ap(half(), half()))
}

pub fn fix(b: Binders, generator: Exp) -> Exp {
    Exp::ap(z_combinator(b), generator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::eval;
    use nothing_core::ctx::Ctx;
    use nothing_core::exp::Op;
    use nothing_core::typing::{is_well_typed, syn};

    fn factorial(b: Binders, fac: Id, n: Id) -> Exp {
        fix(
            b,
            Exp::lam(
                fac,
                Ty::Hole,
                Exp::lam(
                    n,
                    Ty::Num,
                    Exp::if_(
                        Exp::bin_op(Op::Eq, Exp::var(n), Exp::num(0)),
                        Exp::num(1),
                        Exp::bin_op(
                            Op::Mul,
                            Exp::var(n),
                            Exp::ap(
                                Exp::var(fac),
                                Exp::bin_op(Op::Sub, Exp::var(n), Exp::num(1)),
                            ),
                        ),
                    ),
                ),
            ),
        )
    }

    fn ids() -> (Binders, Id, Id) {
        (
            Binders {
                f: Id::from_u128(0x10),
                x: Id::from_u128(0x11),
                v: Id::from_u128(0x12),
            },
            Id::from_u128(0x20),
            Id::from_u128(0x21),
        )
    }

    #[test]
    fn the_combinator_typechecks_in_the_phase_one_surface() {
        let (b, _, _) = ids();
        let z = z_combinator(b);
        assert!(
            is_well_typed(&z),
            "self-application is well-typed at ?, which is the whole point"
        );
        assert_eq!(
            syn(&Ctx::empty(), &z),
            Some(Ty::Arrow(Box::new(Ty::Hole), Box::new(Ty::Hole)))
        );
    }

    #[test]
    fn factorial_computes() {
        let (b, fac, n) = ids();
        let f = factorial(b, fac, n);
        assert!(is_well_typed(&f));

        for (input, expected) in [(0, 1), (1, 1), (2, 2), (3, 6), (5, 120), (10, 3_628_800)] {
            let applied = Exp::ap(f.clone(), Exp::num(input));
            assert!(is_well_typed(&applied));
            assert_eq!(eval(&applied).num(), Some(expected), "factorial({input})");
        }
    }

    #[test]
    fn the_combinator_is_only_unrolled_when_it_is_called() {
        let (b, fac, n) = ids();

        let outcome = eval(&factorial(b, fac, n));
        assert!(
            outcome.is_value(),
            "the fixpoint itself settles to a function: {outcome:?}"
        );
    }

    #[test]
    fn recursion_over_a_second_shape_works_too() {
        let (b, sum, n) = ids();

        let sum_to = fix(
            b,
            Exp::lam(
                sum,
                Ty::Hole,
                Exp::lam(
                    n,
                    Ty::Num,
                    Exp::if_(
                        Exp::bin_op(Op::Lt, Exp::var(n), Exp::num(1)),
                        Exp::num(0),
                        Exp::bin_op(
                            Op::Add,
                            Exp::var(n),
                            Exp::ap(
                                Exp::var(sum),
                                Exp::bin_op(Op::Sub, Exp::var(n), Exp::num(1)),
                            ),
                        ),
                    ),
                ),
            ),
        );
        assert_eq!(eval(&Exp::ap(sum_to, Exp::num(100))).num(), Some(5050));
    }

    #[test]
    fn fresh_binders_are_distinct_and_nameable() {
        let b = Binders::fresh();
        assert_ne!(b.f, b.x);
        assert_ne!(b.x, b.v);
        assert_ne!(b.f, b.v);

        let mut names = NameTable::new();
        b.name(&mut names);
        assert_eq!(
            nothing_core::render::render(&z_combinator(b), &names),
            "λstep:?. (λself:?. step (λarg:?. self self arg)) (λself:?. step (λarg:?. self self arg))"
        );
    }
}
