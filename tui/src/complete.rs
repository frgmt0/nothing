use nothing_action::act::Action;
use nothing_core::exp::Id;
use nothing_core::ty::{Ty, is_consistent};

use crate::app::AppState;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CandidateKind {
    Var(Id),
    Bool(bool),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Candidate {
    pub name: String,
    pub ty: Ty,
    pub kind: CandidateKind,
}

impl Candidate {
    pub fn action(&self) -> Action {
        match self.kind {
            CandidateKind::Var(id) => Action::ConstructVar(id),
            CandidateKind::Bool(b) => Action::ConstructBool(b),
        }
    }

    pub fn fits(&self, expected: &Ty) -> bool {
        is_consistent(&self.ty, expected)
    }
}

pub fn candidates(state: &AppState, prefix: &str) -> Vec<Candidate> {
    let ctx = state.ctx();
    let expected = state.expected_ty();

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
        let name = state.display_name(id);
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

        out.push((rank_key(&candidate, &expected, prefix, 0), candidate));
    }

    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.into_iter().map(|(_, c)| c).collect()
}

pub fn best(state: &AppState, prefix: &str) -> Option<Candidate> {
    candidates(state, prefix).into_iter().next()
}

type RankKey = (u8, u8, usize, usize, String);

fn rank_key(candidate: &Candidate, expected: &Ty, prefix: &str, scope: usize) -> RankKey {
    (
        u8::from(candidate.name != prefix),
        type_rank(&candidate.ty, expected),
        usize::MAX - scope,
        candidate.name.chars().count(),
        candidate.name.clone(),
    )
}

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

    fn two_binders() -> AppState {
        typed("\\x0:n.\\x1:b.")
    }

    fn function_bool_and_caller() -> AppState {
        typed("\\x0:n>n.\\x1:b.\\x2:(n>n)>n.")
    }

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

        assert_eq!(ranked, vec!["x0", "x2", "x1"]);
        assert_eq!(best(&state, "x").map(|c| c.name), Some("x0".to_string()));

        assert_eq!(
            names(&function_bool_and_caller(), "x"),
            vec!["x2", "x1", "x0"]
        );
    }

    #[test]
    fn consistent_via_a_hole_ranks_between_exact_and_inconsistent() {
        let state = typed("\\x0:n.\\x1:?.\\x2:b.+");
        assert_eq!(state.expected_ty(), Ty::Num);
        assert_eq!(names(&state, "x"), vec!["x0", "x1", "x2"]);
    }

    #[test]
    fn a_bool_hole_prefers_a_boolean_literal_to_a_function() {
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

    #[test]
    fn an_unknown_expected_type_ranks_nothing_and_filters_nothing() {
        let state = function_bool_and_caller();
        assert_eq!(state.expected_ty(), Ty::Hole);

        assert_eq!(
            names(&state, ""),
            vec!["x2", "x1", "x0", "main", "true", "false"],
            "the definition the cursor is in is a name like any other"
        );

        assert_eq!(names(&state, "x"), vec!["x2", "x1", "x0"]);
        assert!(best(&state, "x").is_some());

        let empty = AppState::empty();
        assert_eq!(empty.expected_ty(), Ty::Hole);
        assert!(candidates(&empty, "x").is_empty());
        assert_eq!(names(&empty, ""), vec!["main", "true", "false"]);
    }

    #[test]
    fn the_prefix_filters_before_the_type_ranks() {
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
