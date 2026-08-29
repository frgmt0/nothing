use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use nothing_core::doc::Doc;
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
    Str(String),
    BinOp(Op, Box<Value>, Box<Value>),
    If(Box<Value>, Arc<Exp>, Arc<Exp>),
    Pair(Box<Value>, Box<Value>),
    Proj(Side, Box<Value>),
    Nil,
    Cons(Box<Value>, Box<Value>),
    Fold(Box<Value>, Box<Value>, Box<Value>),
    Record(Vec<(Id, Value)>),
    Field(Box<Value>, Id),
    Inj(Id, Box<Value>),
    Match(Box<Value>, Vec<(Id, Id, Arc<Exp>)>),
    Print(Box<Value>),
    Readline,
    CmdPure(Box<Value>),
    CmdBind(Box<Value>, Id, Arc<Exp>, IncrEnv),
    EmptyHole(HoleId, IncrEnv),
    NonEmptyHole(HoleId, IncrEnv, Box<Value>),
}

fn is_fully_reduced(v: &Value) -> bool {
    match v {
        Value::Num(_)
        | Value::Bool(_)
        | Value::Str(_)
        | Value::Closure(..)
        | Value::Readline
        | Value::Nil => true,
        Value::Pair(a, b) | Value::Cons(a, b) => is_fully_reduced(a) && is_fully_reduced(b),
        Value::Record(fields) => fields.iter().all(|(_, value)| is_fully_reduced(value)),
        Value::Inj(_, payload) => is_fully_reduced(payload),
        Value::Print(text) => is_fully_reduced(text),
        Value::CmdPure(value) => is_fully_reduced(value),
        Value::CmdBind(command, _, _, _) => is_fully_reduced(command),
        _ => false,
    }
}

fn env_to_dyn_env(env: &IncrEnv) -> Env {
    env.iter()
        .map(|(id, (v, _))| (*id, value_to_dyn(v)))
        .collect()
}

pub fn value_to_dyn(v: &Value) -> Dyn {
    match v {
        Value::Var(id) => Dyn::Var(*id),
        Value::Closure(id, ty, body, _env) => Dyn::Lam(*id, ty.clone(), Box::new(elaborate(body))),
        Value::Ap(f, a) => Dyn::Ap(Box::new(value_to_dyn(f)), Box::new(value_to_dyn(a))),
        Value::Num(n) => Dyn::Num(*n),
        Value::Bool(b) => Dyn::Bool(*b),
        Value::Str(text) => Dyn::Str(text.clone()),
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
        Value::Nil => Dyn::Nil,
        Value::Cons(head, tail) => {
            Dyn::Cons(Box::new(value_to_dyn(head)), Box::new(value_to_dyn(tail)))
        }
        Value::Fold(list, init, step) => Dyn::Fold(
            Box::new(value_to_dyn(list)),
            Box::new(value_to_dyn(init)),
            Box::new(value_to_dyn(step)),
        ),
        Value::Record(fields) => Dyn::Record(
            fields
                .iter()
                .map(|(id, value)| (*id, value_to_dyn(value)))
                .collect(),
        ),
        Value::Field(subject, id) => Dyn::Field(Box::new(value_to_dyn(subject)), *id),
        Value::Inj(ctor, payload) => Dyn::Inj(*ctor, Box::new(value_to_dyn(payload))),
        Value::Match(scrutinee, arms) => Dyn::Match(
            Box::new(value_to_dyn(scrutinee)),
            arms.iter()
                .map(|(ctor, binder, body)| (*ctor, *binder, elaborate(body)))
                .collect(),
        ),
        Value::Print(text) => Dyn::Print(Box::new(value_to_dyn(text))),
        Value::Readline => Dyn::Readline,
        Value::CmdPure(value) => Dyn::CmdPure(Box::new(value_to_dyn(value))),
        Value::CmdBind(command, id, body, _env) => Dyn::CmdBind(
            Box::new(value_to_dyn(command)),
            *id,
            Box::new(elaborate(body)),
        ),
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
        Value::Ap(a, b) | Value::BinOp(_, a, b) | Value::Pair(a, b) | Value::Cons(a, b) => {
            collect_blocked(a, out);
            collect_blocked(b, out);
        }
        Value::If(c, _, _) => collect_blocked(c, out),
        Value::Fold(list, init, step) => {
            collect_blocked(list, out);
            collect_blocked(init, out);
            collect_blocked(step, out);
        }
        Value::Proj(_, inner)
        | Value::Field(inner, _)
        | Value::Inj(_, inner)
        | Value::Print(inner)
        | Value::CmdPure(inner) => collect_blocked(inner, out),
        Value::CmdBind(command, _, _, _) => collect_blocked(command, out),
        Value::Match(scrutinee, _) => collect_blocked(scrutinee, out),
        Value::Record(fields) => {
            for (_, value) in fields {
                collect_blocked(value, out);
            }
        }
        Value::Var(_)
        | Value::Num(_)
        | Value::Bool(_)
        | Value::Str(_)
        | Value::Nil
        | Value::Readline
        | Value::Closure(..) => {}
    }
}

fn apply_num_op(op: Op, a: i64, b: i64) -> Option<Value> {
    Some(match op {
        Op::Add => Value::Num(a.checked_add(b)?),
        Op::Sub => Value::Num(a.checked_sub(b)?),
        Op::Mul => Value::Num(a.checked_mul(b)?),
        Op::Lt => Value::Bool(a < b),
        Op::Eq => Value::Bool(a == b),
        Op::Concat => return None,
    })
}

fn apply_str_op(op: Op, a: &str, b: &str) -> Option<Value> {
    match op {
        Op::Concat => Some(Value::Str(format!("{a}{b}"))),
        Op::Eq => Some(Value::Bool(a == b)),
        Op::Add | Op::Sub | Op::Mul | Op::Lt => None,
    }
}

fn apply_bool_op(op: Op, a: bool, b: bool) -> Option<Value> {
    match op {
        Op::Eq => Some(Value::Bool(a == b)),
        Op::Add | Op::Sub | Op::Mul | Op::Lt | Op::Concat => None,
    }
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
            Exp::BinOp(_, l, r) | Exp::Pair(l, r) | Exp::Cons(l, r) => {
                go(l, bound, out);
                go(r, bound, out);
            }
            Exp::If(c, t, e) | Exp::Fold(c, t, e) => {
                go(c, bound, out);
                go(t, bound, out);
                go(e, bound, out);
            }
            Exp::Proj(_, e) | Exp::Field(e, _) | Exp::Inj(_, e) | Exp::NonEmptyHole(_, e) => {
                go(e, bound, out)
            }
            Exp::Record(fields) => {
                for (_, value) in fields {
                    go(value, bound, out);
                }
            }
            Exp::Match(scrutinee, arms) => {
                go(scrutinee, bound, out);
                for (_, binder, body) in arms {
                    bound.push(*binder);
                    go(body, bound, out);
                    bound.pop();
                }
            }
            Exp::Print(e) | Exp::CmdPure(e) => go(e, bound, out),
            Exp::CmdBind(command, id, body) => {
                go(command, bound, out);
                bound.push(*id);
                go(body, bound, out);
                bound.pop();
            }
            Exp::Num(_)
            | Exp::Bool(_)
            | Exp::Str(_)
            | Exp::Nil
            | Exp::Readline
            | Exp::EmptyHole(_) => {}
        }
    }
    let mut out = HashSet::new();
    let mut bound = Vec::new();
    go(exp, &mut bound, &mut out);
    out
}

const CONS_HEAD_SALT: Digest = [0xc0; 32];
const CONS_TAIL_SALT: Digest = [0xc1; 32];

fn combine(a: Digest, b: Digest) -> Digest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&a);
    hasher.update(&b);
    *hasher.finalize().as_bytes()
}

fn env_fingerprint_with_defs(
    exp: &Exp,
    env: &IncrEnv,
    def_digests: &HashMap<Id, Digest>,
) -> Digest {
    let free = free_vars(exp);
    let mut relevant: Vec<(Id, Digest)> = env
        .iter()
        .filter(|(id, _)| free.contains(id))
        .map(|(id, (_, dig))| (*id, *dig))
        .collect();
    relevant.extend(
        free.iter()
            .filter(|id| !env.contains_key(*id))
            .filter_map(|id| def_digests.get(id).map(|dig| (*id, *dig))),
    );
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

pub fn definition_digests(doc: &Doc) -> HashMap<Id, Digest> {
    let bodies: HashMap<Id, Digest> = doc
        .defs()
        .iter()
        .map(|def| (def.id, content_hash(&def.body)))
        .collect();
    let refs: HashMap<Id, HashSet<Id>> = doc
        .defs()
        .iter()
        .map(|def| {
            let mut direct = free_vars(&def.body);
            direct.retain(|id| bodies.contains_key(id));
            (def.id, direct)
        })
        .collect();

    doc.defs()
        .iter()
        .map(|def| {
            let mut reached: HashSet<Id> = HashSet::new();
            let mut stack = vec![def.id];
            while let Some(id) = stack.pop() {
                if !reached.insert(id) {
                    continue;
                }
                if let Some(direct) = refs.get(&id) {
                    stack.extend(direct.iter().copied());
                }
            }
            let mut parts: Vec<(Id, Digest)> = reached
                .into_iter()
                .filter_map(|id| bodies.get(&id).map(|d| (id, *d)))
                .collect();
            parts.sort_by_key(|(id, _)| *id);
            let mut hasher = blake3::Hasher::new();
            for (id, dig) in &parts {
                hasher.update(id.uuid().as_bytes());
                hasher.update(dig);
            }
            (def.id, *hasher.finalize().as_bytes())
        })
        .collect()
}

pub struct IncrEngine {
    cache: HashMap<CacheKey, Value>,
    pub node_evals: usize,
    fuel: usize,
    fuel_budget: usize,
    exhausted: bool,
    defs: HashMap<Id, Arc<Exp>>,
    def_digests: HashMap<Id, Digest>,
    unfolding: HashSet<Id>,
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
            defs: HashMap::new(),
            def_digests: HashMap::new(),
            unfolding: HashSet::new(),
        }
    }

    pub fn set_document(&mut self, doc: &Doc) {
        self.defs = doc
            .defs()
            .iter()
            .map(|def| (def.id, Arc::new(def.body.clone())))
            .collect();
        self.def_digests = definition_digests(doc);
    }

    pub fn definitions(&self) -> usize {
        self.defs.len()
    }

    pub fn eval_definition(&mut self, doc: &Doc, id: Id, fuel: usize) -> Outcome {
        self.set_document(doc);
        match doc.get(id) {
            Some(def) => {
                let body = def.body.clone();
                self.eval_with_fuel(&body, fuel)
            }
            None => self.eval_with_fuel(&Exp::var(id), fuel),
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
        self.unfolding.clear();
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
        let env_fp = env_fingerprint_with_defs(exp, env, &self.def_digests);
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

    fn apply_value(&mut self, fun: Value, arg: Value, arg_digest: Digest) -> Value {
        match fun {
            Value::Closure(id, ty, body, cenv) => {
                if self.fuel == 0 {
                    self.exhausted = true;
                    Value::Ap(Box::new(Value::Closure(id, ty, body, cenv)), Box::new(arg))
                } else {
                    self.fuel -= 1;
                    let inner_env = cenv.update(id, (arg, arg_digest));
                    self.eval_node(&body, &inner_env).0
                }
            }
            other => Value::Ap(Box::new(other), Box::new(arg)),
        }
    }

    fn fold_value(
        &mut self,
        list: Value,
        list_digest: Digest,
        init: Value,
        init_digest: Digest,
        step: Value,
        step_digest: Digest,
    ) -> Value {
        let mut spine: Vec<(Value, Digest)> = Vec::new();
        let mut rest = list;
        let mut rest_digest = list_digest;
        loop {
            match rest {
                Value::Cons(head, tail) => {
                    let head_digest = combine(rest_digest, CONS_HEAD_SALT);
                    rest_digest = combine(rest_digest, CONS_TAIL_SALT);
                    spine.push((*head, head_digest));
                    rest = *tail;
                }
                Value::Nil => break,
                blocked => {
                    return Value::Fold(Box::new(blocked), Box::new(init), Box::new(step));
                }
            }
        }
        let mut acc = init;
        let mut acc_digest = init_digest;
        for (head, head_digest) in spine.into_iter().rev() {
            let partial = self.apply_value(step.clone(), head, head_digest);
            acc_digest = combine(combine(head_digest, acc_digest), step_digest);
            acc = self.apply_value(partial, acc, acc_digest);
        }
        acc
    }

    fn eval_uncached(&mut self, exp: &Exp, env: &IncrEnv) -> Value {
        match exp {
            Exp::Var(id) => match env.get(id) {
                Some((v, _)) => v.clone(),
                None => match self.defs.get(id).cloned() {
                    Some(body) if self.unfolding.insert(*id) => {
                        let value = self.eval_node(&body, &IncrEnv::new()).0;
                        self.unfolding.remove(id);
                        value
                    }
                    _ => Value::Var(*id),
                },
            },
            Exp::Num(n) => Value::Num(*n),
            Exp::Bool(b) => Value::Bool(*b),
            Exp::Str(text) => Value::Str(text.clone()),
            Exp::Lam(id, ty, body) => {
                Value::Closure(*id, ty.clone(), Arc::new((**body).clone()), env.clone())
            }
            Exp::Ap(f, a) => {
                let (vf, _) = self.eval_node(f, env);
                let (va, va_dig) = self.eval_node(a, env);
                self.apply_value(vf, va, va_dig)
            }
            Exp::BinOp(op, l, r) => {
                let (vl, _) = self.eval_node(l, env);
                let (vr, _) = self.eval_node(r, env);
                let applied = match (&vl, &vr) {
                    (Value::Num(a), Value::Num(b)) => apply_num_op(*op, *a, *b),
                    (Value::Str(a), Value::Str(b)) => apply_str_op(*op, a, b),
                    (Value::Bool(a), Value::Bool(b)) => apply_bool_op(*op, *a, *b),
                    _ => None,
                };
                match applied {
                    Some(v) => v,
                    None => Value::BinOp(*op, Box::new(vl), Box::new(vr)),
                }
            }
            Exp::If(cond, then, else_) => {
                let (vc, _) = self.eval_node(cond, env);
                match &vc {
                    Value::Bool(true) => self.eval_node(then, env).0,
                    Value::Bool(false) => self.eval_node(else_, env).0,
                    _ => Value::If(
                        Box::new(vc),
                        Arc::new((**then).clone()),
                        Arc::new((**else_).clone()),
                    ),
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
            Exp::Nil => Value::Nil,
            Exp::Cons(head, tail) => {
                let (vh, _) = self.eval_node(head, env);
                let (vt, _) = self.eval_node(tail, env);
                Value::Cons(Box::new(vh), Box::new(vt))
            }
            Exp::Fold(list, init, step) => {
                let (vl, dl) = self.eval_node(list, env);
                let (vi, di) = self.eval_node(init, env);
                let (vs, ds) = self.eval_node(step, env);
                self.fold_value(vl, dl, vi, di, vs, ds)
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
            Exp::Record(fields) => Value::Record(
                fields
                    .iter()
                    .map(|(id, value)| (*id, self.eval_node(value, env).0))
                    .collect(),
            ),
            Exp::Field(subject, id) => {
                let (vs, _) = self.eval_node(subject, env);
                let found = match &vs {
                    Value::Record(fields) => fields
                        .iter()
                        .find(|(f, _)| f == id)
                        .map(|(_, value)| value.clone()),
                    _ => None,
                };
                found.unwrap_or_else(|| Value::Field(Box::new(vs), *id))
            }
            Exp::Inj(ctor, payload) => {
                let (vp, _) = self.eval_node(payload, env);
                Value::Inj(*ctor, Box::new(vp))
            }
            Exp::Print(text) => {
                let (vt, _) = self.eval_node(text, env);
                Value::Print(Box::new(vt))
            }
            Exp::Readline => Value::Readline,
            Exp::CmdPure(value) => {
                let (vv, _) = self.eval_node(value, env);
                Value::CmdPure(Box::new(vv))
            }
            Exp::CmdBind(command, id, body) => {
                let (vc, _) = self.eval_node(command, env);
                Value::CmdBind(Box::new(vc), *id, Arc::new((**body).clone()), env.clone())
            }
            Exp::Match(scrutinee, arms) => {
                let (vs, ds) = self.eval_node(scrutinee, env);
                match &vs {
                    Value::Inj(ctor, payload) => match arms.iter().find(|(id, _, _)| id == ctor) {
                        Some((_, binder, body)) => {
                            let inner = env.update(*binder, ((**payload).clone(), ds));
                            let body = body.clone();
                            self.eval_node(&body, &inner).0
                        }
                        None => Value::Match(Box::new(vs), residual_arms(arms)),
                    },
                    _ => Value::Match(Box::new(vs), residual_arms(arms)),
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
        Exp::Num(_) | Exp::Bool(_) | Exp::Str(_) | Exp::Nil | Exp::Readline | Exp::EmptyHole(_) => {
            next_hash(table, idx)
        }
        Exp::Print(e) | Exp::CmdPure(e) => {
            walk_with_table(e, table, idx, scope, dependents);
            next_hash(table, idx)
        }
        Exp::CmdBind(command, id, body) => {
            let command_hash = walk_with_table(command, table, idx, scope, dependents);
            scope.push((*id, command_hash));
            walk_with_table(body, table, idx, scope, dependents);
            scope.pop();
            next_hash(table, idx)
        }
        Exp::Lam(id, _, body) => {
            let mut inner: Vec<(Id, Digest)> =
                scope.iter().filter(|(bid, _)| bid != id).cloned().collect();
            walk_with_table(body, table, idx, &mut inner, dependents);
            next_hash(table, idx)
        }
        Exp::Ap(a, b) | Exp::BinOp(_, a, b) | Exp::Pair(a, b) | Exp::Cons(a, b) => {
            walk_with_table(a, table, idx, scope, dependents);
            walk_with_table(b, table, idx, scope, dependents);
            next_hash(table, idx)
        }
        Exp::If(c, t, e) | Exp::Fold(c, t, e) => {
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
        Exp::Proj(_, e) | Exp::Field(e, _) | Exp::Inj(_, e) | Exp::NonEmptyHole(_, e) => {
            walk_with_table(e, table, idx, scope, dependents);
            next_hash(table, idx)
        }
        Exp::Record(fields) => {
            for (_, value) in fields {
                walk_with_table(value, table, idx, scope, dependents);
            }
            next_hash(table, idx)
        }
        Exp::Match(scrutinee, arms) => {
            walk_with_table(scrutinee, table, idx, scope, dependents);
            for (_, binder, body) in arms {
                let mut inner: Vec<(Id, Digest)> = scope
                    .iter()
                    .filter(|(bid, _)| bid != binder)
                    .cloned()
                    .collect();
                walk_with_table(body, table, idx, &mut inner, dependents);
            }
            next_hash(table, idx)
        }
    }
}

fn residual_arms(arms: &[(Id, Id, Exp)]) -> Vec<(Id, Id, Arc<Exp>)> {
    arms.iter()
        .map(|(ctor, binder, body)| (*ctor, *binder, Arc::new(body.clone())))
        .collect()
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
    fn the_incremental_engine_agrees_that_a_finished_command_is_a_value() {
        let line = Id::from_u128(0xB11D);
        let program = Exp::cmd_bind(
            Exp::readline(),
            line,
            Exp::print(Exp::bin_op(
                Op::Concat,
                Exp::str_("hello, "),
                Exp::var(line),
            )),
        );

        let mut engine = IncrEngine::new();
        let incremental = engine.eval_with_fuel(&program, 10_000);
        assert!(
            incremental.is_value(),
            "a command with nothing left to compute is a value here too: {incremental:?}"
        );

        let small_step = crate::step::eval_with_fuel(&program, 10_000);
        assert!(small_step.is_value());
        assert_eq!(
            crate::dynamic::render(incremental.dyn_result(), &NameTable::new()),
            crate::dynamic::render(small_step.dyn_result(), &NameTable::new()),
            "the two engines agree on what the command is"
        );

        let blocked = Exp::cmd_bind(
            Exp::empty_hole(nothing_core::exp::HoleId::from_u128(3)),
            line,
            Exp::print(Exp::var(line)),
        );
        assert!(
            engine.eval_with_fuel(&blocked, 10_000).is_indeterminate(),
            "and that one with a hole in the command position is not"
        );
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
        assert!(
            transitive.contains(&graph.root),
            "editing the binding must dirty the whole program"
        );
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
    fn a_string_node_evaluates_caches_and_re_evaluates_only_its_own_path() {
        let program = Exp::bin_op(
            Op::Concat,
            Exp::bin_op(Op::Concat, Exp::str_("hello"), Exp::str_(", ")),
            Exp::str_("world"),
        );
        let mut engine = IncrEngine::new();
        let outcome = engine.eval_with_fuel(&program, 10_000);
        assert!(outcome.is_value());
        assert_eq!(outcome.str(), Some("hello, world"));
        let baseline = engine.node_evals;
        assert_eq!(baseline, 5, "a cold cache evaluates every node once");

        let again = engine.eval_with_fuel(&program, 10_000);
        assert_eq!(again.str(), Some("hello, world"));
        assert_eq!(
            engine.node_evals, baseline,
            "a second evaluation of the same strings evaluates nothing"
        );

        let dirty = dirty_set(&program, content_hash(&Exp::str_("world")));
        engine.invalidate(&dirty);
        let edited = Exp::bin_op(
            Op::Concat,
            Exp::bin_op(Op::Concat, Exp::str_("hello"), Exp::str_(", ")),
            Exp::str_("there"),
        );
        let after = engine.eval_with_fuel(&edited, 10_000);
        assert_eq!(after.str(), Some("hello, there"));
        let delta = engine.node_evals - baseline;
        assert!(delta > 0 && delta < 5, "re-evaluated {delta} nodes");
    }

    #[test]
    fn a_fold_over_a_list_caches_and_re_evaluates_only_the_element_that_changed() {
        let x = Id::from_u128(1);
        let y = Id::from_u128(2);
        let plus = Exp::lam(
            x,
            Ty::Num,
            Exp::lam(y, Ty::Num, Exp::bin_op(Op::Add, Exp::var(x), Exp::var(y))),
        );
        let program = Exp::fold(
            Exp::list([Exp::num(1), Exp::num(2), Exp::num(3), Exp::num(4)]),
            Exp::num(0),
            plus.clone(),
        );
        let mut engine = IncrEngine::new();
        assert_eq!(engine.eval_with_fuel(&program, 100_000).num(), Some(10));
        let baseline = engine.node_evals;
        assert!(baseline > 0);

        assert_eq!(engine.eval_with_fuel(&program, 100_000).num(), Some(10));
        assert_eq!(
            engine.node_evals, baseline,
            "a second fold over the same list evaluates nothing"
        );

        let edited = Exp::fold(
            Exp::list([Exp::num(1), Exp::num(2), Exp::num(3), Exp::num(9)]),
            Exp::num(0),
            plus,
        );
        engine.invalidate(&dirty_set(&program, content_hash(&Exp::num(4))));
        assert_eq!(engine.eval_with_fuel(&edited, 100_000).num(), Some(15));
        let delta = engine.node_evals - baseline;
        assert!(
            delta > 0 && delta < baseline,
            "changing the last element re-evaluated {delta} of {baseline} nodes"
        );
    }

    #[test]
    fn a_list_long_enough_to_blow_a_recursive_fold_still_evaluates() {
        let x = Id::from_u128(1);
        let y = Id::from_u128(2);
        let program = Exp::fold(
            Exp::list((0..1_200).map(Exp::num)),
            Exp::num(0),
            Exp::lam(
                x,
                Ty::Num,
                Exp::lam(y, Ty::Num, Exp::bin_op(Op::Add, Exp::var(x), Exp::var(y))),
            ),
        );
        let mut engine = IncrEngine::new();
        assert_eq!(
            engine.eval_with_fuel(&program, 10_000_000).num(),
            Some((0..1_200).sum()),
            "the incremental fold walks the spine iteratively, not by recursion"
        );
    }

    #[test]
    fn renaming_a_variable_causes_zero_reevaluation() {
        let x = Id::from_u128(42);
        let mut names = NameTable::new();
        names.set(x, "x");
        let program = Exp::let_(
            x,
            Exp::num(5),
            Exp::bin_op(Op::Add, Exp::var(x), Exp::num(1)),
        );
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
                assert_eq!(
                    **b,
                    Dyn::Num(11),
                    "the second call sees x = 10, not a cached 6"
                );
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

    mod definitions {
        use super::*;
        use nothing_core::doc::Def;
        use nothing_core::ty::Ty;

        const FUEL: usize = 10_000;

        fn ids() -> (Id, Id, Id, Id) {
            (
                Id::from_u128(0x11),
                Id::from_u128(0x22),
                Id::from_u128(0x33),
                Id::from_u128(0x44),
            )
        }

        fn program(helper_offset: i64, unused_value: i64) -> Doc {
            let (main, helper, unused, x) = ids();
            Doc::new(vec![
                Def::new(main, Ty::Hole, Exp::ap(Exp::var(helper), Exp::num(1))),
                Def::new(
                    helper,
                    Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Num)),
                    Exp::lam(
                        x,
                        Ty::Num,
                        Exp::bin_op(Op::Add, Exp::var(x), Exp::num(helper_offset)),
                    ),
                ),
                Def::new(unused, Ty::Num, Exp::num(unused_value)),
            ])
            .expect("three distinct definitions")
        }

        #[test]
        fn a_cross_definition_call_resolves_by_id() {
            let (main, _, _, _) = ids();
            let doc = program(1, 7);
            assert!(doc.is_well_typed());
            let mut engine = IncrEngine::new();
            let outcome = engine.eval_definition(&doc, main, FUEL);
            assert_eq!(outcome.num(), Some(2), "{outcome:?}");
        }

        #[test]
        fn editing_a_helper_re_evaluates_its_dependent_and_an_unused_edit_re_evaluates_nothing() {
            let (main, _, _, _) = ids();
            let mut engine = IncrEngine::new();

            let first = engine.eval_definition(&program(1, 7), main, FUEL);
            assert_eq!(first.num(), Some(2));
            let after_cold = engine.node_evals;
            assert!(after_cold > 0, "the cold run evaluated nothing");

            let again = engine.eval_definition(&program(1, 7), main, FUEL);
            assert_eq!(again.num(), Some(2));
            assert_eq!(
                engine.node_evals,
                after_cold,
                "re-running an unchanged document re-evaluated {} nodes",
                engine.node_evals - after_cold
            );

            let edited_helper = engine.eval_definition(&program(2, 7), main, FUEL);
            assert_eq!(edited_helper.num(), Some(3));
            let after_helper_edit = engine.node_evals;
            assert!(
                after_helper_edit > after_cold,
                "editing helper re-evaluated nothing: main did not see the change"
            );

            let edited_unused = engine.eval_definition(&program(2, 8), main, FUEL);
            assert_eq!(edited_unused.num(), Some(3));
            assert_eq!(
                engine.node_evals,
                after_helper_edit,
                "editing an unused definition re-evaluated {} nodes",
                engine.node_evals - after_helper_edit
            );
        }

        #[test]
        fn a_definition_digest_covers_what_that_definition_reaches_and_nothing_else() {
            let (main, helper, unused, _) = ids();

            let base = definition_digests(&program(1, 7));
            let helper_edited = definition_digests(&program(2, 7));
            let unused_edited = definition_digests(&program(1, 8));

            assert_ne!(base[&main], helper_edited[&main]);
            assert_ne!(base[&helper], helper_edited[&helper]);
            assert_eq!(base[&unused], helper_edited[&unused]);

            assert_eq!(base[&main], unused_edited[&main]);
            assert_eq!(base[&helper], unused_edited[&helper]);
            assert_ne!(base[&unused], unused_edited[&unused]);
        }

        #[test]
        fn mutually_recursive_definitions_share_one_digest_and_still_terminate() {
            let (a, b, _, n) = ids();
            let ty = Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Bool));
            let body = |other: Id, at_zero: bool| {
                Exp::lam(
                    n,
                    Ty::Num,
                    Exp::if_(
                        Exp::bin_op(Op::Lt, Exp::var(n), Exp::num(1)),
                        Exp::bool_(at_zero),
                        Exp::ap(
                            Exp::var(other),
                            Exp::bin_op(Op::Sub, Exp::var(n), Exp::num(1)),
                        ),
                    ),
                )
            };
            let doc = Doc::new(vec![
                Def::new(a, ty.clone(), body(b, true)),
                Def::new(b, ty, body(a, false)),
            ])
            .expect("two definitions");
            assert!(doc.is_well_typed());

            let digests = definition_digests(&doc);
            assert_eq!(
                digests[&a], digests[&b],
                "two definitions in one cycle reach the same set, so they share a digest"
            );

            let caller = Id::from_u128(0xca11);
            let mut defs = doc.defs().to_vec();
            defs.push(Def::new(
                caller,
                Ty::Hole,
                Exp::ap(Exp::var(a), Exp::num(6)),
            ));
            let doc = Doc::new(defs).expect("the caller id is fresh");

            let mut engine = IncrEngine::new();
            let outcome = engine.eval_definition(&doc, caller, FUEL);
            assert_eq!(outcome.bool(), Some(true), "{outcome:?}");
        }

        #[test]
        fn a_self_referencing_definition_evaluates_without_the_combinator() {
            let (fact, _, _, n) = ids();
            let doc = Doc::new(vec![Def::new(
                fact,
                Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Num)),
                Exp::lam(
                    n,
                    Ty::Num,
                    Exp::if_(
                        Exp::bin_op(Op::Lt, Exp::var(n), Exp::num(1)),
                        Exp::num(1),
                        Exp::bin_op(
                            Op::Mul,
                            Exp::var(n),
                            Exp::ap(
                                Exp::var(fact),
                                Exp::bin_op(Op::Sub, Exp::var(n), Exp::num(1)),
                            ),
                        ),
                    ),
                ),
            )])
            .expect("one definition");

            let caller = Id::from_u128(0xca11);
            let mut defs = doc.defs().to_vec();
            defs.push(Def::new(
                caller,
                Ty::Num,
                Exp::ap(Exp::var(fact), Exp::num(5)),
            ));
            let doc = Doc::new(defs).expect("the caller id is fresh");

            let mut engine = IncrEngine::new();
            let outcome = engine.eval_definition(&doc, caller, FUEL);
            assert_eq!(outcome.num(), Some(120), "{outcome:?}");
        }
    }
}
