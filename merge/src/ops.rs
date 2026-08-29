use nothing_core::exp::{Exp, HoleId, Id};
use nothing_core::names::NameTable;
use nothing_core::render::render;
use nothing_core::ty::Ty;

use crate::chain;
use crate::path::{Path, is_prefix, label, nested};

#[derive(Clone, PartialEq, Debug)]
pub enum Operation {
    Rename {
        id: Id,
        from: Option<String>,
        to: String,
    },
    Fill {
        path: Path,
        hole: HoleId,
        node: Exp,
    },
    DeleteToHole {
        path: Path,
        node: Exp,
        hole: HoleId,
    },
    Insert {
        path: Path,
        slot: usize,
        node: Exp,
    },
    Delete {
        path: Path,
        slot: usize,
        node: Exp,
    },
    Move {
        from: Path,
        to: Path,
        node: Exp,
        vacated: Exp,
    },
    MoveBinding {
        chain_root: Path,
        chain_len: usize,
        binder: Id,
        from_index: usize,
        to_index: usize,
    },
    Replace {
        path: Path,
        from: Exp,
        to: Exp,
    },
    SetAnn {
        path: Path,
        from: Ty,
        to: Ty,
    },
    Rebind {
        path: Path,
        node: Exp,
    },
    ReorderFields {
        path: Path,
        from: Vec<Id>,
        to: Vec<Id>,
    },
}

#[derive(Clone, PartialEq, Debug)]
pub enum Region {
    Node(Path),
    Shape(Path),
    Name(Id),
    Order(Path, usize),
    Cell(Path),
    FieldOrder(Path),
}

pub fn regions_overlap(a: &Region, b: &Region) -> bool {
    match (a, b) {
        (Region::Name(x), Region::Name(y)) => x == y,
        (Region::Name(_), _) | (_, Region::Name(_)) => false,
        (Region::Cell(p), Region::Cell(q)) => p == q,
        (Region::Cell(p), Region::Order(root, len))
        | (Region::Order(root, len), Region::Cell(p)) => chain::touches_ordering(root, *len, p),
        (Region::Cell(p), Region::Node(q) | Region::Shape(q))
        | (Region::Node(q) | Region::Shape(q), Region::Cell(p)) => is_prefix(q, p),
        (Region::Node(p), Region::Node(q)) => nested(p, q),
        (Region::Node(p), Region::Shape(q)) => is_prefix(p, q),
        (Region::Shape(q), Region::Node(p)) => is_prefix(p, q),
        (Region::Shape(p), Region::Shape(q)) => p == q,
        (Region::Order(root, len), Region::Node(p) | Region::Shape(p)) => {
            chain::touches_ordering(root, *len, p)
        }
        (Region::Node(p) | Region::Shape(p), Region::Order(root, len)) => {
            chain::touches_ordering(root, *len, p)
        }
        (Region::Order(p, _), Region::Order(q, _)) => p == q,
        (Region::FieldOrder(p), Region::FieldOrder(q)) => p == q,
        (Region::FieldOrder(p), Region::Node(q) | Region::Shape(q) | Region::Cell(q))
        | (Region::Node(q) | Region::Shape(q) | Region::Cell(q), Region::FieldOrder(p)) => {
            is_prefix(q, p)
        }
        (Region::FieldOrder(p), Region::Order(root, len))
        | (Region::Order(root, len), Region::FieldOrder(p)) => {
            chain::touches_ordering(root, *len, p)
        }
    }
}

impl Operation {
    pub fn footprint(&self) -> Vec<Region> {
        match self {
            Operation::Rename { id, .. } => vec![Region::Name(*id)],
            Operation::Fill { path, .. }
            | Operation::DeleteToHole { path, .. }
            | Operation::Replace { path, .. }
            | Operation::Rebind { path, .. } => vec![Region::Node(path.clone())],
            Operation::Insert { path, slot, node } | Operation::Delete { path, slot, node } => {
                if matches!(node, Exp::Cons(..)) && *slot == 1 {
                    vec![Region::Cell(path.clone())]
                } else {
                    vec![Region::Node(path.clone())]
                }
            }
            Operation::SetAnn { path, .. } => vec![Region::Shape(path.clone())],
            Operation::Move { from, to, .. } => {
                vec![Region::Node(from.clone()), Region::Node(to.clone())]
            }
            Operation::MoveBinding {
                chain_root,
                chain_len,
                ..
            } => vec![Region::Order(chain_root.clone(), *chain_len)],
            Operation::ReorderFields { path, .. } => vec![Region::FieldOrder(path.clone())],
        }
    }

    pub fn site(&self) -> Option<&Path> {
        match self {
            Operation::Rename { .. } => None,
            Operation::Fill { path, .. }
            | Operation::DeleteToHole { path, .. }
            | Operation::Insert { path, .. }
            | Operation::Delete { path, .. }
            | Operation::Replace { path, .. }
            | Operation::Rebind { path, .. }
            | Operation::SetAnn { path, .. } => Some(path),
            Operation::Move { from, .. } => Some(from),
            Operation::MoveBinding { chain_root, .. } => Some(chain_root),
            Operation::ReorderFields { path, .. } => Some(path),
        }
    }

    pub fn rebasable(&self) -> bool {
        matches!(
            self,
            Operation::Fill { .. }
                | Operation::DeleteToHole { .. }
                | Operation::Insert { .. }
                | Operation::Delete { .. }
                | Operation::Replace { .. }
                | Operation::Rebind { .. }
                | Operation::SetAnn { .. }
        )
    }

    pub fn rebased(&self, from: &[usize], to: &[usize]) -> Option<Operation> {
        if !self.rebasable() {
            return None;
        }
        let site = self.site()?;
        if site.len() <= from.len() || !is_prefix(from, site) {
            return None;
        }
        let mut moved = to.to_vec();
        moved.extend_from_slice(&site[from.len()..]);
        Some(self.at_site(moved))
    }

    fn at_site(&self, path: Path) -> Operation {
        match self.clone() {
            Operation::Fill { hole, node, .. } => Operation::Fill { path, hole, node },
            Operation::DeleteToHole { node, hole, .. } => {
                Operation::DeleteToHole { path, node, hole }
            }
            Operation::Insert { slot, node, .. } => Operation::Insert { path, slot, node },
            Operation::Delete { slot, node, .. } => Operation::Delete { path, slot, node },
            Operation::Replace { from, to, .. } => Operation::Replace { path, from, to },
            Operation::Rebind { node, .. } => Operation::Rebind { path, node },
            Operation::SetAnn { from, to, .. } => Operation::SetAnn { path, from, to },
            other => other,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Operation::Rename { .. } => "Rename",
            Operation::Fill { .. } => "Fill",
            Operation::DeleteToHole { .. } => "DeleteToHole",
            Operation::Insert { .. } => "Insert",
            Operation::Delete { .. } => "Delete",
            Operation::Move { .. } => "Move",
            Operation::MoveBinding { .. } => "MoveBinding",
            Operation::Replace { .. } => "Replace",
            Operation::SetAnn { .. } => "SetAnn",
            Operation::Rebind { .. } => "Rebind",
            Operation::ReorderFields { .. } => "ReorderFields",
        }
    }

    pub fn outcome(&self, names: &NameTable) -> String {
        match self {
            Operation::Rename { to, .. } => to.clone(),
            Operation::Fill { node, .. } => render(node, names),
            Operation::DeleteToHole { .. } => "⦇⦈".to_string(),
            Operation::Insert { node, .. } => render(node, names),
            Operation::Delete { node, slot, .. } => match crate::path::child(node, *slot) {
                Some(kept) => render(kept, names),
                None => render(node, names),
            },
            Operation::Move {
                vacated, node, to, ..
            } => format!(
                "`{}` here, with `{}` moved to {}",
                render(vacated, names),
                render(node, names),
                crate::path::describe(to)
            ),
            Operation::MoveBinding {
                binder,
                from_index,
                to_index,
                ..
            } => format!(
                "binding `{}` moved from position {} to position {}",
                names.display(*binder),
                from_index,
                to_index
            ),
            Operation::Replace { to, .. } => render(to, names),
            Operation::SetAnn { to, .. } => format!("annotation {to}"),
            Operation::Rebind { node, .. } => render(node, names),
            Operation::ReorderFields { to, .. } => {
                format!("fields in the order {}", field_list(to, names))
            }
        }
    }

    pub fn describe(&self, base: &Exp, names: &NameTable) -> String {
        match self {
            Operation::Rename { id, from, to } => match from {
                Some(old) => format!("renames `{old}` to `{to}`"),
                None => format!("names {id:?} `{to}`"),
            },
            Operation::Fill { path, node, .. } => format!(
                "fills the hole at {} with `{}`",
                label(base, path),
                render(node, names)
            ),
            Operation::DeleteToHole { path, node, .. } => format!(
                "deletes `{}` at {}, leaving a hole",
                render(node, names),
                label(base, path)
            ),
            Operation::Insert { path, node, .. } => {
                format!("wraps {} in `{}`", label(base, path), render(node, names))
            }
            Operation::Delete { path, node, slot } => format!(
                "unwraps {} from `{}`, keeping child {slot}",
                label(base, path),
                render(node, names)
            ),
            Operation::Move { from, to, node, .. } => format!(
                "moves `{}` from {} to {}",
                render(node, names),
                label(base, from),
                crate::path::describe(to)
            ),
            Operation::MoveBinding {
                binder,
                from_index,
                to_index,
                ..
            } => format!(
                "moves the binding of `{}` from position {} to position {} in its let chain",
                names.display(*binder),
                from_index,
                to_index
            ),
            Operation::Replace { path, from, to } => format!(
                "replaces `{}` with `{}` at {}",
                render(from, names),
                render(to, names),
                label(base, path)
            ),
            Operation::SetAnn { path, from, to } => format!(
                "changes the parameter annotation at {} from {from} to {to}",
                label(base, path)
            ),
            Operation::Rebind { path, .. } => format!(
                "changes binder identity at {} without changing structure",
                label(base, path)
            ),
            Operation::ReorderFields { path, from, to } => format!(
                "reorders the fields of the record at {} from {} to {}",
                label(base, path),
                field_list(from, names),
                field_list(to, names)
            ),
        }
    }
}

fn field_list(ids: &[Id], names: &NameTable) -> String {
    let shown: Vec<String> = ids.iter().map(|id| names.display(*id)).collect();
    format!("`{}`", shown.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(p: &[usize]) -> Region {
        Region::Node(p.to_vec())
    }

    #[test]
    fn sibling_nodes_do_not_overlap() {
        assert!(!regions_overlap(&node(&[0]), &node(&[1])));
        assert!(!regions_overlap(&node(&[0, 0]), &node(&[0, 1])));
    }

    #[test]
    fn ancestor_and_descendant_overlap() {
        assert!(regions_overlap(&node(&[0]), &node(&[0, 1, 2])));
        assert!(regions_overlap(&node(&[]), &node(&[1])));
    }

    #[test]
    fn a_shape_change_is_invisible_to_edits_below_it() {
        assert!(!regions_overlap(
            &Region::Shape(vec![0]),
            &Region::Node(vec![0, 0])
        ));
        assert!(regions_overlap(
            &Region::Shape(vec![0]),
            &Region::Node(vec![0])
        ));
        assert!(regions_overlap(
            &Region::Shape(vec![0]),
            &Region::Node(vec![])
        ));
    }

    #[test]
    fn names_never_overlap_structure() {
        let n = Region::Name(Id::from_u128(1));
        assert!(!regions_overlap(&n, &node(&[])));
        assert!(regions_overlap(&n, &n));
        assert!(!regions_overlap(&n, &Region::Name(Id::from_u128(2))));
    }

    #[test]
    fn reordering_a_chain_does_not_overlap_a_binding_body_edit() {
        let order = Region::Order(vec![], 2);
        assert!(!regions_overlap(&order, &node(&[0])));
        assert!(!regions_overlap(&order, &node(&[1, 0])));
        assert!(regions_overlap(&order, &node(&[1])));
        assert!(regions_overlap(&order, &node(&[])));
    }
}
