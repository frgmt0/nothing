use nothing_core::exp::{Exp, Id};

use crate::codec::{
    decode_op, decode_side, decode_ty, encode_op, encode_side, encode_ty, read_hole_id, read_i64,
    read_id, read_u8, write_hole_id, write_i64, write_id,
};
use crate::error::DecodeError;

pub type Digest = [u8; 32];

#[derive(Clone, PartialEq, Debug)]
pub struct NodeEntry {
    pub hash: Digest,
    pub tag: u8,
    pub payload: Vec<u8>,
    pub children: Vec<u32>,
}

fn hash_node(tag: u8, canonical_payload: &[u8], children: &[Digest]) -> Digest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&[tag]);
    hasher.update(canonical_payload);
    for child in children {
        hasher.update(child);
    }
    *hasher.finalize().as_bytes()
}

fn debruijn_index(id: Id, stack: &[Id]) -> Option<u64> {
    stack
        .iter()
        .rev()
        .position(|bound| *bound == id)
        .map(|p| p as u64)
}

fn push_entry(
    table: &mut Vec<NodeEntry>,
    hash: Digest,
    tag: u8,
    payload: Vec<u8>,
    children: Vec<u32>,
) -> u32 {
    let idx = table.len() as u32;
    table.push(NodeEntry {
        hash,
        tag,
        payload,
        children,
    });
    idx
}

fn build_rec(exp: &Exp, stack: &mut Vec<Id>, table: &mut Vec<NodeEntry>) -> (u32, Digest) {
    match exp {
        Exp::Var(id) => {
            let mut canon = Vec::new();
            match debruijn_index(*id, stack) {
                Some(idx) => {
                    canon.push(0);
                    crate::codec::write_varint(&mut canon, idx);
                }
                None => {
                    canon.push(1);
                    canon.extend_from_slice(id.uuid().as_bytes());
                }
            }
            let hash = hash_node(0, &canon, &[]);
            let mut payload = Vec::new();
            write_id(&mut payload, *id);
            let idx = push_entry(table, hash, 0, payload, vec![]);
            (idx, hash)
        }
        Exp::Lam(id, ty, body) => {
            stack.push(*id);
            let (body_idx, body_hash) = build_rec(body, stack, table);
            stack.pop();
            let mut ty_bytes = Vec::new();
            encode_ty(&mut ty_bytes, ty);
            let hash = hash_node(1, &ty_bytes, &[body_hash]);
            let mut payload = Vec::new();
            write_id(&mut payload, *id);
            payload.extend_from_slice(&ty_bytes);
            let idx = push_entry(table, hash, 1, payload, vec![body_idx]);
            (idx, hash)
        }
        Exp::Ap(f, a) => {
            let (f_idx, f_hash) = build_rec(f, stack, table);
            let (a_idx, a_hash) = build_rec(a, stack, table);
            let hash = hash_node(2, &[], &[f_hash, a_hash]);
            let idx = push_entry(table, hash, 2, vec![], vec![f_idx, a_idx]);
            (idx, hash)
        }
        Exp::Num(n) => {
            let mut payload = Vec::new();
            write_i64(&mut payload, *n);
            let hash = hash_node(3, &payload, &[]);
            let idx = push_entry(table, hash, 3, payload, vec![]);
            (idx, hash)
        }
        Exp::Bool(b) => {
            let payload = vec![if *b { 1 } else { 0 }];
            let hash = hash_node(4, &payload, &[]);
            let idx = push_entry(table, hash, 4, payload, vec![]);
            (idx, hash)
        }
        Exp::BinOp(op, l, r) => {
            let (l_idx, l_hash) = build_rec(l, stack, table);
            let (r_idx, r_hash) = build_rec(r, stack, table);
            let payload = vec![encode_op(*op)];
            let hash = hash_node(5, &payload, &[l_hash, r_hash]);
            let idx = push_entry(table, hash, 5, payload, vec![l_idx, r_idx]);
            (idx, hash)
        }
        Exp::If(c, t, e) => {
            let (c_idx, c_hash) = build_rec(c, stack, table);
            let (t_idx, t_hash) = build_rec(t, stack, table);
            let (e_idx, e_hash) = build_rec(e, stack, table);
            let hash = hash_node(6, &[], &[c_hash, t_hash, e_hash]);
            let idx = push_entry(table, hash, 6, vec![], vec![c_idx, t_idx, e_idx]);
            (idx, hash)
        }
        Exp::Let(id, bound, body) => {
            let (bound_idx, bound_hash) = build_rec(bound, stack, table);
            stack.push(*id);
            let (body_idx, body_hash) = build_rec(body, stack, table);
            stack.pop();
            let hash = hash_node(7, &[], &[bound_hash, body_hash]);
            let mut payload = Vec::new();
            write_id(&mut payload, *id);
            let idx = push_entry(table, hash, 7, payload, vec![bound_idx, body_idx]);
            (idx, hash)
        }
        Exp::Pair(l, r) => {
            let (l_idx, l_hash) = build_rec(l, stack, table);
            let (r_idx, r_hash) = build_rec(r, stack, table);
            let hash = hash_node(8, &[], &[l_hash, r_hash]);
            let idx = push_entry(table, hash, 8, vec![], vec![l_idx, r_idx]);
            (idx, hash)
        }
        Exp::Proj(side, e) => {
            let (e_idx, e_hash) = build_rec(e, stack, table);
            let payload = vec![encode_side(*side)];
            let hash = hash_node(9, &payload, &[e_hash]);
            let idx = push_entry(table, hash, 9, payload, vec![e_idx]);
            (idx, hash)
        }
        Exp::EmptyHole(h) => {
            let hash = hash_node(10, &[], &[]);
            let mut payload = Vec::new();
            write_hole_id(&mut payload, *h);
            let idx = push_entry(table, hash, 10, payload, vec![]);
            (idx, hash)
        }
        Exp::NonEmptyHole(h, inner) => {
            let (inner_idx, inner_hash) = build_rec(inner, stack, table);
            let hash = hash_node(11, &[], &[inner_hash]);
            let mut payload = Vec::new();
            write_hole_id(&mut payload, *h);
            let idx = push_entry(table, hash, 11, payload, vec![inner_idx]);
            (idx, hash)
        }
    }
}

pub fn build_node_table(exp: &Exp) -> Vec<NodeEntry> {
    let mut table = Vec::new();
    let mut stack = Vec::new();
    build_rec(exp, &mut stack, &mut table);
    table
}

pub fn content_hash(exp: &Exp) -> Digest {
    let table = build_node_table(exp);
    table
        .last()
        .expect("every Exp produces at least one node")
        .hash
}

fn decode_child(
    entries: &[NodeEntry],
    entry: &NodeEntry,
    which: usize,
) -> Result<Exp, DecodeError> {
    let child_idx = *entry.children.get(which).ok_or(DecodeError::MissingChild)?;
    decode_at(entries, child_idx as usize)
}

fn decode_at(entries: &[NodeEntry], idx: usize) -> Result<Exp, DecodeError> {
    let entry = entries
        .get(idx)
        .ok_or(DecodeError::BadNodeRef(idx as u32))?;
    let mut pos = 0usize;
    match entry.tag {
        0 => {
            let id = read_id(&entry.payload, &mut pos)?;
            Ok(Exp::var(id))
        }
        1 => {
            let id = read_id(&entry.payload, &mut pos)?;
            let ty = decode_ty(&entry.payload, &mut pos)?;
            let body = decode_child(entries, entry, 0)?;
            Ok(Exp::lam(id, ty, body))
        }
        2 => {
            let f = decode_child(entries, entry, 0)?;
            let a = decode_child(entries, entry, 1)?;
            Ok(Exp::ap(f, a))
        }
        3 => {
            let n = read_i64(&entry.payload, &mut pos)?;
            Ok(Exp::num(n))
        }
        4 => {
            let b = read_u8(&entry.payload, &mut pos)?;
            match b {
                0 => Ok(Exp::bool_(false)),
                1 => Ok(Exp::bool_(true)),
                other => Err(DecodeError::BadBool(other)),
            }
        }
        5 => {
            let op = decode_op(read_u8(&entry.payload, &mut pos)?)?;
            let l = decode_child(entries, entry, 0)?;
            let r = decode_child(entries, entry, 1)?;
            Ok(Exp::bin_op(op, l, r))
        }
        6 => {
            let c = decode_child(entries, entry, 0)?;
            let t = decode_child(entries, entry, 1)?;
            let e = decode_child(entries, entry, 2)?;
            Ok(Exp::if_(c, t, e))
        }
        7 => {
            let id = read_id(&entry.payload, &mut pos)?;
            let bound = decode_child(entries, entry, 0)?;
            let body = decode_child(entries, entry, 1)?;
            Ok(Exp::let_(id, bound, body))
        }
        8 => {
            let l = decode_child(entries, entry, 0)?;
            let r = decode_child(entries, entry, 1)?;
            Ok(Exp::pair(l, r))
        }
        9 => {
            let side = decode_side(read_u8(&entry.payload, &mut pos)?)?;
            let e = decode_child(entries, entry, 0)?;
            Ok(Exp::proj(side, e))
        }
        10 => {
            let h = read_hole_id(&entry.payload, &mut pos)?;
            Ok(Exp::empty_hole(h))
        }
        11 => {
            let h = read_hole_id(&entry.payload, &mut pos)?;
            let inner = decode_child(entries, entry, 0)?;
            Ok(Exp::non_empty_hole(h, inner))
        }
        other => Err(DecodeError::BadTag(other)),
    }
}

pub fn decode_node_table(entries: &[NodeEntry]) -> Result<Exp, DecodeError> {
    if entries.is_empty() {
        return Err(DecodeError::EmptyNodeTable);
    }
    decode_at(entries, entries.len() - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::exp::HoleId;
    use nothing_core::ty::Ty;

    #[test]
    fn alpha_equivalent_functions_hash_the_same() {
        let a = {
            let x = Id::from_u128(1);
            Exp::lam(
                x,
                Ty::Num,
                Exp::bin_op(nothing_core::exp::Op::Add, Exp::var(x), Exp::num(1)),
            )
        };
        let b = {
            let y = Id::from_u128(999);
            Exp::lam(
                y,
                Ty::Num,
                Exp::bin_op(nothing_core::exp::Op::Add, Exp::var(y), Exp::num(1)),
            )
        };
        assert_ne!(a, b);
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn structurally_different_functions_hash_differently() {
        let x = Id::from_u128(1);
        let a = Exp::lam(
            x,
            Ty::Num,
            Exp::bin_op(nothing_core::exp::Op::Add, Exp::var(x), Exp::num(1)),
        );
        let b = Exp::lam(
            x,
            Ty::Num,
            Exp::bin_op(nothing_core::exp::Op::Add, Exp::var(x), Exp::num(2)),
        );
        assert_ne!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn holes_with_different_ids_hash_the_same() {
        let a = Exp::empty_hole(HoleId::from_u128(1));
        let b = Exp::empty_hole(HoleId::from_u128(2));
        assert_ne!(a, b);
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn node_table_round_trips() {
        let x = Id::from_u128(7);
        let e = Exp::let_(x, Exp::num(3), Exp::var(x));
        let table = build_node_table(&e);
        let decoded = decode_node_table(&table).unwrap();
        assert_eq!(decoded, e);
    }
}
