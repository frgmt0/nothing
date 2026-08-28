use nothing_core::exp::{Exp, Id};

use crate::path::Path;

#[derive(Clone, PartialEq, Debug)]
pub struct Binding {
    pub id: Id,
    pub bound: Exp,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Chain {
    pub bindings: Vec<Binding>,
    pub tail: Exp,
}

pub fn chain_of(exp: &Exp) -> Chain {
    let mut bindings = Vec::new();
    let mut cursor = exp;
    while let Exp::Let(id, bound, body) = cursor {
        bindings.push(Binding {
            id: *id,
            bound: (**bound).clone(),
        });
        cursor = body;
    }
    Chain {
        bindings,
        tail: cursor.clone(),
    }
}

pub fn rebuild(chain: &Chain) -> Exp {
    let mut out = chain.tail.clone();
    for binding in chain.bindings.iter().rev() {
        out = Exp::let_(binding.id, binding.bound.clone(), out);
    }
    out
}

pub fn spine_path(chain_root: &[usize], index: usize) -> Path {
    let mut out = chain_root.to_vec();
    for _ in 0..index {
        out.push(1);
    }
    out
}

pub fn bound_path(chain_root: &[usize], index: usize) -> Path {
    let mut out = spine_path(chain_root, index);
    out.push(0);
    out
}

pub fn tail_path(chain_root: &[usize], len: usize) -> Path {
    let mut out = spine_path(chain_root, len.saturating_sub(1));
    out.push(1);
    out
}

pub fn touches_ordering(chain_root: &[usize], len: usize, other: &[usize]) -> bool {
    if crate::path::is_prefix(other, chain_root) {
        return true;
    }
    if !crate::path::is_prefix(chain_root, other) {
        return false;
    }
    let rest = &other[chain_root.len()..];
    let mut spine = 0usize;
    for step in rest {
        if *step == 0 {
            return false;
        }
        spine += 1;
        if spine >= len {
            return false;
        }
    }
    true
}

pub fn longest_common_subsequence(a: &[Id], b: &[Id]) -> Vec<Id> {
    let n = a.len();
    let m = b.len();
    let mut table = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i][j] = if a[i] == b[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            out.push(a[i]);
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> Id {
        Id::from_u128(n)
    }

    #[test]
    fn a_let_chain_flattens_and_rebuilds() {
        let e = Exp::let_(
            id(1),
            Exp::num(1),
            Exp::let_(id(2), Exp::num(2), Exp::var(id(1))),
        );
        let chain = chain_of(&e);
        assert_eq!(chain.bindings.len(), 2);
        assert_eq!(chain.tail, Exp::var(id(1)));
        assert_eq!(rebuild(&chain), e);
    }

    #[test]
    fn chain_paths_land_on_the_right_nodes() {
        let e = Exp::let_(
            id(1),
            Exp::num(11),
            Exp::let_(id(2), Exp::num(22), Exp::num(33)),
        );
        assert_eq!(crate::path::at(&e, &bound_path(&[], 0)), Some(&Exp::num(11)));
        assert_eq!(crate::path::at(&e, &bound_path(&[], 1)), Some(&Exp::num(22)));
        assert_eq!(crate::path::at(&e, &tail_path(&[], 2)), Some(&Exp::num(33)));
    }

    #[test]
    fn ordering_is_touched_by_spine_edits_but_not_by_binding_bodies() {
        assert!(touches_ordering(&[], 2, &[]));
        assert!(touches_ordering(&[], 2, &[1]));
        assert!(!touches_ordering(&[], 2, &[0]));
        assert!(!touches_ordering(&[], 2, &[0, 1]));
        assert!(!touches_ordering(&[], 2, &[1, 0]));
        assert!(!touches_ordering(&[], 2, &[1, 1]));
    }

    #[test]
    fn lcs_of_a_swap_keeps_one_element() {
        let got = longest_common_subsequence(&[id(1), id(2)], &[id(2), id(1)]);
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn lcs_of_an_identical_sequence_keeps_everything() {
        let seq = [id(1), id(2), id(3)];
        assert_eq!(longest_common_subsequence(&seq, &seq), seq.to_vec());
    }
}
