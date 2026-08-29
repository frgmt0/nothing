use nothing_core::doc::{Def, Doc};
use nothing_core::exp::{Exp, HoleId, Id, Op};
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FactorialIds {
    pub fact: Id,
    pub n: Id,
}

impl FactorialIds {
    pub fn fresh() -> FactorialIds {
        FactorialIds {
            fact: Id::fresh(),
            n: Id::fresh(),
        }
    }

    pub fn name(&self, names: &mut NameTable) {
        names.set(self.fact, "main");
        names.set(self.n, "n");
    }
}

fn num_to_num() -> Ty {
    Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Num))
}

pub fn recursive_factorial(ids: FactorialIds) -> Def {
    let body = Exp::lam(
        ids.n,
        Ty::Num,
        Exp::if_(
            Exp::bin_op(Op::Lt, Exp::var(ids.n), Exp::num(1)),
            Exp::num(1),
            Exp::bin_op(
                Op::Mul,
                Exp::var(ids.n),
                Exp::ap(
                    Exp::var(ids.fact),
                    Exp::bin_op(Op::Sub, Exp::var(ids.n), Exp::num(1)),
                ),
            ),
        ),
    );
    Def::new(ids.fact, num_to_num(), body)
}

pub fn factorial_document(ids: FactorialIds) -> Doc {
    Doc::new(vec![recursive_factorial(ids)]).expect("one definition")
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MutualIds {
    pub even: Id,
    pub odd: Id,
    pub n: Id,
}

impl MutualIds {
    pub fn fresh() -> MutualIds {
        MutualIds {
            even: Id::fresh(),
            odd: Id::fresh(),
            n: Id::fresh(),
        }
    }

    pub fn name(&self, names: &mut NameTable) {
        names.set(self.even, "even");
        names.set(self.odd, "odd");
        names.set(self.n, "n");
    }
}

fn parity_body(ids: MutualIds, at_zero: bool, other: Id) -> Exp {
    Exp::lam(
        ids.n,
        Ty::Num,
        Exp::if_(
            Exp::bin_op(Op::Lt, Exp::var(ids.n), Exp::num(1)),
            Exp::bool_(at_zero),
            Exp::ap(
                Exp::var(other),
                Exp::bin_op(Op::Sub, Exp::var(ids.n), Exp::num(1)),
            ),
        ),
    )
}

pub fn mutual_parity_document(ids: MutualIds) -> Doc {
    let ty = Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Bool));
    Doc::new(vec![
        Def::new(ids.even, ty.clone(), parity_body(ids, true, ids.odd)),
        Def::new(ids.odd, ty, parity_body(ids, false, ids.even)),
    ])
    .expect("two definitions")
}

pub fn hole(n: u128) -> HoleId {
    HoleId::from_u128(0xf0_0000_0000_0000_0000_0000_0000_0000 | n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::{eval_doc, eval_doc_with_fuel};

    fn call(doc: &Doc, f: Id, arg: i64) -> Doc {
        let caller = Id::from_u128(0xca11);
        let mut defs = doc.defs().to_vec();
        defs.push(Def::new(
            caller,
            Ty::Hole,
            Exp::ap(Exp::var(f), Exp::num(arg)),
        ));
        Doc::new(defs).expect("the caller id is fresh")
    }

    #[test]
    fn a_self_referencing_definition_typechecks() {
        let ids = FactorialIds::fresh();
        assert!(factorial_document(ids).is_well_typed());
    }

    #[test]
    fn factorial_written_as_a_definition_computes_one_hundred_and_twenty() {
        let ids = FactorialIds::fresh();
        let doc = factorial_document(ids);
        let caller = Id::from_u128(0xca11);
        let outcome = eval_doc(&call(&doc, ids.fact, 5), caller);
        assert_eq!(outcome.num(), Some(120), "{outcome:?}");
    }

    #[test]
    fn the_recursive_definition_agrees_with_the_combinator_on_every_input() {
        let ids = FactorialIds::fresh();
        let doc = factorial_document(ids);
        let caller = Id::from_u128(0xca11);
        let expected = [1i64, 1, 2, 6, 24, 120, 720];
        for (n, want) in expected.iter().enumerate() {
            let outcome = eval_doc(&call(&doc, ids.fact, n as i64), caller);
            assert_eq!(outcome.num(), Some(*want), "factorial({n})");
        }
    }

    #[test]
    fn mutual_recursion_resolves_across_definitions() {
        let ids = MutualIds::fresh();
        let doc = mutual_parity_document(ids);
        assert!(doc.is_well_typed());
        let caller = Id::from_u128(0xca11);
        for n in 0..8i64 {
            let outcome = eval_doc(&call(&doc, ids.even, n), caller);
            assert_eq!(outcome.bool(), Some(n % 2 == 0), "even({n})");
            let outcome = eval_doc(&call(&doc, ids.odd, n), caller);
            assert_eq!(outcome.bool(), Some(n % 2 == 1), "odd({n})");
        }
    }

    #[test]
    fn a_definition_that_diverges_runs_out_of_fuel_rather_than_hanging() {
        let f = Id::from_u128(1);
        let doc = Doc::new(vec![Def::new(
            f,
            Ty::Num,
            Exp::bin_op(Op::Add, Exp::var(f), Exp::num(1)),
        )])
        .expect("one definition");
        assert!(doc.is_well_typed());
        let outcome = eval_doc_with_fuel(&doc, f, 500);
        assert!(outcome.is_out_of_fuel(), "{outcome:?}");
    }

    #[test]
    fn a_missing_definition_evaluates_to_a_blocked_variable() {
        let ids = FactorialIds::fresh();
        let doc = factorial_document(ids);
        let outcome = eval_doc(&doc, Id::from_u128(0xdead));
        assert!(outcome.is_indeterminate(), "{outcome:?}");
    }
}
