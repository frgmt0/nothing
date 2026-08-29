use nothing_core::exp::Exp;
use nothing_core::names::NameTable;

use crate::chain;
use crate::ops::Operation;
use crate::path::{at, child, replace_at, with_child};
use crate::version::Version;

#[derive(Clone, PartialEq, Debug)]
pub struct Applied {
    pub version: Version,
    pub dropped: Vec<Operation>,
}

pub fn apply_all(base: &Version, ops: &[Operation]) -> Applied {
    let mut names = base.names.clone();
    for op in ops {
        if let Operation::Rename { id, to, .. } = op {
            names.set(*id, to.clone());
        }
    }

    let mut structural: Vec<&Operation> = ops
        .iter()
        .filter(|op| {
            !matches!(
                op,
                Operation::Rename { .. }
                    | Operation::MoveBinding { .. }
                    | Operation::ReorderFields { .. }
            )
        })
        .collect();
    structural.sort_by_key(|op| std::cmp::Reverse(op.site().map_or(0, Vec::len)));

    let mut orders: Vec<&Operation> = ops
        .iter()
        .filter(|op| {
            matches!(
                op,
                Operation::MoveBinding { .. } | Operation::ReorderFields { .. }
            )
        })
        .collect();
    orders.sort_by_key(|op| std::cmp::Reverse(op.site().map_or(0, Vec::len)));

    let mut exp = base.exp.clone();
    let mut dropped = Vec::new();
    for op in structural.into_iter().chain(orders) {
        match apply_one(&exp, op) {
            Some(next) => exp = next,
            None => dropped.push(op.clone()),
        }
    }

    Applied {
        version: Version::new(exp, names),
        dropped,
    }
}

pub fn apply_one(exp: &Exp, op: &Operation) -> Option<Exp> {
    match op {
        Operation::Rename { .. } => Some(exp.clone()),
        Operation::Fill { path, node, .. } => replace_at(exp, path, node.clone()),
        Operation::DeleteToHole { path, hole, .. } => replace_at(exp, path, Exp::empty_hole(*hole)),
        Operation::Insert { path, slot, node } => {
            let current = at(exp, path)?.clone();
            let wrapped = with_child(node, *slot, current)?;
            replace_at(exp, path, wrapped)
        }
        Operation::Delete { path, slot, .. } => {
            let current = at(exp, path)?;
            let kept = child(current, *slot)?.clone();
            replace_at(exp, path, kept)
        }
        Operation::Move {
            from,
            to,
            node,
            vacated,
        } => {
            let stepped = replace_at(exp, from, vacated.clone())?;
            replace_at(&stepped, to, node.clone())
        }
        Operation::MoveBinding {
            chain_root,
            binder,
            to_index,
            ..
        } => {
            let current = at(exp, chain_root)?;
            let mut flat = chain::chain_of(current);
            let from = flat.bindings.iter().position(|b| b.id == *binder)?;
            let binding = flat.bindings.remove(from);
            let target = (*to_index).min(flat.bindings.len());
            flat.bindings.insert(target, binding);
            replace_at(exp, chain_root, chain::rebuild(&flat))
        }
        Operation::Replace { path, to, .. } => replace_at(exp, path, to.clone()),
        Operation::SetAnn { path, to, .. } => {
            let current = at(exp, path)?;
            match current {
                Exp::Lam(id, _, body) => {
                    let rebuilt = Exp::Lam(*id, to.clone(), body.clone());
                    replace_at(exp, path, rebuilt)
                }
                _ => None,
            }
        }
        Operation::Rebind { path, node } => replace_at(exp, path, node.clone()),
        Operation::ReorderFields { path, to, .. } => {
            let current = at(exp, path)?;
            let Exp::Record(fields) = current else {
                return None;
            };
            if fields.len() != to.len() {
                return None;
            }
            let mut reordered = Vec::with_capacity(to.len());
            for id in to {
                let (_, value) = fields.iter().find(|(other, _)| other == id)?;
                reordered.push((*id, value.clone()));
            }
            replace_at(exp, path, Exp::Record(reordered))
        }
    }
}

pub fn apply_names(names: &NameTable, ops: &[Operation]) -> NameTable {
    let mut out = names.clone();
    for op in ops {
        if let Operation::Rename { id, to, .. } = op {
            out.set(*id, to.clone());
        }
    }
    out
}
