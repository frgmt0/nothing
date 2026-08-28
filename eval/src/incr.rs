use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use nothing_core::exp::{Exp, HoleId, Id, Op, Side};
use nothing_core::ty::Ty;
use nothing_store::nodes::{Digest, NodeEntry, build_node_table, content_hash};

use crate::dynamic::{Dyn, Env, elaborate};
use crate::step::{Blocked, HoleKind, Outcome};

pub type IncrEnv = im::HashMap<Id, (Value, Digest)>;

#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Var(Id),
    Closure(Id, Ty, Arc<Exp>, IncrEnv),
    Ap(Box<Value>, Box<Value>),
    Num(i64),
    Bool(bool),
    BinOp(Op, Box<Value>, Box<Value>),
    If(Box<Value>, Arc<Exp>, Arc<Exp>),
    Pair(Box<Value>, Box<Value>),
    Proj(Side, Box<Value>),
    EmptyHole(HoleId, IncrEnv),
    NonEmptyHole(HoleId, IncrEnv, Box<Value>),
}

fn is_fully_reduced(v: &Value) -> bool {
    match v {
        Value::Num(_) | Value::Bool(_) | Value::Closure(..) => true,
        Value::Pair(a, b) => is_fully_reduced(a) && is_fully_reduced(b),
        _ => false,
    }
}

fn env_to_dyn_env(env: &IncrEnv) -> Env {
    env.iter().map(|(id, (v, _))| (*id, value_to_dyn(v))).collect()
}

pub fn value_to_dyn(v: &Value) -> Dyn {
    match v {
        Value::Var(id) => Dyn::Var(*id),
        Value::Closure(id, ty, body, _env) => Dyn::Lam(*id, ty.clone(), Box::new(elaborate(body))),
        Value::Ap(f, a) => Dyn::Ap(Box::new(value_to_dyn(f)), Box::new(value_to_dyn(a))),
        Value::Num(n) => Dyn::Num(*n),
        Value::Bool(b) => Dyn::Bool(*b),
        Value::BinOp(op, l, r) => {
            Dyn::BinOp(*op, Box::new(value_to_dyn(l)), Box::new(value_to_dyn(r)))
        }
        Value::If(c, t, e) => Dyn::If(
            Box::new(value_to_dyn(c)),
            Box::new(elaborate(t)),
            Box::new(elaborate(e)),
        ),
        Value::Pair(l, r) => Dyn::Pair(Box::new(value_to_dyn(l)), Box::new(value_to_dyn(r))),
        Value::Proj(side, inner) => Dyn::Proj(*side, Box::new(value_to_dyn(inner))),
        Value::EmptyHole(h, env) => Dyn::EmptyHole(*h, env_to_dyn_env(env)),
        Value::NonEmptyHole(h, env, inner) => {
            Dyn::NonEmptyHole(*h, env_to_dyn_env(env), Box::new(value_to_dyn(inner)))
        }
    }
}

fn collect_blocked(v: &Value, out: &mut Vec<Blocked>) {
    match v {
        Value::EmptyHole(h, env) => out.push(Blocked {
            hole: *h,
            kind: HoleKind::Empty,
            env: env_to_dyn_env(env),
        }),
        Value::NonEmptyHole(h, env, inner) => {
            out.push(Blocked {
                hole: *h,
                kind: HoleKind::NonEmpty,
                env: env_to_dyn_env(env),
            });
            collect_blocked(inner, out);
        }
        Value::Ap(a, b) | Value::BinOp(_, a, b) | Value::Pair(a, b) => {
            collect_blocked(a, out);
            collect_blocked(b, out);
        }
        Value::If(c, _, _) => collect_blocked(c, out),
        Value::Proj(_, inner) => collect_blocked(inner, out),
        Value::Var(_) | Value::Num(_) | Value::Bool(_) | Value::Closure(..) => {}
    }
}

fn apply_op(op: Op, a: i64, b: i64) -> Option<Value> {
    Some(match op {
        Op::Add => Value::Num(a.checked_add(b)?),
        Op::Sub => Value::Num(a.checked_sub(b)?),
        Op::Mul => Value::Num(a.checked_mul(b)?),
        Op::Lt => Value::Bool(a < b),
        Op::Eq => Value::Bool(a == b),
    })
}

fn free_vars(exp: &Exp) -> HashSet<Id> {
    fn go(exp: &Exp, bound: &mut Vec<Id>, out: &mut HashSet<Id>) {
        match exp {
            Exp::Var(id) => {
                if !bound.contains(id) {
                    out.insert(*id);
                }
            }
            Exp::Lam(id, _, body) => {
                bound.push(*id);
                go(body, bound, out);
                bound.pop();
            }
            Exp::Let(id, bound_e, body) => {
                go(bound_e, bound, out);
                bound.push(*id);
                go(body, bound, out);
                bound.pop();
            }
            Exp::Ap(f, a) => {
                go(f, bound, out);
                go(a, bound, out);
            }
            Exp::BinOp(_, l, r) | Exp::Pair(l, r) => {
                go(l, bound, out);
                go(r, bound, out);
            }
            Exp::If(c, t, e) => {
                go(c, bound, out);
                go(t, bound, out);
                go(e, bound, out);
            }
            Exp::Proj(_, e) | Exp::NonEmptyHole(_, e) => go(e, bound, out),
            Exp::Num(_) | Exp::Bool(_) | Exp::EmptyHole(_) => {}
        }
    }
    let mut out = HashSet::new();
    let mut bound = Vec::new();
    go(exp, &mut bound, &mut out);
    out
}

fn combine(a: Digest, b: Digest) -> Digest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&a);
    hasher.update(&b);
    *hasher.finalize().as_bytes()
}

fn env_fingerprint(exp: &Exp, env: &IncrEnv) -> Digest {
    let free = free_vars(exp);
    let mut relevant: Vec<(Id, Digest)> = env
        .iter()
        .filter(|(id, _)| free.contains(id))
        .map(|(id, (_, dig))| (*id, *dig))
        .collect();
    relevant.sort_by_key(|(id, _)| *id);
    let mut hasher = blake3::Hasher::new();
    for (id, dig) in &relevant {
        hasher.update(id.uuid().as_bytes());
        hasher.update(dig);
    }
    *hasher.finalize().as_bytes()
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct CacheKey {
    node_hash: Digest,
    env_fp: Digest,
}

pub struct IncrEngine {
    cache: HashMap<CacheKey, Value>,
    pub node_evals: usize,
    fuel: usize,
    fuel_budget: usize,
    exhausted: bool,
}

impl Default for IncrEngine {
    fn default() -> IncrEngine {
        IncrEngine::new()
    }
}

impl IncrEngine {
    pub fn new() -> IncrEngine {
        IncrEngine {
            cache: HashMap::new(),
            node_evals: 0,
            fuel: 0,
            fuel_budget: 0,
            exhausted: false,
        }
    }

    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    pub fn invalidate(&mut self, dirty: &HashSet<Digest>) {
        self.cache.retain(|key, _| !dirty.contains(&key.node_hash));
    }

    pub fn eval_with_fuel(&mut self, exp: &Exp, fuel: usize) -> Outcome {
        self.eval_in_with_fuel(exp, &IncrEnv::new(), fuel)
    }

    pub fn eval_in_with_fuel(&mut self, exp: &Exp, env: &IncrEnv, fuel: usize) -> Outcome {
        self.fuel = fuel;
        self.fuel_budget = fuel;
        self.exhausted = false;
        let value = std::thread::scope(|scope| {
            std::thread::Builder::new()
                .stack_size(64 * 1024 * 1024)
                .spawn_scoped(scope, || self.eval_node(exp, env).0)
                .expect("failed to spawn the incremental evaluator thread")
                .join()
                .expect("the incremental evaluator thread panicked")
        });
        if self.exhausted {
            return Outcome::OutOfFuel {
                partial: value_to_dyn(&value),
                steps: self.fuel_budget,
            };
        }
        if is_fully_reduced(&value) {
            Outcome::Value(value_to_dyn(&value))
        } else {
            let mut blocked = Vec::new();
            collect_blocked(&value, &mut blocked);
            Outcome::Indeterminate {
                result: value_to_dyn(&value),
                blocked,
            }
        }
    }

    fn eval_node(&mut self, exp: &Exp, env: &IncrEnv) -> (Value, Digest) {
        let node_hash = content_hash(exp);
        let env_fp = env_fingerprint(exp, env);
        let key = CacheKey { node_hash, env_fp };
        let digest = combine(node_hash, env_fp);
        if let Some(v) = self.cache.get(&key) {
            return (v.clone(), digest);
        }
        self.node_evals += 1;
        let value = self.eval_uncached(exp, env);
        if !self.exhausted {
            self.cache.insert(key, value.clone());
        }
        (value, digest)
    }

    fn eval_uncached(&mut self, exp: &Exp, env: &IncrEnv) -> Value {
        match exp {
            Exp::Var(id) => match env.get(id) {
                Some((v, _)) => v.clone(),
                None => Value::Var(*id),
            },
            Exp::Num(n) => Value::Num(*n),
            Exp::Bool(b) => Value::Bool(*b),
            Exp::Lam(id, ty, body) => {
                Value::Closure(*id, ty.clone(), Arc::new((**body).clone()), env.clone())
            }
            Exp::Ap(f, a) => {
                let (vf, _) = self.eval_node(f, env);
                let (va, va_dig) = self.eval_node(a, env);
                match vf {
                    Value::Closure(id, ty, body, cenv) => {
                        if self.fuel == 0 {
                            self.exhausted = true;
                            Value::Ap(
                                Box::new(Value::Closure(id, ty, body, cenv)),
                                Box::new(va),
                            )
                        } else {
                            self.fuel -= 1;
                            let inner_env = cenv.update(id, (va, va_dig));
                            self.eval_node(&body, &inner_env).0
                        }
                    }
                    other => Value::Ap(Box::new(other), Box::new(va)),
                }
            }
            Exp::BinOp(op, l, r) => {
                let (vl, _) = self.eval_node(l, env);
                let (vr, _) = self.eval_node(r, env);
                match (&vl, &vr) {
                    (Value::Num(a), Value::Num(b)) => match apply_op(*op, *a, *b) {
                        Some(v) => v,
                        None => Value::BinOp(*op, Box::new(vl), Box::new(vr)),
                    },
                    _ => Value::BinOp(*op, Box::new(vl), Box::new(vr)),
                }
            }
            Exp::If(cond, then, else_) => {
                let (vc, _) = self.eval_node(cond, env);
                match &vc {
                    Value::Bool(true) => self.eval_node(then, env).0,
                    Value::Bool(false) => self.eval_node(else_, env).0,
                    _ => Value::If(Box::new(vc), Arc::new((**then).clone()), Arc::new((**else_).clone())),
                }
            }
            Exp::Let(id, bound, body) => {
                let (vb, db) = self.eval_node(bound, env);
                let inner_env = env.update(*id, (vb, db));
                self.eval_node(body, &inner_env).0
            }
            Exp::Pair(l, r) => {
                let (vl, _) = self.eval_node(l, env);
                let (vr, _) = self.eval_node(r, env);
                Value::Pair(Box::new(vl), Box::new(vr))
            }
            Exp::Proj(side, inner) => {
                let (vi, _) = self.eval_node(inner, env);
                match &vi {
                    Value::Pair(a, b) => (**match side {
                        Side::L => a,
                        Side::R => b,
                    })
                    .clone(),
                    _ => Value::Proj(*side, Box::new(vi)),
                }
            }
            Exp::EmptyHole(h) => Value::EmptyHole(*h, env.clone()),
            Exp::NonEmptyHole(h, inner) => {
                let (vi, _) = self.eval_node(inner, env);
                Value::NonEmptyHole(*h, env.clone(), Box::new(vi))
            }
        }
    }
}

pub struct DepGraph {
    dependents: HashMap<Digest, HashSet<Digest>>,
    pub root: Digest,
}

impl DepGraph {
    pub fn build(exp: &Exp) -> DepGraph {
        let table = build_node_table(exp);
        let mut dependents: HashMap<Digest, HashSet<Digest>> = HashMap::new();
        for entry in &table {
            for &child_idx in &entry.children {
                let child_hash = table[child_idx as usize].hash;
                dependents.entry(child_hash).or_default().insert(entry.hash);
            }
        }
        let mut scope: Vec<(Id, Digest)> = Vec::new();
        let mut idx = 0usize;
        walk_with_table(exp, &table, &mut idx, &mut scope, &mut dependents);
        debug_assert_eq!(idx, table.len());
        let root = table
            .last()
            .expect("build_node_table produces at least one node")
            .hash;
        DepGraph { dependents, root }
    }

    pub fn dependents_of(&self, hash: Digest) -> Vec<Digest> {
        self.dependents
            .get(&hash)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn transitive_dependents(&self, hash: Digest) -> HashSet<Digest> {
        let mut seen = HashSet::new();
        let mut stack = vec![hash];
        while let Some(h) = stack.pop() {
            if let Some(direct) = self.dependents.get(&h) {
                for &d in direct {
                    if seen.insert(d) {
                        stack.push(d);
                    }
                }
            }
        }
        seen
    }
}

fn next_hash(table: &[NodeEntry], idx: &mut usize) -> Digest {
    let hash = table[*idx].hash;
    *idx += 1;
    hash
}

fn walk_with_table(
    exp: &Exp,
    table: &[NodeEntry],
    idx: &mut usize,
    scope: &mut Vec<(Id, Digest)>,
    dependents: &mut HashMap<Digest, HashSet<Digest>>,
) -> Digest {
    match exp {
        Exp::Var(id) => {
            let hash = next_hash(table, idx);
            if let Some((_, bound_hash)) = scope.iter().rev().find(|(bid, _)| bid == id) {
                dependents.entry(*bound_hash).or_default().insert(hash);
            }
            hash
        }
        Exp::Num(_) | Exp::Bool(_) | Exp::EmptyHole(_) => next_hash(table, idx),
        Exp::Lam(id, _, body) => {
            let mut inner: Vec<(Id, Digest)> =
                scope.iter().filter(|(bid, _)| bid != id).cloned().collect();
            walk_with_table(body, table, idx, &mut inner, dependents);
            next_hash(table, idx)
        }
        Exp::Ap(a, b) | Exp::BinOp(_, a, b) | Exp::Pair(a, b) => {
            walk_with_table(a, table, idx, scope, dependents);
            walk_with_table(b, table, idx, scope, dependents);
            next_hash(table, idx)
        }
        Exp::If(c, t, e) => {
            walk_with_table(c, table, idx, scope, dependents);
            walk_with_table(t, table, idx, scope, dependents);
            walk_with_table(e, table, idx, scope, dependents);
            next_hash(table, idx)
        }
        Exp::Let(id, bound, body) => {
            let bound_hash = walk_with_table(bound, table, idx, scope, dependents);
            scope.push((*id, bound_hash));
            walk_with_table(body, table, idx, scope, dependents);
            scope.pop();
            next_hash(table, idx)
        }
        Exp::Proj(_, e) | Exp::NonEmptyHole(_, e) => {
            walk_with_table(e, table, idx, scope, dependents);
            next_hash(table, idx)
        }
    }
}

pub fn dirty_set(old_exp: &Exp, changed_hash: Digest) -> HashSet<Digest> {
    let graph = DepGraph::build(old_exp);
    let mut dirty = graph.transitive_dependents(changed_hash);
    dirty.insert(changed_hash);
    dirty
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_action::act::{Action, EditState};
    use nothing_core::names::NameTable;
    use nothing_core::ty::Ty;

    fn balanced_sum_tree(depth: u32, counter: &mut i64) -> Exp {
        if depth == 0 {
            let n = *counter;
            *counter += 1;
            Exp::num(n)
        } else {
            let l = balanced_sum_tree(depth - 1, counter);
            let r = balanced_sum_tree(depth - 1, counter);
            Exp::bin_op(Op::Add, l, r)
        }
    }

    #[test]
    fn dependents_of_a_lets_bound_expression_include_every_occurrence_transitively() {
        let x = Id::from_u128(1);
        let program = Exp::let_(
            x,
            Exp::num(5),
            Exp::pair(
                Exp::bin_op(Op::Add, Exp::var(x), Exp::num(1)),
                Exp::bin_op(Op::Mul, Exp::var(x), Exp::num(2)),
            ),
        );
        let graph = DepGraph::build(&program);

        let bound_hash = content_hash(&Exp::num(5));

        let direct = graph.dependents_of(bound_hash);
        assert_eq!(
            direct.len(),
            2,
            "the let node depends on its own bound child, and both (structurally identical) \
             occurrences of x collapse to one further dependent: {direct:?}"
        );
        assert!(
            direct.contains(&graph.root),
            "the let node itself is a direct dependent of its own bound expression"
        );
        let var_hash = *direct
            .iter()
            .find(|h| **h != graph.root)
            .expect("a dependent other than the let node itself");

        let var_dependents = graph.dependents_of(var_hash);
        assert_eq!(
            var_dependents.len(),
            2,
            "x is used inside both arithmetic expressions: {var_dependents:?}"
        );

        let transitive = graph.transitive_dependents(bound_hash);
        assert!(transitive.contains(&var_hash));
        assert!(transitive.contains(&graph.root), "editing the binding must dirty the whole program");
        assert_eq!(
            transitive.len(),
            5,
            "the variable occurrence, the two arithmetic expressions, their enclosing pair, \
             and the root let: {transitive:?}"
        );
    }

    #[test]
    fn a_leaf_with_no_dependents_only_dirties_its_ancestors() {
        let mut counter = 0i64;
        let program = balanced_sum_tree(3, &mut counter);
        let graph = DepGraph::build(&program);

        let leaf_hash = content_hash(&Exp::num(0));
        let dirty = graph.transitive_dependents(leaf_hash);

        assert!(dirty.contains(&graph.root));
        let unrelated_leaf_hash = content_hash(&Exp::num(7));
        assert!(!dirty.contains(&unrelated_leaf_hash));
    }

    #[test]
    fn editing_a_leaf_in_a_hundred_node_program_reevaluates_under_ten_nodes() {
        let mut counter = 0i64;
        let program = balanced_sum_tree(6, &mut counter);
        assert_eq!(nothing_action::generate::size(&program), 127);

        let mut engine = IncrEngine::new();
        let before = engine.eval_with_fuel(&program, 10_000);
        assert!(before.is_value());
        let baseline = engine.node_evals;
        assert_eq!(baseline, 127, "a cold cache evaluates every node once");

        let leaf_hash = content_hash(&Exp::num(0));
        let dirty = dirty_set(&program, leaf_hash);
        assert!(
            dirty.len() < 10,
            "the dependency graph says {} nodes are dirty, expected under 10",
            dirty.len()
        );
        engine.invalidate(&dirty);
        assert_eq!(engine.cache_len(), baseline - dirty.len());

        let mut state = EditState::new(program.clone());
        for _ in 0..6 {
            assert!(state.apply_mut(Action::MoveChild(0)));
        }
        assert!(matches!(state.zipper.focus, Exp::Num(0)));
        assert!(state.apply_mut(Action::Delete));
        assert!(state.apply_mut(Action::ConstructNum(999)));
        for _ in 0..6 {
            assert!(state.apply_mut(Action::MoveParent));
        }
        let edited = state.exp();
        assert_ne!(edited, program);
        assert_eq!(nothing_action::generate::size(&edited), 127);

        let after = engine.eval_with_fuel(&edited, 10_000);
        assert!(after.is_value());
        let delta = engine.node_evals - baseline;
        assert!(
            delta < 10,
            "editing one leaf re-evaluated {delta} nodes, expected fewer than 10"
        );
        assert!(delta > 0, "the edited path must actually be recomputed");
    }

    #[test]
    fn renaming_a_variable_causes_zero_reevaluation() {
        let x = Id::from_u128(42);
        let mut names = NameTable::new();
        names.set(x, "x");
        let program = Exp::let_(x, Exp::num(5), Exp::bin_op(Op::Add, Exp::var(x), Exp::num(1)));
        let mut state = EditState::with_names(program, names);

        let mut engine = IncrEngine::new();
        let before_exp = state.exp();
        let outcome1 = engine.eval_with_fuel(&before_exp, 10_000);
        assert_eq!(outcome1.num(), Some(6));
        let baseline = engine.node_evals;
        assert!(baseline > 0);

        assert!(state.apply_mut(Action::Rename(x, "renamed".to_string())));
        let after_exp = state.exp();
        assert_eq!(after_exp, before_exp, "rename must not touch the AST");

        let outcome2 = engine.eval_with_fuel(&after_exp, 10_000);
        assert_eq!(outcome2.num(), Some(6));
        assert_eq!(
            engine.node_evals, baseline,
            "renaming must not trigger a single re-evaluation"
        );
    }

    #[test]
    fn the_same_body_evaluated_under_two_different_bindings_does_not_reuse_a_stale_value() {
        let x = Id::from_u128(7);
        let call = |arg: i64| {
            Exp::ap(
                Exp::lam(x, Ty::Num, Exp::bin_op(Op::Add, Exp::var(x), Exp::num(1))),
                Exp::num(arg),
            )
        };
        let program = Exp::pair(call(5), call(10));

        let mut engine = IncrEngine::new();
        let outcome = engine.eval_with_fuel(&program, 10_000);
        match outcome.dyn_result() {
            Dyn::Pair(a, b) => {
                assert_eq!(**a, Dyn::Num(6), "the first call sees x = 5");
                assert_eq!(**b, Dyn::Num(11), "the second call sees x = 10, not a cached 6");
            }
            other => panic!("expected a pair of numbers, got {other:?}"),
        }
    }

    #[test]
    fn a_runaway_self_application_exhausts_its_fuel_instead_of_looping_forever() {
        let x = Id::from_u128(0x77);
        let omega = Exp::lam(x, Ty::Hole, Exp::ap(Exp::var(x), Exp::var(x)));
        let program = Exp::ap(omega.clone(), omega);

        let mut engine = IncrEngine::new();
        let outcome = engine.eval_with_fuel(&program, 4_000);
        match outcome {
            Outcome::OutOfFuel { steps, .. } => assert_eq!(steps, 4_000),
            other => panic!("expected exhaustion, got {other:?}"),
        }
    }
}
