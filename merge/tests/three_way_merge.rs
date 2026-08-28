use nothing_core::exp::{Exp, HoleId, Id, Op};
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;
use nothing_core::typing::is_well_typed;
use nothing_merge::merge3::{ConflictKind, merge};
use nothing_merge::ops::Operation;
use nothing_merge::version::Version;

fn id(n: u128) -> Id {
    Id::from_u128(n)
}

fn hole(n: u128) -> HoleId {
    HoleId::from_u128(n)
}

fn record(a: Exp, b: Exp, c: Exp, d: Exp) -> Exp {
    Exp::pair(Exp::pair(a, b), Exp::pair(c, d))
}

fn names() -> NameTable {
    let mut names = NameTable::new();
    names.set(id(1), "square");
    names.set(id(2), "bump");
    names.set(id(10), "a");
    names.set(id(11), "b");
    names
}

#[test]
fn two_branches_editing_different_fields_of_one_record_merge_with_zero_conflicts() {
    let base = Version::new(
        record(
            Exp::num(1),
            Exp::bool_(true),
            Exp::num(3),
            Exp::bool_(false),
        ),
        names(),
    );
    let ours = Version::new(
        record(
            Exp::num(42),
            Exp::bool_(true),
            Exp::num(3),
            Exp::bool_(false),
        ),
        names(),
    );
    let theirs = Version::new(
        record(Exp::num(1), Exp::bool_(true), Exp::num(3), Exp::bool_(true)),
        names(),
    );

    let outcome = merge(&base, &ours, &theirs);
    assert!(
        outcome.is_clean(),
        "expected a clean merge, got:\n{}",
        outcome.report()
    );
    assert_eq!(outcome.conflicts.len(), 0);
    assert_eq!(
        outcome.merged.exp,
        record(
            Exp::num(42),
            Exp::bool_(true),
            Exp::num(3),
            Exp::bool_(true)
        )
    );
    assert!(outcome.merged.is_well_typed());
    assert!(outcome.repairs.is_empty());
    assert!(outcome.dropped.is_empty());
}

#[test]
fn two_branches_editing_the_two_halves_of_one_pair_merge_with_zero_conflicts() {
    let base = Version::new(Exp::pair(Exp::num(1), Exp::num(2)), names());
    let ours = Version::new(Exp::pair(Exp::num(9), Exp::num(2)), names());
    let theirs = Version::new(Exp::pair(Exp::num(1), Exp::num(8)), names());
    let outcome = merge(&base, &ours, &theirs);
    assert!(outcome.is_clean(), "{}", outcome.report());
    assert_eq!(outcome.merged.exp, Exp::pair(Exp::num(9), Exp::num(8)));
}

#[test]
fn two_branches_changing_the_same_expression_produce_exactly_one_conflict_with_both_alternatives() {
    let base = Version::new(Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2)), names());
    let ours = Version::new(Exp::bin_op(Op::Add, Exp::num(1), Exp::num(3)), names());
    let theirs = Version::new(Exp::bin_op(Op::Add, Exp::num(1), Exp::num(4)), names());

    let outcome = merge(&base, &ours, &theirs);
    assert_eq!(
        outcome.conflicts.len(),
        1,
        "expected exactly one conflict, got:\n{}",
        outcome.report()
    );

    let conflict = &outcome.conflicts[0];
    assert_eq!(conflict.kind, ConflictKind::SameNodeDifferentValues);
    assert_eq!(conflict.base_text, "2");
    assert_eq!(conflict.ours_text, "3");
    assert_eq!(conflict.theirs_text, "4");
    assert!(conflict.site.contains("right operand"), "{}", conflict.site);
    assert!(
        conflict.why.contains("replaces `2` with `3`"),
        "{}",
        conflict.why
    );
    assert!(
        conflict.why.contains("replaces `2` with `4`"),
        "{}",
        conflict.why
    );

    assert_eq!(
        outcome.merged.exp, base.exp,
        "a conflict leaves the base alone"
    );
    assert!(outcome.merged.is_well_typed());
}

#[test]
fn a_conflict_report_names_the_program_position_and_both_alternatives() {
    let base = Version::new(
        Exp::let_(
            id(1),
            Exp::lam(
                id(10),
                Ty::Num,
                Exp::bin_op(Op::Mul, Exp::var(id(10)), Exp::num(2)),
            ),
            Exp::ap(Exp::var(id(1)), Exp::num(5)),
        ),
        names(),
    );
    let ours = Version::new(
        Exp::let_(
            id(1),
            Exp::lam(
                id(10),
                Ty::Num,
                Exp::bin_op(Op::Mul, Exp::var(id(10)), Exp::num(3)),
            ),
            Exp::ap(Exp::var(id(1)), Exp::num(5)),
        ),
        names(),
    );
    let theirs = Version::new(
        Exp::let_(
            id(1),
            Exp::lam(
                id(10),
                Ty::Num,
                Exp::bin_op(Op::Mul, Exp::var(id(10)), Exp::num(7)),
            ),
            Exp::ap(Exp::var(id(1)), Exp::num(5)),
        ),
        names(),
    );
    let outcome = merge(&base, &ours, &theirs);
    assert_eq!(outcome.conflicts.len(), 1, "{}", outcome.report());
    let text = outcome.report();
    assert!(text.contains("base:   2"), "{text}");
    assert!(text.contains("ours:   3"), "{text}");
    assert!(text.contains("theirs: 7"), "{text}");
    assert!(text.contains("same node, different values"), "{text}");
}

#[test]
fn a_rename_never_conflicts_with_a_structural_edit() {
    let base = Version::new(
        Exp::let_(
            id(1),
            Exp::num(1),
            Exp::bin_op(Op::Add, Exp::var(id(1)), Exp::num(2)),
        ),
        names(),
    );
    let ours = Version::new(base.exp.clone(), base.names.with(id(1), "total"));
    let theirs = Version::new(
        Exp::let_(
            id(1),
            Exp::num(1),
            Exp::bin_op(Op::Add, Exp::var(id(1)), Exp::num(99)),
        ),
        names(),
    );
    let outcome = merge(&base, &ours, &theirs);
    assert!(outcome.is_clean(), "{}", outcome.report());
    assert_eq!(outcome.merged.names.get(id(1)), Some("total"));
    assert_eq!(outcome.merged.render(), "let total = 1 in total + 99");
}

#[test]
fn two_branches_renaming_one_binder_differently_conflict_once() {
    let base = Version::new(Exp::let_(id(1), Exp::num(1), Exp::var(id(1))), names());
    let ours = Version::new(base.exp.clone(), base.names.with(id(1), "total"));
    let theirs = Version::new(base.exp.clone(), base.names.with(id(1), "sum"));
    let outcome = merge(&base, &ours, &theirs);
    assert_eq!(outcome.conflicts.len(), 1, "{}", outcome.report());
    assert_eq!(outcome.conflicts[0].kind, ConflictKind::CompetingRenames);
    assert_eq!(outcome.conflicts[0].ours_text, "total");
    assert_eq!(outcome.conflicts[0].theirs_text, "sum");
}

#[test]
fn an_edit_inside_a_moved_subtree_is_rebased_onto_its_new_home() {
    let x = id(20);
    let function = |step: i64| {
        Exp::lam(
            x,
            Ty::Num,
            Exp::bin_op(Op::Add, Exp::var(x), Exp::num(step)),
        )
    };
    let base = Version::new(
        Exp::pair(function(10), Exp::empty_hole(hole(1))),
        NameTable::new(),
    );
    let ours = Version::new(
        Exp::pair(Exp::empty_hole(hole(2)), function(10)),
        NameTable::new(),
    );
    let theirs = Version::new(
        Exp::pair(function(20), Exp::empty_hole(hole(1))),
        NameTable::new(),
    );

    let outcome = merge(&base, &ours, &theirs);
    assert!(outcome.is_clean(), "{}", outcome.report());
    assert_eq!(outcome.commuted.len(), 1, "{:#?}", outcome.commuted);
    match &outcome.merged.exp {
        Exp::Pair(fst, snd) => {
            assert!(matches!(**fst, Exp::EmptyHole(_)));
            assert_eq!(**snd, function(20));
        }
        other => panic!("expected a pair, got {other:?}"),
    }
}

#[test]
fn a_reorder_and_an_edit_inside_a_reordered_binding_merge_cleanly() {
    let f = id(1);
    let g = id(2);
    let chain = |first: Id, first_body: Exp, second: Id, second_body: Exp| {
        Exp::let_(
            first,
            first_body,
            Exp::let_(
                second,
                second_body,
                Exp::bin_op(Op::Add, Exp::var(f), Exp::var(g)),
            ),
        )
    };
    let base = Version::new(chain(f, Exp::num(1), g, Exp::num(2)), names());
    let ours = Version::new(chain(g, Exp::num(2), f, Exp::num(1)), names());
    let theirs = Version::new(chain(f, Exp::num(41), g, Exp::num(2)), names());

    let outcome = merge(&base, &ours, &theirs);
    assert!(outcome.is_clean(), "{}", outcome.report());
    assert_eq!(
        outcome.merged.exp,
        chain(g, Exp::num(2), f, Exp::num(41)),
        "{}",
        outcome.merged.render()
    );
}

#[test]
fn two_branches_moving_the_same_subtree_conflict() {
    let x = id(20);
    let function = Exp::lam(x, Ty::Num, Exp::bin_op(Op::Add, Exp::var(x), Exp::num(1)));
    let base = Version::new(
        Exp::pair(
            function.clone(),
            Exp::pair(Exp::empty_hole(hole(1)), Exp::empty_hole(hole(2))),
        ),
        NameTable::new(),
    );
    let ours = Version::new(
        Exp::pair(
            Exp::empty_hole(hole(3)),
            Exp::pair(function.clone(), Exp::empty_hole(hole(2))),
        ),
        NameTable::new(),
    );
    let theirs = Version::new(
        Exp::pair(
            Exp::empty_hole(hole(4)),
            Exp::pair(Exp::empty_hole(hole(1)), function),
        ),
        NameTable::new(),
    );
    let outcome = merge(&base, &ours, &theirs);
    assert!(!outcome.is_clean());
    assert_eq!(outcome.conflicts[0].kind, ConflictKind::CompetingMoves);
}

#[test]
fn identical_edits_on_both_branches_apply_once_and_do_not_conflict() {
    let base = Version::new(Exp::bin_op(Op::Add, Exp::num(1), Exp::num(2)), names());
    let same = Version::new(Exp::bin_op(Op::Add, Exp::num(1), Exp::num(5)), names());
    let outcome = merge(&base, &same, &same);
    assert!(outcome.is_clean(), "{}", outcome.report());
    assert_eq!(outcome.applied.len(), 1, "{:#?}", outcome.applied);
    assert_eq!(outcome.merged.exp, same.exp);
}

#[test]
fn a_merge_that_would_be_ill_typed_is_quarantined_rather_than_produced() {
    let f = id(1);
    let base = Version::new(
        Exp::let_(
            f,
            Exp::lam(id(10), Ty::Num, Exp::var(id(10))),
            Exp::ap(Exp::var(f), Exp::empty_hole(hole(1))),
        ),
        names(),
    );
    let ours = Version::new(
        Exp::let_(
            f,
            Exp::lam(id(10), Ty::Bool, Exp::var(id(10))),
            Exp::ap(Exp::var(f), Exp::empty_hole(hole(1))),
        ),
        names(),
    );
    let theirs = Version::new(
        Exp::let_(
            f,
            Exp::lam(id(10), Ty::Num, Exp::var(id(10))),
            Exp::ap(Exp::var(f), Exp::num(5)),
        ),
        names(),
    );

    assert!(is_well_typed(&ours.exp));
    assert!(is_well_typed(&theirs.exp));

    let outcome = merge(&base, &ours, &theirs);
    assert!(
        outcome.is_clean(),
        "the two edits do not overlap: {}",
        outcome.report()
    );
    assert!(
        outcome.merged.is_well_typed(),
        "merged program is ill-typed: {}",
        outcome.merged.render()
    );
    assert_eq!(outcome.repairs.len(), 1, "{:#?}", outcome.repairs);
    assert!(
        outcome.merged.render().contains("⦇5⦈"),
        "the offending argument should be quarantined, got {}",
        outcome.merged.render()
    );
}

#[test]
fn a_conflicting_operation_is_never_applied() {
    let base = Version::new(Exp::pair(Exp::num(1), Exp::num(2)), names());
    let ours = Version::new(Exp::pair(Exp::num(9), Exp::num(7)), names());
    let theirs = Version::new(Exp::pair(Exp::num(1), Exp::num(8)), names());
    let outcome = merge(&base, &ours, &theirs);
    assert_eq!(outcome.conflicts.len(), 1, "{}", outcome.report());
    assert_eq!(
        outcome.merged.exp,
        Exp::pair(Exp::num(9), Exp::num(2)),
        "the non-conflicting half still merged"
    );
    assert!(
        !outcome.applied.iter().any(|op| matches!(
            op,
            Operation::Replace { path, .. } if path == &vec![1]
        )),
        "{:#?}",
        outcome.applied
    );
}
