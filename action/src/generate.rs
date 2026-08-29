use nothing_core::doc::{Def, Doc, MAIN_NAME};
use nothing_core::exp::{Exp, HoleId, Id, Op, Side, UuidStream};
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;

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

    pub fn below(&mut self, n: usize) -> usize {
        assert!(n > 0, "Rng::below(0)");
        (self.next_u64() % (n as u64)) as usize
    }

    pub fn boolean(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }

    pub fn small_int(&mut self) -> i64 {
        (self.next_u64() % 21) as i64 - 10
    }

    pub fn text(&mut self) -> String {
        const PIECES: [&str; 8] = ["", "a", "hi", "hello", "the ", "x y", "\"", "\\"];
        let mut out = String::new();
        for _ in 0..=self.below(3) {
            out.push_str(PIECES[self.below(PIECES.len())]);
        }
        out
    }

    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Form {
    Canonical,
    Var,
    Let,
    If,
    Ap,
    Proj,
    BinOp,
    NonEmptyHole,
}

#[derive(Clone, Debug)]
pub struct Gen {
    rng: Rng,
    ids: UuidStream,
}

impl Gen {
    pub fn new(seed: u64) -> Gen {
        Gen {
            rng: Rng::new(seed),
            ids: UuidStream::new((seed as u128) << 64 | 0x5EED),
        }
    }

    pub fn rng(&mut self) -> &mut Rng {
        &mut self.rng
    }

    fn fresh_id(&mut self) -> Id {
        self.ids.next_id()
    }

    fn fresh_hole(&mut self) -> HoleId {
        self.ids.next_hole_id()
    }

    pub fn ty(&mut self, depth: u32) -> Ty {
        let n = if depth == 0 { 4 } else { 6 };
        match self.rng.below(n) {
            0 => Ty::Num,
            1 => Ty::Bool,
            2 => Ty::Str,
            3 => Ty::Hole,
            4 => Ty::Arrow(Box::new(self.ty(depth - 1)), Box::new(self.ty(depth - 1))),
            _ => Ty::Prod(Box::new(self.ty(depth - 1)), Box::new(self.ty(depth - 1))),
        }
    }

    pub fn document(&mut self, count: usize, depth: u32) -> (Doc, NameTable) {
        let count = count.max(1);
        let heads: Vec<(Id, Ty)> = (0..count)
            .map(|_| {
                let id = self.fresh_id();
                let ty = self.ty(2);
                (id, ty)
            })
            .collect();
        let ctx: Vec<(Id, Ty)> = heads.clone();
        let defs: Vec<Def> = heads
            .iter()
            .map(|(id, ty)| {
                let body = self.exp_syn(&ctx, ty, depth);
                Def::new(*id, ty.clone(), body)
            })
            .collect();
        let mut names = NameTable::new();
        for (i, (id, _)) in heads.iter().enumerate() {
            if i == 0 {
                names.set(*id, MAIN_NAME);
            } else {
                names.set(*id, format!("d{i}"));
            }
        }
        let doc = Doc::new(defs).expect("the generator never repeats a definition id");
        (doc, names)
    }

    pub fn program(&mut self, depth: u32) -> Exp {
        let ty = self.ty(2);
        self.exp_syn(&[], &ty, depth)
    }

    pub fn exp_syn(&mut self, ctx: &[(Id, Ty)], ty: &Ty, depth: u32) -> Exp {
        let vars: Vec<Id> = ctx
            .iter()
            .filter(|(_, t)| t == ty)
            .map(|(id, _)| *id)
            .collect();

        let mut cands = vec![Form::Canonical];
        if !vars.is_empty() {
            cands.push(Form::Var);
            cands.push(Form::Var);
        }
        if depth > 0 {
            cands.push(Form::Let);
            cands.push(Form::If);
            cands.push(Form::Ap);
            cands.push(Form::Proj);
            if *ty == Ty::Num || *ty == Ty::Bool || *ty == Ty::Str {
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
            Form::Canonical => match ty {
                Ty::Num => Exp::num(self.rng.small_int()),
                Ty::Bool => Exp::bool_(self.rng.boolean()),
                Ty::Str => Exp::str_(self.rng.text()),
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

            Form::Var => Exp::var(*self.rng.pick(&vars)),

            Form::Let => {
                let sigma = self.ty(1);
                let bound = self.exp_syn(ctx, &sigma, d);
                let id = self.fresh_id();
                let mut inner = ctx.to_vec();
                inner.push((id, sigma));
                let body = self.exp_syn(&inner, ty, d);
                Exp::let_(id, bound, body)
            }

            Form::If => {
                let cond = self.exp_ana(ctx, &Ty::Bool, d);
                let then = self.exp_syn(ctx, ty, d);
                let else_ = self.exp_syn(ctx, ty, d);
                Exp::if_(cond, then, else_)
            }

            Form::Ap => {
                let sigma = self.ty(1);
                let fun_ty = Ty::Arrow(Box::new(sigma.clone()), Box::new(ty.clone()));
                let fun = self.exp_syn(ctx, &fun_ty, d);
                let arg = self.exp_ana(ctx, &sigma, d);
                Exp::ap(fun, arg)
            }

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

            Form::BinOp => {
                let op = match ty {
                    Ty::Num => *self.rng.pick(&[Op::Add, Op::Sub, Op::Mul]),
                    Ty::Str => Op::Concat,
                    _ => *self.rng.pick(&[Op::Lt, Op::Eq]),
                };
                let operand = match op {
                    Op::Concat => Ty::Str,
                    Op::Eq => self.rng.pick(&[Ty::Num, Ty::Bool, Ty::Str]).clone(),
                    _ => Ty::Num,
                };
                let lhs = self.exp_ana(ctx, &operand, d);
                let rhs = self.exp_ana(ctx, &operand, d);
                Exp::bin_op(op, lhs, rhs)
            }

            Form::NonEmptyHole => {
                let sigma = self.ty(1);
                let inner = self.exp_syn(ctx, &sigma, d);
                Exp::non_empty_hole(self.fresh_hole(), inner)
            }
        }
    }

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

pub const DEFAULT_DEPTH: u32 = 3;

pub fn well_typed_exp(seed: u64) -> Exp {
    well_typed_exp_with_depth(seed, DEFAULT_DEPTH)
}

pub fn well_typed_exp_with_depth(seed: u64, depth: u32) -> Exp {
    Gen::new(seed).program(depth)
}

pub fn well_typed_doc(seed: u64) -> (Doc, NameTable) {
    let mut g = Gen::new(seed);
    let count = 1 + g.rng().below(4);
    g.document(count, DEFAULT_DEPTH)
}

pub fn well_typed_exp_of_ty(seed: u64, ty: &Ty, depth: u32) -> Exp {
    Gen::new(seed).exp_syn(&[], ty, depth)
}

pub fn size(exp: &Exp) -> usize {
    match exp {
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::Str(_) | Exp::EmptyHole(_) => 1,
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
        assert!(
            max >= 10,
            "generator never produced a program of ten nodes (max {max})"
        );
        assert!(
            mean >= 2.0,
            "generated programs are trivially small (mean {mean})"
        );
    }

    #[test]
    fn generated_programs_contain_holes_of_both_kinds() {
        fn count(e: &Exp, empty: &mut usize, non_empty: &mut usize) {
            match e {
                Exp::EmptyHole(_) => *empty += 1,
                Exp::NonEmptyHole(_, inner) => {
                    *non_empty += 1;
                    count(inner, empty, non_empty);
                }
                Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::Str(_) => {}
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
