
use crate::ty::Ty;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub u64);

impl Id {
    pub fn new(n: u64) -> Self {
        Id(n)
    }
}

impl std::fmt::Debug for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HoleId(pub u64);

impl HoleId {
    pub fn new(n: u64) -> Self {
        HoleId(n)
    }
}

impl std::fmt::Debug for HoleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "?{}", self.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Lt,
    Eq,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    L,
    R,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Exp {
    Var(Id),
    Lam(Id, Ty, Box<Exp>),
    Ap(Box<Exp>, Box<Exp>),
    Num(i64),
    Bool(bool),
    BinOp(Op, Box<Exp>, Box<Exp>),
    If(Box<Exp>, Box<Exp>, Box<Exp>),
    Let(Id, Box<Exp>, Box<Exp>),
    Pair(Box<Exp>, Box<Exp>),
    Proj(Side, Box<Exp>),

    /// A gap where an expression has not been written yet. It synthesises
    /// type `Hole` so the program stays well-typed mid-edit: an empty hole
    /// is not an error, it is what "not written yet" looks like as a value.
    EmptyHole(HoleId),

    /// Wraps an expression that is well-typed *on its own* but does not fit
    /// its context — the type-error-shaped hole. It quarantines a mistake
    /// (a type inconsistency) so the surrounding program remains
    /// well-formed rather than becoming an invalid tree. Distinguish this
    /// from `EmptyHole`: an empty hole has no content because none was
    /// written; a non-empty hole has content, and that content is exactly
    /// the thing that doesn't fit.
    NonEmptyHole(HoleId, Box<Exp>),
}

impl Exp {
    pub fn var(id: Id) -> Exp {
        Exp::Var(id)
    }

    pub fn lam(id: Id, ty: Ty, body: impl Into<Box<Exp>>) -> Exp {
        Exp::Lam(id, ty, body.into())
    }

    pub fn ap(fun: impl Into<Box<Exp>>, arg: impl Into<Box<Exp>>) -> Exp {
        Exp::Ap(fun.into(), arg.into())
    }

    pub fn num(n: i64) -> Exp {
        Exp::Num(n)
    }

    pub fn bool_(b: bool) -> Exp {
        Exp::Bool(b)
    }

    pub fn bin_op(op: Op, lhs: impl Into<Box<Exp>>, rhs: impl Into<Box<Exp>>) -> Exp {
        Exp::BinOp(op, lhs.into(), rhs.into())
    }

    pub fn if_(
        cond: impl Into<Box<Exp>>,
        then: impl Into<Box<Exp>>,
        else_: impl Into<Box<Exp>>,
    ) -> Exp {
        Exp::If(cond.into(), then.into(), else_.into())
    }

    pub fn let_(id: Id, bound: impl Into<Box<Exp>>, body: impl Into<Box<Exp>>) -> Exp {
        Exp::Let(id, bound.into(), body.into())
    }

    pub fn pair(fst: impl Into<Box<Exp>>, snd: impl Into<Box<Exp>>) -> Exp {
        Exp::Pair(fst.into(), snd.into())
    }

    pub fn proj(side: Side, e: impl Into<Box<Exp>>) -> Exp {
        Exp::Proj(side, e.into())
    }

    pub fn empty_hole(id: HoleId) -> Exp {
        Exp::EmptyHole(id)
    }

    pub fn non_empty_hole(id: HoleId, e: impl Into<Box<Exp>>) -> Exp {
        Exp::NonEmptyHole(id, e.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ty::Ty;

    #[test]
    fn every_variant_reachable_from_a_constructor() {
        let x = Id::new(0);
        let h0 = HoleId::new(0);
        let h1 = HoleId::new(1);

        let exps: Vec<Exp> = vec![
            Exp::var(x),
            Exp::lam(x, Ty::Num, Exp::var(x)),
            Exp::ap(Exp::var(x), Exp::num(1)),
            Exp::num(42),
            Exp::bool_(true),
            Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2)),
            Exp::if_(Exp::bool_(true), Exp::num(1), Exp::num(2)),
            Exp::let_(x, Exp::num(1), Exp::var(x)),
            Exp::pair(Exp::num(1), Exp::bool_(true)),
            Exp::proj(Side::L, Exp::pair(Exp::num(1), Exp::bool_(true))),
            Exp::empty_hole(h0),
            Exp::non_empty_hole(h1, Exp::bool_(true)),
        ];


        assert_eq!(exps.len(), 12);
    }

    #[test]
    fn ids_are_distinguishable_by_value() {
        assert_eq!(Id::new(1), Id::new(1));
        assert_ne!(Id::new(1), Id::new(2));
        assert_eq!(HoleId::new(1), HoleId::new(1));
        assert_ne!(HoleId::new(1), HoleId::new(2));
    }

    #[test]
    fn hole_kinds_are_distinct_variants() {
        let h = HoleId::new(0);
        let empty = Exp::empty_hole(h);
        let non_empty = Exp::non_empty_hole(h, Exp::num(1));
        assert_ne!(empty, non_empty);
        match empty {
            Exp::EmptyHole(_) => {}
            _ => panic!("expected EmptyHole"),
        }
        match non_empty {
            Exp::NonEmptyHole(_, inner) => assert_eq!(*inner, Exp::num(1)),
            _ => panic!("expected NonEmptyHole"),
        }
    }
}