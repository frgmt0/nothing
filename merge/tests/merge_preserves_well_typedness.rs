use nothing_action::act::{Action, EditState};
use nothing_action::generate::{Rng, well_typed_exp_with_depth};
use nothing_core::exp::{Exp, Id, Op, Side};
use nothing_core::names::NameTable;
use nothing_core::typing::is_well_typed;
use nothing_merge::apply::apply_all;
use nothing_merge::diff::{diff, structurally_equal};
use nothing_merge::merge3::merge;
use nothing_merge::version::Version;
use proptest::prelude::*;

fn binders(exp: &Exp, out: &mut Vec<Id>) {
    match exp {
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::EmptyHole(_) => {}
        Exp::Lam(id, _, body) => {
            out.push(*id);
            binders(body, out);
        }
        Exp::Let(id, bound, body) => {
            out.push(*id);
            binders(bound, out);
            binders(body, out);
        }
        Exp::Ap(f, a) => {
            binders(f, out);
            binders(a, out);
        }
        Exp::BinOp(_, l, r) | Exp::Pair(l, r) => {
            binders(l, out);
            binders(r, out);
        }
        Exp::If(c, t, e) => {
            binders(c, out);
            binders(t, out);
            binders(e, out);
        }
        Exp::Proj(_, e) | Exp::NonEmptyHole(_, e) => binders(e, out),
    }
}

const ANCESTOR_DEPTH: u32 = 4;

fn ancestor(seed: u64) -> Version {
    let exp = well_typed_exp_with_depth(seed, ANCESTOR_DEPTH);
    let mut ids = Vec::new();
    binders(&exp, &mut ids);
    let mut names = NameTable::new();
    for (n, id) in ids.into_iter().enumerate() {
        names.set(id, format!("v{n}"));
    }
    Version::new(exp, names)
}

fn pool(base: &Exp) -> Vec<Action> {
    let mut ids = Vec::new();
    binders(base, &mut ids);
    let mut actions = vec![
        Action::ConstructNum(1),
        Action::ConstructNum(-3),
        Action::ConstructNum(77),
        Action::ConstructBool(true),
        Action::ConstructBool(false),
        Action::ConstructBinOp(Op::Add),
        Action::ConstructBinOp(Op::Mul),
        Action::ConstructBinOp(Op::Lt),
        Action::ConstructIf,
        Action::ConstructLet,
        Action::ConstructPair,
        Action::ConstructProj(Side::L),
        Action::ConstructProj(Side::R),
        Action::ConstructAp,
        Action::ConstructLam,
        Action::Finish,
        Action::Delete,
        Action::MoveChild(0),
        Action::MoveChild(1),
        Action::MoveChild(2),
        Action::MoveNextSibling,
        Action::MovePrevSibling,
    ];
    for (n, id) in ids.into_iter().enumerate().take(4) {
        actions.push(Action::ConstructVar(id));
        actions.push(Action::Rename(id, format!("branch{n}")));
    }
    actions
}

fn descend(state: &mut EditState, rng: &mut Rng, depth: usize) {
    for _ in 0..depth {
        let n = rng.below(3);
        if !state.apply_mut(Action::MoveChild(n)) {
            let _ = state.apply_mut(Action::MoveChild(0));
        }
    }
}

fn branch(base: &Version, seed: u64, steps: usize) -> Version {
    let mut state = EditState::with_names(base.exp.clone(), base.names.clone());
    let mut rng = Rng::new(seed);
    let depth = 1 + rng.below(4);
    descend(&mut state, &mut rng, depth);
    let choices = pool(&base.exp);
    let mut taken = 0usize;
    let mut attempts = 0usize;
    while taken < steps && attempts < steps * 8 {
        attempts += 1;
        let action = choices[rng.below(choices.len())].clone();
        if state.apply_mut(action) {
            taken += 1;
        }
    }
    Version::from_state(&state)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 5_000,
        max_shrink_iters: 64,
        ..ProptestConfig::default()
    })]

    #[test]
    fn every_successful_merge_of_two_well_typed_branches_is_well_typed(
        seed in any::<u64>(),
        left in any::<u64>(),
        right in any::<u64>(),
        steps in 1usize..8,
    ) {
        let base = ancestor(seed);
        prop_assert!(is_well_typed(&base.exp));

        let ours = branch(&base, left, steps);
        let theirs = branch(&base, right, steps);
        prop_assert!(is_well_typed(&ours.exp), "ours: {}", ours.render());
        prop_assert!(is_well_typed(&theirs.exp), "theirs: {}", theirs.render());

        let outcome = merge(&base, &ours, &theirs);
        prop_assert!(
            is_well_typed(&outcome.merged.exp),
            "merge produced an ill-typed program\n base:   {}\n ours:   {}\n theirs: {}\n merged: {}",
            base.render(),
            ours.render(),
            theirs.render(),
            outcome.merged.render()
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 2_000,
        max_shrink_iters: 64,
        ..ProptestConfig::default()
    })]

    #[test]
    fn replaying_a_diff_onto_its_own_ancestor_reproduces_the_branch(
        seed in any::<u64>(),
        edit in any::<u64>(),
        steps in 1usize..8,
    ) {
        let base = ancestor(seed);
        let other = branch(&base, edit, steps);
        let ops = diff(&base, &other);
        let replayed = apply_all(&base, &ops);

        prop_assert!(replayed.dropped.is_empty(), "dropped {:#?}", replayed.dropped);
        prop_assert!(
            structurally_equal(&replayed.version.exp, &other.exp),
            "diff did not reproduce the branch\n base:     {}\n other:    {}\n replayed: {}\n ops: {:#?}",
            base.render(),
            other.render(),
            replayed.version.render(),
            ops
        );
        for (id, name) in other.names.entries() {
            prop_assert_eq!(replayed.version.names.display(id), name);
        }
    }
}

#[test]
fn enough_random_branch_pairs_merge_cleanly_for_the_property_to_mean_something() {
    let mut clean = 0usize;
    let mut repaired = 0usize;
    let mut conflicting = 0usize;
    for seed in 0..1_000u64 {
        let base = ancestor(seed);
        let ours = branch(&base, seed.wrapping_mul(31), 4);
        let theirs = branch(&base, seed.wrapping_mul(97).wrapping_add(7), 4);
        let outcome = merge(&base, &ours, &theirs);
        assert!(
            is_well_typed(&outcome.merged.exp),
            "seed {seed} merged to an ill-typed program: {}",
            outcome.merged.render()
        );
        if !outcome.is_clean() {
            conflicting += 1;
        } else if outcome.repairs.is_empty() {
            clean += 1;
        } else {
            repaired += 1;
        }
    }
    assert_eq!(clean + repaired + conflicting, 1_000);
    println!(
        "1000 merges: {clean} clean, {repaired} repaired by quarantine, {conflicting} conflicting"
    );
    assert!(
        clean + repaired >= 200,
        "only {} of 1000 random branch pairs merged without conflict; the well-typedness \
         property would be close to vacuous",
        clean + repaired
    );
}
