use nothing_core::exp::{Exp, HoleId, Id, Op, Side};
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;

pub type Env = im::HashMap<Id, Dyn>;

#[derive(Clone, PartialEq, Debug)]
pub enum Dyn {
    Var(Id),
    Lam(Id, Ty, Box<Dyn>),
    Ap(Box<Dyn>, Box<Dyn>),
    Num(i64),
    Bool(bool),
    BinOp(Op, Box<Dyn>, Box<Dyn>),
    If(Box<Dyn>, Box<Dyn>, Box<Dyn>),
    Let(Id, Box<Dyn>, Box<Dyn>),
    Pair(Box<Dyn>, Box<Dyn>),
    Proj(Side, Box<Dyn>),

    EmptyHole(HoleId, Env),
    NonEmptyHole(HoleId, Env, Box<Dyn>),
}

pub fn elaborate(exp: &Exp) -> Dyn {
    elaborate_in(exp, &Env::new())
}

pub fn elaborate_in(exp: &Exp, sigma: &Env) -> Dyn {
    match exp {
        Exp::Var(id) => Dyn::Var(*id),
        Exp::Num(n) => Dyn::Num(*n),
        Exp::Bool(b) => Dyn::Bool(*b),
        Exp::Lam(id, ty, body) => {
            let inner = sigma.update(*id, Dyn::Var(*id));
            Dyn::Lam(*id, ty.clone(), Box::new(elaborate_in(body, &inner)))
        }
        Exp::Ap(fun, arg) => Dyn::Ap(
            Box::new(elaborate_in(fun, sigma)),
            Box::new(elaborate_in(arg, sigma)),
        ),
        Exp::BinOp(op, lhs, rhs) => Dyn::BinOp(
            *op,
            Box::new(elaborate_in(lhs, sigma)),
            Box::new(elaborate_in(rhs, sigma)),
        ),
        Exp::If(cond, then, else_) => Dyn::If(
            Box::new(elaborate_in(cond, sigma)),
            Box::new(elaborate_in(then, sigma)),
            Box::new(elaborate_in(else_, sigma)),
        ),
        Exp::Let(id, bound, body) => {
            let bound = elaborate_in(bound, sigma);
            let inner = sigma.update(*id, Dyn::Var(*id));
            Dyn::Let(*id, Box::new(bound), Box::new(elaborate_in(body, &inner)))
        }
        Exp::Pair(fst, snd) => Dyn::Pair(
            Box::new(elaborate_in(fst, sigma)),
            Box::new(elaborate_in(snd, sigma)),
        ),
        Exp::Proj(side, inner) => Dyn::Proj(*side, Box::new(elaborate_in(inner, sigma))),
        Exp::EmptyHole(h) => Dyn::EmptyHole(*h, sigma.clone()),
        Exp::NonEmptyHole(h, inner) => {
            Dyn::NonEmptyHole(*h, sigma.clone(), Box::new(elaborate_in(inner, sigma)))
        }
    }
}

pub fn subst(x: Id, v: &Dyn, d: &Dyn) -> Dyn {
    match d {
        Dyn::Var(id) if *id == x => v.clone(),
        Dyn::Var(id) => Dyn::Var(*id),
        Dyn::Num(n) => Dyn::Num(*n),
        Dyn::Bool(b) => Dyn::Bool(*b),
        Dyn::Lam(id, ty, body) => {
            if *id == x {
                d.clone()
            } else {
                Dyn::Lam(*id, ty.clone(), Box::new(subst(x, v, body)))
            }
        }
        Dyn::Ap(fun, arg) => Dyn::Ap(Box::new(subst(x, v, fun)), Box::new(subst(x, v, arg))),
        Dyn::BinOp(op, lhs, rhs) => {
            Dyn::BinOp(*op, Box::new(subst(x, v, lhs)), Box::new(subst(x, v, rhs)))
        }
        Dyn::If(cond, then, else_) => Dyn::If(
            Box::new(subst(x, v, cond)),
            Box::new(subst(x, v, then)),
            Box::new(subst(x, v, else_)),
        ),
        Dyn::Let(id, bound, body) => {
            let bound = Box::new(subst(x, v, bound));
            let body = if *id == x {
                body.clone()
            } else {
                Box::new(subst(x, v, body))
            };
            Dyn::Let(*id, bound, body)
        }
        Dyn::Pair(fst, snd) => Dyn::Pair(Box::new(subst(x, v, fst)), Box::new(subst(x, v, snd))),
        Dyn::Proj(side, inner) => Dyn::Proj(*side, Box::new(subst(x, v, inner))),
        Dyn::EmptyHole(h, env) => Dyn::EmptyHole(*h, subst_env(x, v, env)),
        Dyn::NonEmptyHole(h, env, inner) => {
            Dyn::NonEmptyHole(*h, subst_env(x, v, env), Box::new(subst(x, v, inner)))
        }
    }
}

fn subst_env(x: Id, v: &Dyn, env: &Env) -> Env {
    env.iter().map(|(id, d)| (*id, subst(x, v, d))).collect()
}

pub fn is_value(d: &Dyn) -> bool {
    match d {
        Dyn::Num(_) | Dyn::Bool(_) | Dyn::Lam(..) => true,
        Dyn::Pair(fst, snd) => is_value(fst) && is_value(snd),
        _ => false,
    }
}

pub fn to_exp(d: &Dyn) -> Exp {
    match d {
        Dyn::Var(id) => Exp::Var(*id),
        Dyn::Num(n) => Exp::Num(*n),
        Dyn::Bool(b) => Exp::Bool(*b),
        Dyn::Lam(id, ty, body) => Exp::Lam(*id, ty.clone(), Box::new(to_exp(body))),
        Dyn::Ap(fun, arg) => Exp::Ap(Box::new(to_exp(fun)), Box::new(to_exp(arg))),
        Dyn::BinOp(op, lhs, rhs) => Exp::BinOp(*op, Box::new(to_exp(lhs)), Box::new(to_exp(rhs))),
        Dyn::If(cond, then, else_) => Exp::If(
            Box::new(to_exp(cond)),
            Box::new(to_exp(then)),
            Box::new(to_exp(else_)),
        ),
        Dyn::Let(id, bound, body) => Exp::Let(*id, Box::new(to_exp(bound)), Box::new(to_exp(body))),
        Dyn::Pair(fst, snd) => Exp::Pair(Box::new(to_exp(fst)), Box::new(to_exp(snd))),
        Dyn::Proj(side, inner) => Exp::Proj(*side, Box::new(to_exp(inner))),
        Dyn::EmptyHole(h, _) => Exp::EmptyHole(*h),
        Dyn::NonEmptyHole(h, _, inner) => Exp::NonEmptyHole(*h, Box::new(to_exp(inner))),
    }
}

pub fn render(d: &Dyn, names: &NameTable) -> String {
    nothing_core::render::render(&to_exp(d), names)
}

pub fn size(d: &Dyn) -> usize {
    match d {
        Dyn::Var(_) | Dyn::Num(_) | Dyn::Bool(_) | Dyn::EmptyHole(..) => 1,
        Dyn::Lam(_, _, b) | Dyn::Proj(_, b) | Dyn::NonEmptyHole(_, _, b) => 1 + size(b),
        Dyn::Ap(a, b) | Dyn::BinOp(_, a, b) | Dyn::Let(_, a, b) | Dyn::Pair(a, b) => {
            1 + size(a) + size(b)
        }
        Dyn::If(c, t, e) => 1 + size(c) + size(t) + size(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::examples;

    fn x() -> Id {
        Id::from_u128(1)
    }

    fn y() -> Id {
        Id::from_u128(2)
    }

    fn h(n: u128) -> HoleId {
        HoleId::from_u128(n)
    }

    #[test]
    fn elaboration_keeps_the_tree_and_only_adds_hole_environments() {
        for exp in [
            examples::let_identity(),
            examples::increment_applied(),
            examples::clamp_to_one(),
            examples::pair_and_project(),
            examples::pair_with_empty_hole(),
            examples::add_with_empty_hole(),
            examples::square_and_compare(),
            examples::identity_hole_annotated_applied(),
            examples::add_with_non_empty_hole(),
            examples::if_over_pairs_with_hole(),
        ] {
            assert_eq!(to_exp(&elaborate(&exp)), exp);
        }
    }

    #[test]
    fn a_hole_starts_with_the_identity_substitution_over_its_scope() {
        let e = Exp::lam(x(), Ty::Num, Exp::empty_hole(h(0)));
        match elaborate(&e) {
            Dyn::Lam(_, _, body) => match *body {
                Dyn::EmptyHole(_, env) => {
                    assert_eq!(env.len(), 1);
                    assert_eq!(env.get(&x()), Some(&Dyn::Var(x())));
                }
                other => panic!("expected a hole, got {other:?}"),
            },
            other => panic!("expected a lambda, got {other:?}"),
        }
    }

    #[test]
    fn a_hole_at_the_top_captures_nothing() {
        match elaborate(&Exp::empty_hole(h(0))) {
            Dyn::EmptyHole(_, env) => assert!(env.is_empty()),
            other => panic!("expected a hole, got {other:?}"),
        }
    }

    #[test]
    fn substitution_rewrites_a_holes_environment_rather_than_the_hole() {
        let d = elaborate(&Exp::lam(x(), Ty::Num, Exp::empty_hole(h(0))));
        let body = match d {
            Dyn::Lam(_, _, body) => *body,
            other => panic!("expected a lambda, got {other:?}"),
        };
        match subst(x(), &Dyn::Num(5), &body) {
            Dyn::EmptyHole(id, env) => {
                assert_eq!(id, h(0), "the hole keeps its identity");
                assert_eq!(env.get(&x()), Some(&Dyn::Num(5)));
            }
            other => panic!("expected a hole, got {other:?}"),
        }
    }

    #[test]
    fn substitution_stops_at_a_binder_that_shadows_it() {
        let inner = Dyn::Lam(x(), Ty::Num, Box::new(Dyn::Var(x())));
        assert_eq!(subst(x(), &Dyn::Num(9), &inner), inner);

        let outer = Dyn::Lam(y(), Ty::Num, Box::new(Dyn::Var(x())));
        assert_eq!(
            subst(x(), &Dyn::Num(9), &outer),
            Dyn::Lam(y(), Ty::Num, Box::new(Dyn::Num(9)))
        );
    }

    #[test]
    fn a_let_binds_its_body_but_not_its_bound_expression() {
        let d = Dyn::Let(x(), Box::new(Dyn::Var(x())), Box::new(Dyn::Var(x())));
        assert_eq!(
            subst(x(), &Dyn::Num(3), &d),
            Dyn::Let(x(), Box::new(Dyn::Num(3)), Box::new(Dyn::Var(x())))
        );
    }

    #[test]
    fn values_are_the_hole_free_normal_forms() {
        assert!(is_value(&Dyn::Num(1)));
        assert!(is_value(&Dyn::Bool(true)));
        assert!(is_value(&Dyn::Lam(x(), Ty::Num, Box::new(Dyn::Var(x())))));
        assert!(is_value(&Dyn::Pair(
            Box::new(Dyn::Num(1)),
            Box::new(Dyn::Bool(false))
        )));
        assert!(!is_value(&Dyn::EmptyHole(h(0), Env::new())));
        assert!(!is_value(&Dyn::Pair(
            Box::new(Dyn::Num(1)),
            Box::new(Dyn::EmptyHole(h(0), Env::new()))
        )));
        assert!(!is_value(&Dyn::Var(x())));
    }

    #[test]
    fn a_residual_renders_through_the_ordinary_projection() {
        let names = examples::names();
        let d = elaborate(&examples::add_with_empty_hole());
        assert_eq!(render(&d, &names), "1 + ⦇⦈");
    }
}
