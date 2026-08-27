//! A generator for arbitrary **well-typed** programs.
//!
//! This module is load-bearing for the whole of Phase 2: the sensibility
//! proptest, the reachability proptest, the zipper round-trip, and the
//! movement invariants all need a supply of random programs that satisfy
//! [`nothing_core::typing::is_well_typed`]. Rejection sampling would be
//! hopeless (a random `Exp` is almost never well-typed), so instead
//! programs are built **top-down from an expected type**, which is exactly
//! the discipline bidirectional typing gives us.
//!
//! # The invariant
//!
//! [`Gen::exp_syn`] produces `e` such that `syn(ctx, e) == Some(ty)` —
//! *exactly* `ty`, not merely something consistent with it. That exactness
//! is what makes the construction compose: every rule below can rely on its
//! children synthesising precisely what it asked for.
//!
//! [`Gen::exp_ana`] produces `e` such that `ana(ctx, e, ty)` holds. It is
//! used at the positions where the typing rules *analyse* rather than
//! synthesise (binary-operator operands, function arguments, the scrutinee
//! of an `if`), and it is where holes get to appear in non-hole positions —
//! which is how programs like `1 + ⦇⦈` come out of the generator.
//!
//! # Why a seed, not a `proptest::Strategy`
//!
//! The generator is a plain deterministic function of a `u64` seed and
//! carries **no dependency on `proptest`**, so `proptest` stays a
//! dev-dependency of this crate and the generator remains usable from
//! benchmarks, fuzzers, and the REPL harness. To use it in a property test,
//! one line suffices:
//!
//! ```ignore
//! proptest! {
//!     #[test]
//!     fn my_property(seed in any::<u64>()) {
//!         let e = gen::well_typed_exp(seed);
//!         // ...
//!     }
//! }
//! ```
//!
//! The cost is that shrinking shrinks the *seed*, not the program, so a
//! failing case is not automatically minimised. When that matters, shrink
//! by hand with [`well_typed_exp_with_depth`] at a smaller depth.

use nothing_core::exp::{Exp, HoleId, Id, Op, Side};
use nothing_core::ty::Ty;

/// A tiny deterministic PRNG (SplitMix64). Deliberately dependency-free —
/// this crate should not grow a `rand` dependency just to shuffle a few
/// constructor choices.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform-ish integer in `0..n`. Panics if `n == 0`.
    pub fn below(&mut self, n: usize) -> usize {
        assert!(n > 0, "Rng::below(0)");
        (self.next_u64() % (n as u64)) as usize
    }

    pub fn boolean(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }

    /// A small signed integer, for `Num` literals.
    pub fn small_int(&mut self) -> i64 {
        (self.next_u64() % 21) as i64 - 10
    }

    /// Pick an element of a non-empty slice.
    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

/// Which form to build at a given position. Kept as an explicit tag rather
/// than a closure list so the candidate set is easy to read and to extend.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Form {
    /// The form dictated by the target type's own shape: a literal for
    /// `Num`/`Bool`, an empty hole for `?`, a lambda for an arrow, a pair
    /// for a product. Always available, and always terminating.
    Canonical,
    Var,
    Let,
    If,
    Ap,
    Proj,
    BinOp,
    NonEmptyHole,
}

/// The generator state: a PRNG plus the fresh-name counters.
///
/// Binder [`Id`]s are globally fresh within one `Gen`, so the generated
/// contexts never shadow and a `Var` picked out of the context always
/// resolves to the binding the generator intended.
#[derive(Clone, Debug)]
pub struct Gen {
    rng: Rng,
    next_id: u64,
    next_hole: u64,
}

impl Gen {
    pub fn new(seed: u64) -> Gen {
        Gen {
            rng: Rng::new(seed),
            next_id: 0,
            next_hole: 0,
        }
    }

    pub fn rng(&mut self) -> &mut Rng {
        &mut self.rng
    }

    fn fresh_id(&mut self) -> Id {
        let id = Id::new(self.next_id);
        self.next_id += 1;
        id
    }

    fn fresh_hole(&mut self) -> HoleId {
        let h = HoleId::new(self.next_hole);
        self.next_hole += 1;
        h
    }

    /// A random type, nested at most `depth` deep.
    pub fn ty(&mut self, depth: u32) -> Ty {
        let n = if depth == 0 { 3 } else { 5 };
        match self.rng.below(n) {
            0 => Ty::Num,
            1 => Ty::Bool,
            2 => Ty::Hole,
            3 => Ty::Arrow(
                Box::new(self.ty(depth - 1)),
                Box::new(self.ty(depth - 1)),
            ),
            _ => Ty::Prod(
                Box::new(self.ty(depth - 1)),
                Box::new(self.ty(depth - 1)),
            ),
        }
    }

    /// A whole program: a random type, then an expression synthesising it
    /// in the empty context. The result satisfies `is_well_typed`.
    pub fn program(&mut self, depth: u32) -> Exp {
        let ty = self.ty(2);
        self.exp_syn(&[], &ty, depth)
    }

    /// Generate `e` with `syn(ctx, e) == Some(ty)`.
    ///
    /// `ctx` is the same context the typing judgment will build as it
    /// descends, represented as a list of bindings in scope; `depth` is the
    /// remaining budget for *non-structural* recursion. The `Canonical`
    /// form may still recurse at depth 0, but only on a structurally
    /// smaller type, so termination is guaranteed either way.
    pub fn exp_syn(&mut self, ctx: &[(Id, Ty)], ty: &Ty, depth: u32) -> Exp {
        // Which in-scope bindings synthesise exactly `ty`? Only those are
        // usable, because `syn(Var id)` is the context lookup verbatim.
        let vars: Vec<Id> = ctx
            .iter()
            .filter(|(_, t)| t == ty)
            .map(|(id, _)| *id)
            .collect();

        let mut cands = vec![Form::Canonical];
        if !vars.is_empty() {
            // Weighted up: programs that never mention their binders are
            // boring, and the interesting typing rules are the ones that
            // thread a context.
            cands.push(Form::Var);
            cands.push(Form::Var);
        }
        if depth > 0 {
            cands.push(Form::Let);
            cands.push(Form::If);
            cands.push(Form::Ap);
            cands.push(Form::Proj);
            if *ty == Ty::Num || *ty == Ty::Bool {
                cands.push(Form::BinOp);
                cands.push(Form::BinOp);
            }
            if *ty == Ty::Hole {
                cands.push(Form::NonEmptyHole);
            }
        }

        let form = *self.rng.pick(&cands);
        let d = depth.saturating_sub(1);

        match form {
            // syn(Num) = Num, syn(Bool) = Bool, syn(⦇⦈) = ?,
            // syn(λx:a. body) = a -> syn(body), syn((a, b)) = syn a * syn b.
            Form::Canonical => match ty {
                Ty::Num => Exp::num(self.rng.small_int()),
                Ty::Bool => Exp::bool_(self.rng.boolean()),
                Ty::Hole => Exp::empty_hole(self.fresh_hole()),
                Ty::Arrow(a, b) => {
                    let id = self.fresh_id();
                    let mut inner = ctx.to_vec();
                    inner.push((id, (**a).clone()));
                    let body = self.exp_syn(&inner, b, d);
                    Exp::lam(id, (**a).clone(), body)
                }
                Ty::Prod(a, b) => {
                    let fst = self.exp_syn(ctx, a, d);
                    let snd = self.exp_syn(ctx, b, d);
                    Exp::pair(fst, snd)
                }
            },

            // syn(Var id) = ctx[id], and `vars` was filtered to exactly ty.
            Form::Var => Exp::var(*self.rng.pick(&vars)),

            // syn(let x = e1 in e2) = syn(e2) under ctx, x : syn(e1).
            Form::Let => {
                let sigma = self.ty(1);
                let bound = self.exp_syn(ctx, &sigma, d);
                let id = self.fresh_id();
                let mut inner = ctx.to_vec();
                inner.push((id, sigma));
                let body = self.exp_syn(&inner, ty, d);
                Exp::let_(id, bound, body)
            }

            // syn(if c then t else e) = join(syn t, syn e), and join(τ, τ) = τ.
            Form::If => {
                let cond = self.exp_ana(ctx, &Ty::Bool, d);
                let then = self.exp_syn(ctx, ty, d);
                let else_ = self.exp_syn(ctx, ty, d);
                Exp::if_(cond, then, else_)
            }

            // syn(f a) = the output side of matched_arrow(syn f), given
            // ana(a, input side).
            Form::Ap => {
                let sigma = self.ty(1);
                let fun_ty = Ty::Arrow(Box::new(sigma.clone()), Box::new(ty.clone()));
                let fun = self.exp_syn(ctx, &fun_ty, d);
                let arg = self.exp_ana(ctx, &sigma, d);
                Exp::ap(fun, arg)
            }

            // syn(proj_L e) = the left side of matched_prod(syn e).
            Form::Proj => {
                let sigma = self.ty(1);
                let side = if self.rng.boolean() { Side::L } else { Side::R };
                let pair_ty = match side {
                    Side::L => Ty::Prod(Box::new(ty.clone()), Box::new(sigma)),
                    Side::R => Ty::Prod(Box::new(sigma), Box::new(ty.clone())),
                };
                let inner = self.exp_syn(ctx, &pair_ty, d);
                Exp::proj(side, inner)
            }

            // Operands are *analysed* against Num; the result type is Num
            // for arithmetic and Bool for comparison.
            Form::BinOp => {
                let op = if *ty == Ty::Num {
                    *self.rng.pick(&[Op::Add, Op::Sub, Op::Mul])
                } else {
                    *self.rng.pick(&[Op::Lt, Op::Eq])
                };
                let lhs = self.exp_ana(ctx, &Ty::Num, d);
                let rhs = self.exp_ana(ctx, &Ty::Num, d);
                Exp::bin_op(op, lhs, rhs)
            }

            // syn(⦇e⦈) = ? provided e synthesises *something* in context.
            Form::NonEmptyHole => {
                let sigma = self.ty(1);
                let inner = self.exp_syn(ctx, &sigma, d);
                Exp::non_empty_hole(self.fresh_hole(), inner)
            }
        }
    }

    /// Generate `e` with `ana(ctx, e, ty)`.
    ///
    /// Three ways to satisfy analysis:
    ///
    /// 1. an expression synthesising exactly `ty` (consistency is
    ///    reflexive, and the analysis rules that are *not* subsumption —
    ///    lambda, `if`, `let`, pair — all agree with synthesis when the
    ///    synthesised type is the expected one);
    /// 2. an empty hole, which synthesises `?` and so is consistent with
    ///    anything;
    /// 3. a non-empty hole, likewise `?` outward, quarantining an
    ///    expression of some unrelated type.
    ///
    /// Cases 2 and 3 are the reason the generator emits programs like
    /// `1 + ⦇⦈` and `1 + ⦇true⦈` rather than only fully-written ones.
    pub fn exp_ana(&mut self, ctx: &[(Id, Ty)], ty: &Ty, depth: u32) -> Exp {
        match self.rng.below(6) {
            0 => Exp::empty_hole(self.fresh_hole()),
            1 if depth > 0 => {
                let sigma = self.ty(1);
                let inner = self.exp_syn(ctx, &sigma, depth - 1);
                Exp::non_empty_hole(self.fresh_hole(), inner)
            }
            _ => self.exp_syn(ctx, ty, depth),
        }
    }
}

/// The default depth budget. Deep enough that programs have interesting
/// nesting (five to a few dozen nodes), shallow enough that ten thousand of
/// them generate in well under a second.
pub const DEFAULT_DEPTH: u32 = 3;

/// An arbitrary well-typed program from a seed.
pub fn well_typed_exp(seed: u64) -> Exp {
    well_typed_exp_with_depth(seed, DEFAULT_DEPTH)
}

/// An arbitrary well-typed program from a seed, with an explicit depth
/// budget. Depth 0 yields leaves and type-shaped canonical forms only.
pub fn well_typed_exp_with_depth(seed: u64, depth: u32) -> Exp {
    Gen::new(seed).program(depth)
}

/// An arbitrary program synthesising exactly `ty` in the empty context.
pub fn well_typed_exp_of_ty(seed: u64, ty: &Ty, depth: u32) -> Exp {
    Gen::new(seed).exp_syn(&[], ty, depth)
}

/// The number of nodes in an expression. Handy for sizing assertions and
/// for the benchmark harness.
pub fn size(exp: &Exp) -> usize {
    match exp {
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::EmptyHole(_) => 1,
        Exp::Lam(_, _, body) => 1 + size(body),
        Exp::Ap(f, a) => 1 + size(f) + size(a),
        Exp::BinOp(_, l, r) => 1 + size(l) + size(r),
        Exp::If(c, t, e) => 1 + size(c) + size(t) + size(e),
        Exp::Let(_, bound, body) => 1 + size(bound) + size(body),
        Exp::Pair(l, r) => 1 + size(l) + size(r),
        Exp::Proj(_, e) => 1 + size(e),
        Exp::NonEmptyHole(_, e) => 1 + size(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::ctx::Ctx;
    use nothing_core::typing::{is_well_typed, syn};
    use proptest::prelude::*;

    #[test]
    fn ten_thousand_generated_programs_are_well_typed() {
        for seed in 0..10_000u64 {
            let e = well_typed_exp(seed);
            assert!(
                is_well_typed(&e),
                "seed {seed} produced an ill-typed program: {e:?}"
            );
        }
    }

    /// The stronger invariant the whole construction rests on: the program
    /// synthesises *exactly* the type it was asked for.
    #[test]
    fn generated_programs_synthesise_exactly_the_requested_type() {
        for seed in 0..2_000u64 {
            let mut g = Gen::new(seed);
            let ty = g.ty(2);
            let e = g.exp_syn(&[], &ty, DEFAULT_DEPTH);
            assert_eq!(
                syn(&Ctx::empty(), &e),
                Some(ty.clone()),
                "seed {seed}, type {ty}: {e:?}"
            );
        }
    }

    #[test]
    fn generator_is_deterministic() {
        assert_eq!(well_typed_exp(1234), well_typed_exp(1234));
    }

    #[test]
    fn generated_programs_are_not_all_trivial() {
        let sizes: Vec<usize> = (0..500u64).map(|s| size(&well_typed_exp(s))).collect();
        let max = *sizes.iter().max().unwrap();
        let mean = sizes.iter().sum::<usize>() as f64 / sizes.len() as f64;
        assert!(max >= 10, "generator never produced a program of ten nodes (max {max})");
        assert!(mean >= 2.0, "generated programs are trivially small (mean {mean})");
    }

    /// Holes must actually show up, otherwise the generator is not
    /// exercising the part of the calculus this project exists for.
    #[test]
    fn generated_programs_contain_holes_of_both_kinds() {
        fn count(e: &Exp, empty: &mut usize, non_empty: &mut usize) {
            match e {
                Exp::EmptyHole(_) => *empty += 1,
                Exp::NonEmptyHole(_, inner) => {
                    *non_empty += 1;
                    count(inner, empty, non_empty);
                }
                Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) => {}
                Exp::Lam(_, _, b) => count(b, empty, non_empty),
                Exp::Ap(f, a) => {
                    count(f, empty, non_empty);
                    count(a, empty, non_empty);
                }
                Exp::BinOp(_, l, r) | Exp::Pair(l, r) | Exp::Let(_, l, r) => {
                    count(l, empty, non_empty);
                    count(r, empty, non_empty);
                }
                Exp::If(c, t, el) => {
                    count(c, empty, non_empty);
                    count(t, empty, non_empty);
                    count(el, empty, non_empty);
                }
                Exp::Proj(_, e) => count(e, empty, non_empty),
            }
        }

        let (mut empty, mut non_empty) = (0, 0);
        for seed in 0..500u64 {
            count(&well_typed_exp(seed), &mut empty, &mut non_empty);
        }
        assert!(empty > 0, "no empty holes generated");
        assert!(non_empty > 0, "no non-empty holes generated");
    }

    proptest! {
        #[test]
        fn arbitrary_seeds_give_well_typed_programs(seed in any::<u64>()) {
            let e = well_typed_exp(seed);
            prop_assert!(is_well_typed(&e), "{:?}", e);
        }

        #[test]
        fn arbitrary_depths_give_well_typed_programs(seed in any::<u64>(), depth in 0u32..5) {
            let e = well_typed_exp_with_depth(seed, depth);
            prop_assert!(is_well_typed(&e), "{:?}", e);
        }
    }
}
