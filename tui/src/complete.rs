//! The candidate list behind the name run (`KEYS.md` §"Literal entry").
//!
//! A letter starts a *run*: the characters typed since the cursor last
//! moved. After each keystroke the run is matched against the names that
//! could legally appear at the cursor — the in-scope binders plus `true` and
//! `false` — and the top-ranked match is **committed immediately** as a real
//! `ConstructVar` / `ConstructBool` against the real program. There is no
//! confirm key and no moment where the render disagrees with the AST.
//!
//! # What this module decides, and what it does not
//!
//! It decides *which names are candidates* (prefix filter over `ctx_at` plus
//! the two boolean literals) and *in what order* they are offered. The
//! ordering is where the payoff of bidirectional typing lands, and it is
//! `KEYS.md` §"Literal entry" rank order, in full:
//!
//! 1. an **exact** name match first — you typed the whole thing, so no
//!    inference outranks you;
//! 2. then **type consistency with `expected_ty_at(cursor)`** (`type_rank`):
//!    a type equal to the expected type above one merely consistent with it
//!    via `?` above one that is inconsistent. This is the payoff of
//!    bidirectional typing — at a hole expecting `Num → Num` a binder of
//!    that type outranks an unrelated `Bool`, and at a `Bool` hole `false`
//!    outranks `f : Num → Num`;
//! 3. then innermost scope, then shortest name, then alphabetical, so the
//!    choice is total and deterministic and never depends on the order the
//!    bindings happened to be walked in.
//!
//! Ranking is not filtering. The prefix decides *membership*; the type only
//! decides *order*, so a name that does not fit is still offered (marked
//! `✗` on the status line) and still commits — the calculus quarantines it
//! rather than refusing it. That is what keeps "the user is never told no"
//! true in the completion path too.
//!
//! Names are pre-Phase-5 display names (`core::render::render_id`, so `x0`,
//! `x1`, …). When the name table lands, [`display_name`] becomes a table
//! read and nothing else here changes.

use nothing_action::act::Action;
use nothing_core::exp::Id;
use nothing_core::render::render_id;
use nothing_core::ty::{Ty, is_consistent};

use crate::app::AppState;

/// What a candidate writes into the program when it is committed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CandidateKind {
    /// An in-scope binder.
    Var(Id),
    /// `true` / `false` — candidates, not keys, so `t` is one keystroke at a
    /// `Bool` hole without spending a letter on a verb.
    Bool(bool),
}

/// One offer in the completion list.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Candidate {
    /// The display name, as the projection would render it.
    pub name: String,
    /// The type this name has here. Shown beside it on the status line,
    /// because ranking by type is only legible if the types are visible.
    pub ty: Ty,
    pub kind: CandidateKind,
}

impl Candidate {
    /// The single primitive action that commits this candidate.
    pub fn action(&self) -> Action {
        match self.kind {
            CandidateKind::Var(id) => Action::ConstructVar(id),
            CandidateKind::Bool(b) => Action::ConstructBool(b),
        }
    }

    /// Is this candidate's type the one the cursor is asking for? Surfaced
    /// on the status line so the ranking reads as a consequence of the
    /// program rather than as magic.
    pub fn fits(&self, expected: &Ty) -> bool {
        is_consistent(&self.ty, expected)
    }
}

/// The display name of a binder. Phase 5 turns this into a name-table read;
/// until then it is `core::render`'s placeholder spelling, which is what the
/// projection shows, so what the user types is what they see.
pub fn display_name(id: Id) -> String {
    render_id(id)
}

/// Every candidate matching `prefix` at the cursor, best first.
///
/// The list is drawn from `ctx_at`, which is why `ConstructVar` — the one
/// fallible construction — can never fail from the keyboard.
pub fn candidates(state: &AppState, prefix: &str) -> Vec<Candidate> {
    let ctx = state.ctx();
    let expected = state.expected_ty();

    // Innermost scope last in `binders_in_scope`; a shadowed id must appear
    // once, at its innermost depth.
    let binders = state.binders_in_scope();
    let mut depth: Vec<(Id, usize)> = Vec::new();
    for (i, id) in binders.iter().enumerate() {
        match depth.iter_mut().find(|(seen, _)| seen == id) {
            Some(slot) => slot.1 = i,
            None => depth.push((*id, i)),
        }
    }

    let mut out: Vec<(RankKey, Candidate)> = Vec::new();
    for (id, scope) in depth {
        let name = display_name(id);
        if !name.starts_with(prefix) {
            continue;
        }
        let ty = ctx.lookup(&id).unwrap_or(Ty::Hole);
        let candidate = Candidate {
            name,
            ty,
            kind: CandidateKind::Var(id),
        };
        out.push((rank_key(&candidate, &expected, prefix, scope), candidate));
    }
    for b in [true, false] {
        let name = b.to_string();
        if !name.starts_with(prefix) {
            continue;
        }
        let candidate = Candidate {
            name,
            ty: Ty::Bool,
            kind: CandidateKind::Bool(b),
        };
        // Literals are not in any scope; rank them outside every binder so a
        // binder of the same length wins the tie-break by being nearer.
        out.push((rank_key(&candidate, &expected, prefix, 0), candidate));
    }

    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.into_iter().map(|(_, c)| c).collect()
}

/// The top-ranked candidate — the one commit-live writes into the program.
pub fn best(state: &AppState, prefix: &str) -> Option<Candidate> {
    candidates(state, prefix).into_iter().next()
}

/// The sort key. Smaller is better; every field is total, so the order never
/// depends on the input order of the bindings.
type RankKey = (u8, u8, usize, usize, String);

/// Rank one candidate, by the four fields the module docs list, in order.
///
/// `expected` is the type the cursor asks for (`expected_ty_at`); every
/// other field is a deterministic tie-break, so two candidates can never
/// compare equal unless they render identically.
fn rank_key(candidate: &Candidate, expected: &Ty, prefix: &str, scope: usize) -> RankKey {
    (
        u8::from(candidate.name != prefix), // an exact match first
        type_rank(&candidate.ty, expected),
        usize::MAX - scope, // innermost scope first
        candidate.name.chars().count(),
        candidate.name.clone(),
    )
}

/// How well a candidate's type answers the expected type at the cursor.
/// Smaller is better: equal, then consistent-via-`?`, then inconsistent.
///
/// **An unknown expected type ranks nothing.** When the cursor asks for `?`
/// the position constrains nothing, so every candidate scores alike and the
/// scope tie-break decides. The alternative — reading `?` literally and
/// letting a `?`-typed binder score "equal" — would promote the binder the
/// editor knows *least* about at exactly the position where it knows
/// nothing, which is ranking by accident. Recorded in `KEYS.md`
/// §"Settled by the implementation".
fn type_rank(ty: &Ty, expected: &Ty) -> u8 {
    if *expected == Ty::Hole {
        1
    } else if ty == expected {
        0
    } else if is_consistent(ty, expected) {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{handle_key, key};
    use crossterm::event::KeyCode;

    /// Drive the pure key handler over `keys`, so no test here can drift
    /// from the grammar the editor actually implements.
    fn typed(keys: &str) -> AppState {
        keys.chars().fold(AppState::empty(), |state, c| {
            handle_key(key(KeyCode::Char(c)), state)
        })
    }

    fn names(state: &AppState, prefix: &str) -> Vec<String> {
        candidates(state, prefix)
            .into_iter()
            .map(|c| c.name)
            .collect()
    }

    /// `λx0:Num. λx1:Bool. ⦇⦈` with the cursor in the innermost body, built
    /// with the editor's own keys so the test cannot drift from the grammar.
    fn two_binders() -> AppState {
        typed("\\x0:n.\\x1:b.")
    }

    /// `λx0:Num→Num. λx1:Bool. λx2:(Num→Num)→Num. ⦇⦈`, the cursor in the
    /// innermost body — a context holding a function of the interesting type
    /// and an unrelated `Bool`. The body hole is unconstrained (`?`).
    fn function_bool_and_caller() -> AppState {
        typed("\\x0:n>n.\\x1:b.\\x2:(n>n)>n.")
    }

    /// The same three binders, with the cursor moved to the *argument* of
    /// `x2 ⦇⦈` — the one position in this program that asks for `Num → Num`.
    /// `x` `2` names the caller, `space` applies it (`ConstructAp` wraps the
    /// focus and lands on the new argument hole).
    fn hole_expecting_num_to_num() -> AppState {
        let state = function_bool_and_caller();
        let state = ['x', '2', ' ']
            .into_iter()
            .fold(state, |s, c| handle_key(key(KeyCode::Char(c)), s));
        assert_eq!(
            state.expected_ty(),
            Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Num)),
            "the fixture must actually stand at a Num -> Num hole"
        );
        state
    }

    /// The Phase 4 checkbox, stated as its own acceptance criterion: *at a
    /// hole expecting `Num → Num`, a function of that type ranks above an
    /// unrelated `Bool`.*
    #[test]
    fn a_function_of_the_expected_type_outranks_an_unrelated_bool() {
        let state = hole_expecting_num_to_num();

        let ranked = names(&state, "x");
        let function = ranked.iter().position(|n| n == "x0").expect("x0 offered");
        let boolean = ranked.iter().position(|n| n == "x1").expect("x1 offered");
        assert!(
            function < boolean,
            "x0 : Num -> Num must outrank x1 : Bool at a Num -> Num hole, got {ranked:?}"
        );

        // The whole order, pinned: the exact type first, then the two
        // inconsistent names in innermost-scope order.
        assert_eq!(ranked, vec!["x0", "x2", "x1"]);
        assert_eq!(best(&state, "x").map(|c| c.name), Some("x0".to_string()));

        // And it is the *type* doing the work, not the scope: at the
        // unconstrained hole of the same program, with the same context and
        // the same prefix, the innermost binder wins instead.
        assert_eq!(
            names(&function_bool_and_caller(), "x"),
            vec!["x2", "x1", "x0"]
        );
    }

    /// `KEYS.md` rank 2 in full: equal, then consistent-via-`?`, then
    /// inconsistent. `λx0:Num. λx1:?. λx2:Bool. ⦇⦈ + ⦇⦈` with the cursor on
    /// the left operand, which asks for `Num`. Scope order here is exactly
    /// the reverse of type order, so nothing else could produce this answer.
    #[test]
    fn consistent_via_a_hole_ranks_between_exact_and_inconsistent() {
        let state = typed("\\x0:n.\\x1:?.\\x2:b.+");
        assert_eq!(state.expected_ty(), Ty::Num);
        assert_eq!(names(&state, "x"), vec!["x0", "x1", "x2"]);
    }

    /// The other example `KEYS.md` gives: at a `Bool` hole, `false`
    /// outranks a function.
    #[test]
    fn a_bool_hole_prefers_a_boolean_literal_to_a_function() {
        // λx0:Num→Num. if ⦇⦈ then ⦇⦈ else ⦇⦈ — the scrutinee asks for Bool.
        let state = typed("\\x0:n>n.?");
        assert_eq!(state.expected_ty(), Ty::Bool);
        let ranked = names(&state, "");
        assert_eq!(
            &ranked[..2],
            ["true", "false"],
            "the literals fit and the function does not: {ranked:?}"
        );
        assert_eq!(ranked.last(), Some(&"x0".to_string()));
    }

    /// An unknown expected type is not a type to rank by, and it is not a
    /// filter either: every in-scope name is still offered.
    #[test]
    fn an_unknown_expected_type_ranks_nothing_and_filters_nothing() {
        let state = function_bool_and_caller();
        assert_eq!(state.expected_ty(), Ty::Hole);

        // Nothing is dropped: three binders plus the two literals.
        assert_eq!(names(&state, ""), vec!["x2", "x1", "x0", "true", "false"]);
        // ... and the order is the scope tie-break, untouched by the types.
        assert_eq!(names(&state, "x"), vec!["x2", "x1", "x0"]);
        assert!(best(&state, "x").is_some());

        // The degenerate case the ranking must also survive: no binders at
        // all, at the root hole, where the expected type is `?`.
        let empty = AppState::empty();
        assert_eq!(empty.expected_ty(), Ty::Hole);
        assert!(candidates(&empty, "x").is_empty());
        assert_eq!(names(&empty, ""), vec!["true", "false"]);
    }

    #[test]
    fn the_prefix_filters_before_the_type_ranks() {
        // A prefix that excludes the best-typed name must not resurrect it.
        let state = hole_expecting_num_to_num();
        assert_eq!(names(&state, "x1"), vec!["x1"]);
        assert_eq!(best(&state, "x1").map(|c| c.name), Some("x1".to_string()));
        assert!(candidates(&state, "z").is_empty());
    }

    #[test]
    fn the_prefix_filters_the_in_scope_names() {
        let state = two_binders();
        assert_eq!(
            names(&state, "x"),
            vec!["x1", "x0"],
            "innermost binder first"
        );
        assert_eq!(names(&state, "x0"), vec!["x0"]);
        assert!(candidates(&state, "z").is_empty());
    }

    #[test]
    fn the_boolean_literals_are_candidates_not_keys() {
        let state = two_binders();
        let names: Vec<String> = candidates(&state, "t")
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert_eq!(names, vec!["true"]);
        assert_eq!(
            best(&state, "f").map(|c| c.kind),
            Some(CandidateKind::Bool(false))
        );
    }

    #[test]
    fn an_exact_match_outranks_a_longer_one() {
        // `x1` is a prefix of nothing else here, but the rule has to hold
        // for the case where it is: an exact hit is always offered first.
        let state = two_binders();
        assert_eq!(best(&state, "x1").map(|c| c.name), Some("x1".to_string()));
        assert_eq!(
            best(&state, "true").map(|c| c.kind),
            Some(CandidateKind::Bool(true))
        );
    }

    #[test]
    fn candidates_carry_the_type_the_status_line_shows() {
        let state = two_binders();
        let x0 = candidates(&state, "x0").remove(0);
        assert_eq!(x0.ty, Ty::Num);
        let x1 = candidates(&state, "x1").remove(0);
        assert_eq!(x1.ty, Ty::Bool);
        assert!(x1.fits(&Ty::Bool));
        assert!(!x1.fits(&Ty::Num));
    }

    #[test]
    fn every_candidate_is_constructible_from_the_keyboard() {
        // `ConstructVar` is the one fallible construction; the list is drawn
        // from `ctx_at`, so it must never offer a name that fails.
        let state = two_binders();
        for candidate in candidates(&state, "") {
            assert!(
                state.apply_actions(&[candidate.action()]).is_some(),
                "{} was offered but does not apply",
                candidate.name
            );
        }
    }
}
