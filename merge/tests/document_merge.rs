use nothing_core::doc::{Def, Doc, MAIN_NAME, references};
use nothing_core::docs::DocTable;
use nothing_core::exp::{Exp, HoleId, Id, Op};
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;
use nothing_merge::document::{DefChange, DocConflictKind, DocVersion, merge_documents};

fn main_id() -> Id {
    Id::from_u128(0x0a)
}

fn helper_id() -> Id {
    Id::from_u128(0x0b)
}

fn other_id() -> Id {
    Id::from_u128(0x0c)
}

fn param() -> Id {
    Id::from_u128(0x0d)
}

fn hole(n: u128) -> HoleId {
    HoleId::from_u128(n)
}

fn num_to_num() -> Ty {
    Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Num))
}

fn names() -> NameTable {
    let mut names = NameTable::new();
    names.set(main_id(), MAIN_NAME);
    names.set(helper_id(), "helper");
    names.set(other_id(), "other");
    names.set(param(), "x");
    names
}

fn helper(offset: i64) -> Def {
    Def::new(
        helper_id(),
        num_to_num(),
        Exp::lam(
            param(),
            Ty::Num,
            Exp::bin_op(Op::Add, Exp::var(param()), Exp::num(offset)),
        ),
    )
}

fn other(n: i64) -> Def {
    Def::new(other_id(), Ty::Num, Exp::num(n))
}

fn main_def(arg: i64) -> Def {
    Def::new(
        main_id(),
        Ty::Num,
        Exp::ap(Exp::var(helper_id()), Exp::num(arg)),
    )
}

fn version(defs: Vec<Def>) -> DocVersion {
    DocVersion::new(Doc::new(defs).expect("distinct ids"), names())
}

fn base() -> DocVersion {
    version(vec![main_def(1), helper(1), other(0)])
}

#[test]
fn the_base_document_is_well_typed() {
    assert!(base().is_well_typed());
}

#[test]
fn two_branches_editing_different_definitions_merge_with_zero_conflicts() {
    let ours = version(vec![main_def(2), helper(1), other(0)]);
    let theirs = version(vec![main_def(1), helper(5), other(0)]);

    let outcome = merge_documents(&base(), &ours, &theirs);
    assert!(
        outcome.is_clean(),
        "expected a clean merge, got:\n{}",
        outcome.report()
    );
    assert!(outcome.merged.is_well_typed());

    let merged_main = outcome.merged.definition(main_id()).expect("main survives");
    assert_eq!(
        merged_main.body,
        main_def(2).body,
        "ours' edit to main was lost"
    );
    let merged_helper = outcome
        .merged
        .definition(helper_id())
        .expect("helper survives");
    assert_eq!(
        merged_helper.body,
        helper(5).body,
        "theirs' edit to helper was lost"
    );
    assert_eq!(
        outcome
            .merged
            .definition(other_id())
            .map(|d| d.body.clone()),
        Some(other(0).body)
    );
}

#[test]
fn three_branches_worth_of_independent_definition_edits_all_land() {
    let ours = version(vec![main_def(7), helper(1), other(0)]);
    let theirs = version(vec![main_def(1), helper(9), other(42)]);

    let outcome = merge_documents(&base(), &ours, &theirs);
    assert!(outcome.is_clean(), "{}", outcome.report());
    assert_eq!(
        outcome
            .merged
            .definition(other_id())
            .map(|d| d.body.clone()),
        Some(Exp::num(42))
    );
}

#[test]
fn a_rename_on_one_side_is_detected_and_carried() {
    let mut renamed_names = names();
    renamed_names.set(helper_id(), "increment");
    let ours = DocVersion::new(
        Doc::new(vec![main_def(1), helper(1), other(0)]).expect("distinct ids"),
        renamed_names,
    );
    let theirs = version(vec![main_def(1), helper(4), other(0)]);

    let outcome = merge_documents(&base(), &ours, &theirs);
    assert!(outcome.is_clean(), "{}", outcome.report());
    assert_eq!(outcome.merged.names.get(helper_id()), Some("increment"));
    assert_eq!(
        outcome
            .merged
            .definition(helper_id())
            .map(|d| d.body.clone()),
        Some(helper(4).body)
    );
    assert!(
        outcome
            .ours_changes
            .contains(&(helper_id(), DefChange::Renamed)),
        "the rename was not detected: {:?}",
        outcome.ours_changes
    );
}

#[test]
fn competing_renames_of_the_same_definition_conflict() {
    let mut ours_names = names();
    ours_names.set(helper_id(), "increment");
    let mut theirs_names = names();
    theirs_names.set(helper_id(), "succ");
    let defs = vec![main_def(1), helper(1), other(0)];
    let ours = DocVersion::new(Doc::new(defs.clone()).expect("ids"), ours_names);
    let theirs = DocVersion::new(Doc::new(defs).expect("ids"), theirs_names);

    let outcome = merge_documents(&base(), &ours, &theirs);
    assert!(!outcome.is_clean());
    assert_eq!(outcome.conflicts[0].kind, DocConflictKind::CompetingNames);
}

fn documented(defs: Vec<Def>, docs: DocTable) -> DocVersion {
    DocVersion::documented(Doc::new(defs).expect("distinct ids"), names(), docs)
}

fn doc_table(line: &str) -> DocTable {
    let mut docs = DocTable::new();
    docs.set(helper_id(), line);
    docs
}

#[test]
fn a_doc_line_written_on_one_side_survives_the_merge() {
    let defs = vec![main_def(1), helper(1), other(0)];
    let ours = documented(defs.clone(), doc_table("adds one to a number"));
    let theirs = version(defs);

    let outcome = merge_documents(&base(), &ours, &theirs);
    assert!(outcome.is_clean(), "{}", outcome.report());
    assert_eq!(
        outcome.merged.docs.get(helper_id()),
        Some("adds one to a number")
    );
}

#[test]
fn competing_doc_lines_for_the_same_definition_conflict() {
    let defs = vec![main_def(1), helper(1), other(0)];
    let ours = documented(defs.clone(), doc_table("adds one to a number"));
    let theirs = documented(defs, doc_table("the next number up"));

    let outcome = merge_documents(&base(), &ours, &theirs);
    assert!(!outcome.is_clean());
    assert!(
        outcome
            .conflicts
            .iter()
            .any(|c| c.kind == DocConflictKind::CompetingDocs),
        "{:?}",
        outcome.conflicts
    );
}

#[test]
fn the_same_doc_line_written_on_both_sides_is_not_a_conflict() {
    let defs = vec![main_def(1), helper(1), other(0)];
    let ours = documented(defs.clone(), doc_table("adds one to a number"));
    let theirs = documented(defs, doc_table("adds one to a number"));

    let outcome = merge_documents(&base(), &ours, &theirs);
    assert!(outcome.is_clean(), "{}", outcome.report());
    assert_eq!(
        outcome.merged.docs.get(helper_id()),
        Some("adds one to a number")
    );
}

#[test]
fn a_move_on_one_side_is_detected_and_carried() {
    let ours = version(vec![other(0), main_def(1), helper(1)]);
    let theirs = version(vec![main_def(1), helper(3), other(0)]);

    let outcome = merge_documents(&base(), &ours, &theirs);
    assert!(outcome.is_clean(), "{}", outcome.report());
    assert_eq!(
        outcome.merged.doc.ids(),
        vec![other_id(), main_id(), helper_id()],
        "the move was not carried"
    );
    assert!(
        outcome
            .ours_changes
            .contains(&(other_id(), DefChange::Moved)),
        "the move was not detected: {:?}",
        outcome.ours_changes
    );
    assert_eq!(
        outcome
            .merged
            .definition(helper_id())
            .map(|d| d.body.clone()),
        Some(helper(3).body),
        "theirs' edit was lost while carrying ours' move"
    );
}

#[test]
fn an_addition_on_each_side_keeps_both() {
    let fresh_a = Id::from_u128(0xa1);
    let fresh_b = Id::from_u128(0xb1);
    let mut ours = base();
    let mut ours_defs = ours.doc.defs().to_vec();
    ours_defs.push(Def::new(fresh_a, Ty::Num, Exp::num(10)));
    ours.doc = Doc::new(ours_defs).expect("ids");
    ours.names.set(fresh_a, "ours_new");

    let mut theirs = base();
    let mut theirs_defs = theirs.doc.defs().to_vec();
    theirs_defs.push(Def::new(fresh_b, Ty::Num, Exp::num(20)));
    theirs.doc = Doc::new(theirs_defs).expect("ids");
    theirs.names.set(fresh_b, "theirs_new");

    let outcome = merge_documents(&base(), &ours, &theirs);
    assert!(outcome.is_clean(), "{}", outcome.report());
    assert!(outcome.merged.definition(fresh_a).is_some());
    assert!(outcome.merged.definition(fresh_b).is_some());
    assert!(outcome.merged.is_well_typed());
}

#[test]
fn deleting_a_definition_on_one_side_leaves_holes_not_dangling_references() {
    let ours = version(vec![main_def(1), other(0)]);
    let theirs = base();

    let outcome = merge_documents(&base(), &ours, &theirs);
    assert!(outcome.is_clean(), "{}", outcome.report());
    assert!(outcome.merged.definition(helper_id()).is_none());
    let merged_main = outcome.merged.definition(main_id()).expect("main survives");
    assert!(
        !references(&merged_main.body, helper_id()),
        "a dangling reference survived: {:?}",
        merged_main.body
    );
    assert!(
        outcome.merged.is_well_typed(),
        "the merge left an ill-typed document: {}",
        outcome.merged.render()
    );
}

#[test]
fn deleting_on_one_side_and_editing_on_the_other_conflicts() {
    let ours = version(vec![main_def(1), other(0)]);
    let theirs = version(vec![main_def(1), helper(8), other(0)]);

    let outcome = merge_documents(&base(), &ours, &theirs);
    assert!(!outcome.is_clean());
    assert_eq!(outcome.conflicts[0].kind, DocConflictKind::DeletedAndEdited);
    assert!(outcome.merged.definition(helper_id()).is_some());
}

#[test]
fn competing_annotations_conflict_but_still_produce_a_document() {
    let ours = version(vec![
        main_def(1),
        Def::new(helper_id(), Ty::Hole, helper(1).body),
        other(0),
    ]);
    let theirs = version(vec![
        main_def(1),
        Def::new(
            helper_id(),
            Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Hole)),
            helper(1).body,
        ),
        other(0),
    ]);

    let outcome = merge_documents(&base(), &ours, &theirs);
    assert!(!outcome.is_clean());
    assert!(
        outcome
            .conflicts
            .iter()
            .any(|c| c.kind == DocConflictKind::CompetingAnnotations)
    );
    assert!(outcome.merged.is_well_typed());
}

#[test]
fn an_edit_inside_the_same_definition_still_conflicts_the_way_it_always_did() {
    let ours = version(vec![main_def(2), helper(1), other(0)]);
    let theirs = version(vec![main_def(3), helper(1), other(0)]);

    let outcome = merge_documents(&base(), &ours, &theirs);
    assert!(!outcome.is_clean());
    assert!(matches!(
        outcome.conflicts[0].kind,
        DocConflictKind::WithinDefinition(_)
    ));
}

#[test]
fn deleting_every_definition_on_both_sides_still_leaves_a_document() {
    let empty_ish = version(vec![Def::hole(main_id(), hole(1))]);
    let ours = version(vec![Def::hole(main_id(), hole(1))]);
    let theirs = version(vec![Def::hole(main_id(), hole(1))]);
    let outcome = merge_documents(&empty_ish, &ours, &theirs);
    assert_eq!(outcome.merged.doc.len(), 1);
    assert!(outcome.merged.is_well_typed());
}
