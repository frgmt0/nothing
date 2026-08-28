use std::collections::HashSet;

use nothing_core::exp::{Exp, Id};
use nothing_store::nodes::{Digest, content_hash};

use crate::chain;
use crate::ops::Operation;
use crate::path::{Path, arity, child, extend};
use crate::version::Version;

pub const MIN_MOVE_SIZE: usize = 3;

pub fn size(exp: &Exp) -> usize {
    let mut total = 1;
    for n in 0..arity(exp) {
        if let Some(c) = child(exp, n) {
            total += size(c);
        }
    }
    total
}

pub fn structurally_equal(a: &Exp, b: &Exp) -> bool {
    match (a, b) {
        (Exp::EmptyHole(_), Exp::EmptyHole(_)) => true,
        (Exp::NonEmptyHole(_, x), Exp::NonEmptyHole(_, y)) => structurally_equal(x, y),
        (Exp::Var(x), Exp::Var(y)) => x == y,
        (Exp::Num(x), Exp::Num(y)) => x == y,
        (Exp::Bool(x), Exp::Bool(y)) => x == y,
        (Exp::Lam(x, tx, bx), Exp::Lam(y, ty, by)) => {
            x == y && tx == ty && structurally_equal(bx, by)
        }
        (Exp::Ap(f1, a1), Exp::Ap(f2, a2)) => {
            structurally_equal(f1, f2) && structurally_equal(a1, a2)
        }
        (Exp::BinOp(o1, l1, r1), Exp::BinOp(o2, l2, r2)) => {
            o1 == o2 && structurally_equal(l1, l2) && structurally_equal(r1, r2)
        }
        (Exp::If(c1, t1, e1), Exp::If(c2, t2, e2)) => {
            structurally_equal(c1, c2) && structurally_equal(t1, t2) && structurally_equal(e1, e2)
        }
        (Exp::Let(x, bound1, body1), Exp::Let(y, bound2, body2)) => {
            x == y && structurally_equal(bound1, bound2) && structurally_equal(body1, body2)
        }
        (Exp::Pair(l1, r1), Exp::Pair(l2, r2)) => {
            structurally_equal(l1, l2) && structurally_equal(r1, r2)
        }
        (Exp::Proj(s1, e1), Exp::Proj(s2, e2)) => s1 == s2 && structurally_equal(e1, e2),
        _ => false,
    }
}

pub fn diff(base: &Version, other: &Version) -> Vec<Operation> {
    let mut ops = diff_names(base, other);
    let mut structure = Vec::new();
    diff_exp(&base.exp, &other.exp, &[], &mut structure);
    ops.extend(detect_moves(structure));
    ops
}

pub fn diff_names(base: &Version, other: &Version) -> Vec<Operation> {
    let mut seen: HashSet<Id> = HashSet::new();
    let mut ids: Vec<Id> = Vec::new();
    for id in base.names.ids().into_iter().chain(other.names.ids()) {
        if seen.insert(id) {
            ids.push(id);
        }
    }
    ids.sort();
    ids.into_iter()
        .filter_map(|id| {
            let from = base.names.get(id).map(str::to_string);
            let to = other.names.get(id)?.to_string();
            if from.as_deref() == Some(to.as_str()) {
                None
            } else {
                Some(Operation::Rename { id, from, to })
            }
        })
        .collect()
}

fn diff_exp(a: &Exp, b: &Exp, path: &[usize], out: &mut Vec<Operation>) {
    if structurally_equal(a, b) {
        return;
    }
    if content_hash(a) == content_hash(b) {
        out.push(Operation::Rebind {
            path: path.to_vec(),
            node: b.clone(),
        });
        return;
    }
    match (a, b) {
        (Exp::EmptyHole(h), _) => out.push(Operation::Fill {
            path: path.to_vec(),
            hole: *h,
            node: b.clone(),
        }),
        (_, Exp::EmptyHole(h)) => out.push(Operation::DeleteToHole {
            path: path.to_vec(),
            node: a.clone(),
            hole: *h,
        }),
        (Exp::Lam(id_a, ty_a, body_a), Exp::Lam(id_b, ty_b, body_b)) => {
            if id_a != id_b {
                out.push(replace(path, a, b));
                return;
            }
            if ty_a != ty_b {
                out.push(Operation::SetAnn {
                    path: path.to_vec(),
                    from: ty_a.clone(),
                    to: ty_b.clone(),
                });
            }
            diff_exp(body_a, body_b, &extend(path, 0), out);
        }
        (Exp::Let(..), Exp::Let(..)) => diff_chain(a, b, path, out),
        (Exp::Ap(..), Exp::Ap(..)) | (Exp::Pair(..), Exp::Pair(..)) => {
            diff_children(a, b, path, 2, out)
        }
        (Exp::If(..), Exp::If(..)) => diff_children(a, b, path, 3, out),
        (Exp::NonEmptyHole(..), Exp::NonEmptyHole(..)) => diff_children(a, b, path, 1, out),
        (Exp::BinOp(op_a, ..), Exp::BinOp(op_b, ..)) => {
            if op_a == op_b {
                diff_children(a, b, path, 2, out);
            } else {
                out.push(replace(path, a, b));
            }
        }
        (Exp::Proj(side_a, ..), Exp::Proj(side_b, ..)) => {
            if side_a == side_b {
                diff_children(a, b, path, 1, out);
            } else {
                out.push(replace(path, a, b));
            }
        }
        _ => diff_shapes(a, b, path, out),
    }
}

fn diff_children(a: &Exp, b: &Exp, path: &[usize], count: usize, out: &mut Vec<Operation>) {
    for n in 0..count {
        if let (Some(ca), Some(cb)) = (child(a, n), child(b, n)) {
            diff_exp(ca, cb, &extend(path, n), out);
        }
    }
}

fn diff_shapes(a: &Exp, b: &Exp, path: &[usize], out: &mut Vec<Operation>) {
    for n in 0..arity(b) {
        if let Some(c) = child(b, n)
            && structurally_equal(c, a)
        {
            out.push(Operation::Insert {
                path: path.to_vec(),
                slot: n,
                node: b.clone(),
            });
            return;
        }
    }
    for n in 0..arity(a) {
        if let Some(c) = child(a, n)
            && structurally_equal(c, b)
        {
            out.push(Operation::Delete {
                path: path.to_vec(),
                slot: n,
                node: a.clone(),
            });
            return;
        }
    }
    out.push(replace(path, a, b));
}

fn replace(path: &[usize], a: &Exp, b: &Exp) -> Operation {
    Operation::Replace {
        path: path.to_vec(),
        from: a.clone(),
        to: b.clone(),
    }
}

fn diff_chain(a: &Exp, b: &Exp, path: &[usize], out: &mut Vec<Operation>) {
    let chain_a = chain::chain_of(a);
    let chain_b = chain::chain_of(b);
    let ids_a: Vec<Id> = chain_a.bindings.iter().map(|x| x.id).collect();
    let ids_b: Vec<Id> = chain_b.bindings.iter().map(|x| x.id).collect();

    let reordered = ids_a.len() == ids_b.len() && ids_a != ids_b && {
        let mut sorted_a = ids_a.clone();
        let mut sorted_b = ids_b.clone();
        sorted_a.sort();
        sorted_b.sort();
        sorted_a == sorted_b
    };

    if !reordered {
        if let (Exp::Let(id_a, ..), Exp::Let(id_b, ..)) = (a, b)
            && id_a != id_b
        {
            out.push(replace(path, a, b));
            return;
        }
        diff_children(a, b, path, 2, out);
        return;
    }

    emit_binding_moves(&ids_a, &ids_b, path, out);

    for (index, binding) in chain_a.bindings.iter().enumerate() {
        if let Some(other) = chain_b.bindings.iter().find(|x| x.id == binding.id) {
            diff_exp(
                &binding.bound,
                &other.bound,
                &chain::bound_path(path, index),
                out,
            );
        }
    }
    diff_exp(
        &chain_a.tail,
        &chain_b.tail,
        &chain::tail_path(path, chain_a.bindings.len()),
        out,
    );
}

fn emit_binding_moves(ids_a: &[Id], ids_b: &[Id], path: &[usize], out: &mut Vec<Operation>) {
    let keep: HashSet<Id> = chain::longest_common_subsequence(ids_a, ids_b)
        .into_iter()
        .collect();
    for (from_index, id) in ids_a.iter().enumerate() {
        if keep.contains(id) {
            continue;
        }
        let to_index = ids_b
            .iter()
            .position(|other| other == id)
            .unwrap_or(from_index);
        out.push(Operation::MoveBinding {
            chain_root: path.to_vec(),
            chain_len: ids_a.len(),
            binder: *id,
            from_index,
            to_index,
        });
    }
}

struct Endpoint {
    index: usize,
    path: Path,
    subtree: Exp,
    hash: Digest,
    replacement: Exp,
}

fn removal(index: usize, op: &Operation) -> Option<Endpoint> {
    match op {
        Operation::DeleteToHole { path, node, hole } => Some(Endpoint {
            index,
            path: path.clone(),
            subtree: node.clone(),
            hash: content_hash(node),
            replacement: Exp::empty_hole(*hole),
        }),
        Operation::Replace { path, from, to } => Some(Endpoint {
            index,
            path: path.clone(),
            subtree: from.clone(),
            hash: content_hash(from),
            replacement: to.clone(),
        }),
        _ => None,
    }
}

fn insertion(index: usize, op: &Operation) -> Option<Endpoint> {
    match op {
        Operation::Fill { path, node, .. } => Some(Endpoint {
            index,
            path: path.clone(),
            subtree: node.clone(),
            hash: content_hash(node),
            replacement: node.clone(),
        }),
        Operation::Replace { path, to, .. } => Some(Endpoint {
            index,
            path: path.clone(),
            subtree: to.clone(),
            hash: content_hash(to),
            replacement: to.clone(),
        }),
        _ => None,
    }
}

pub fn detect_moves(ops: Vec<Operation>) -> Vec<Operation> {
    let removals: Vec<Endpoint> = ops
        .iter()
        .enumerate()
        .filter_map(|(i, op)| removal(i, op))
        .filter(|e| size(&e.subtree) >= MIN_MOVE_SIZE)
        .collect();
    let insertions: Vec<Endpoint> = ops
        .iter()
        .enumerate()
        .filter_map(|(i, op)| insertion(i, op))
        .filter(|e| size(&e.subtree) >= MIN_MOVE_SIZE)
        .collect();

    let mut consumed: HashSet<usize> = HashSet::new();
    let mut moves: Vec<(usize, Operation)> = Vec::new();

    for source in &removals {
        if consumed.contains(&source.index) {
            continue;
        }
        let target = insertions.iter().find(|target| {
            target.index != source.index
                && !consumed.contains(&target.index)
                && target.hash == source.hash
                && target.path != source.path
        });
        if let Some(target) = target {
            consumed.insert(source.index);
            consumed.insert(target.index);
            moves.push((
                source.index,
                Operation::Move {
                    from: source.path.clone(),
                    to: target.path.clone(),
                    node: target.subtree.clone(),
                    vacated: source.replacement.clone(),
                },
            ));
        }
    }

    let mut out = Vec::new();
    for (i, op) in ops.into_iter().enumerate() {
        if let Some(pos) = moves.iter().position(|(index, _)| *index == i) {
            out.push(moves[pos].1.clone());
            continue;
        }
        if consumed.contains(&i) {
            continue;
        }
        out.push(op);
    }
    out
}
