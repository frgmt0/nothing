use nothing_core::exp::{Exp, HoleId, Id, Op};
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;
use nothing_merge::apply::apply_all;
use nothing_merge::diff::{diff, structurally_equal};
use nothing_merge::ops::Operation;
use nothing_merge::version::Version;

fn id(n: u128) -> Id {
    Id::from_u128(n)
}

fn hole(n: u128) -> HoleId {
    HoleId::from_u128(n)
}

fn square_and_bump() -> (Exp, NameTable) {
    let f = id(1);
    let g = id(2);
    let a = id(10);
    let b = id(11);
    let exp = Exp::let_(
        f,
        Exp::lam(a, Ty::Num, Exp::bin_op(Op::Mul, Exp::var(a), Exp::var(a))),
        Exp::let_(
            g,
            Exp::lam(b, Ty::Num, Exp::bin_op(Op::Add, Exp::var(b), Exp::num(1))),
            Exp::bin_op(
                Op::Add,
                Exp::ap(Exp::var(f), Exp::num(3)),
                Exp::ap(Exp::var(g), Exp::num(4)),
            ),
        ),
    );
    let mut names = NameTable::new();
    names.set(f, "square");
    names.set(g, "bump");
    names.set(a, "a");
    names.set(b, "b");
    (exp, names)
}

#[test]
fn a_program_against_itself_with_one_renamed_variable_is_exactly_one_rename() {
    let (exp, names) = square_and_bump();
    let base = Version::new(exp.clone(), names.clone());

    let mut renamed_names = names.clone();
    renamed_names.set(id(10), "value");
    let renamed = Version::new(exp, renamed_names);

    let ops = diff(&base, &renamed);
    assert_eq!(ops.len(), 1, "expected exactly one operation, got {ops:#?}");
    match &ops[0] {
        Operation::Rename {
            id: target,
            from,
            to,
        } => {
            assert_eq!(*target, id(10));
            assert_eq!(from.as_deref(), Some("a"));
            assert_eq!(to, "value");
        }
        other => panic!("expected a Rename, got {other:?}"),
    }

    let replayed = apply_all(&base, &ops);
    assert_eq!(replayed.version.names.get(id(10)), Some("value"));
    assert_eq!(replayed.version.exp, renamed.exp);
}

#[test]
fn a_renamed_binder_used_in_forty_places_is_still_one_operation() {
    let x = id(7);
    let mut body = Exp::var(x);
    for _ in 0..39 {
        body = Exp::bin_op(Op::Add, Exp::var(x), body);
    }
    let exp = Exp::let_(x, Exp::num(1), body);
    let mut names = NameTable::new();
    names.set(x, "n");
    let base = Version::new(exp.clone(), names.clone());
    let after = Version::new(exp, names.with(x, "total"));

    let ops = diff(&base, &after);
    assert_eq!(ops.len(), 1, "{ops:#?}");
    assert!(matches!(ops[0], Operation::Rename { .. }));
}

#[test]
fn moving_a_function_to_a_different_position_is_a_one_operation_diff() {
    let f = id(20);
    let function = Exp::lam(f, Ty::Num, Exp::bin_op(Op::Mul, Exp::var(f), Exp::var(f)));
    let mut names = NameTable::new();
    names.set(f, "n");

    let before = Version::new(
        Exp::pair(function.clone(), Exp::empty_hole(hole(1))),
        names.clone(),
    );
    let after = Version::new(Exp::pair(Exp::empty_hole(hole(2)), function.clone()), names);

    let ops = diff(&before, &after);
    assert_eq!(ops.len(), 1, "expected one operation, got {ops:#?}");
    match &ops[0] {
        Operation::Move { from, to, node, .. } => {
            assert_eq!(from, &vec![0]);
            assert_eq!(to, &vec![1]);
            assert_eq!(node, &function);
        }
        other => panic!("expected a Move, got {other:?}"),
    }

    let replayed = apply_all(&before, &ops);
    assert!(structurally_equal(&replayed.version.exp, &after.exp));
    assert!(replayed.dropped.is_empty());
}

#[test]
fn reordering_two_let_bindings_is_a_one_operation_diff() {
    let (exp, names) = square_and_bump();
    let before = Version::new(exp, names.clone());

    let f = id(1);
    let g = id(2);
    let a = id(10);
    let b = id(11);
    let swapped = Exp::let_(
        g,
        Exp::lam(b, Ty::Num, Exp::bin_op(Op::Add, Exp::var(b), Exp::num(1))),
        Exp::let_(
            f,
            Exp::lam(a, Ty::Num, Exp::bin_op(Op::Mul, Exp::var(a), Exp::var(a))),
            Exp::bin_op(
                Op::Add,
                Exp::ap(Exp::var(f), Exp::num(3)),
                Exp::ap(Exp::var(g), Exp::num(4)),
            ),
        ),
    );
    let after = Version::new(swapped, names);

    let ops = diff(&before, &after);
    assert_eq!(ops.len(), 1, "expected one operation, got {ops:#?}");
    match &ops[0] {
        Operation::MoveBinding {
            binder,
            from_index,
            to_index,
            ..
        } => {
            assert_eq!(*binder, f);
            assert_eq!(*from_index, 0);
            assert_eq!(*to_index, 1);
        }
        other => panic!("expected a MoveBinding, got {other:?}"),
    }

    let replayed = apply_all(&before, &ops);
    assert!(structurally_equal(&replayed.version.exp, &after.exp));
}

#[test]
fn a_move_is_not_reported_as_a_delete_plus_an_insert() {
    let f = id(20);
    let function = Exp::lam(f, Ty::Num, Exp::bin_op(Op::Mul, Exp::var(f), Exp::var(f)));
    let before = Version::new(
        Exp::pair(function.clone(), Exp::empty_hole(hole(1))),
        NameTable::new(),
    );
    let after = Version::new(
        Exp::pair(Exp::empty_hole(hole(2)), function),
        NameTable::new(),
    );
    let ops = diff(&before, &after);
    assert!(
        !ops.iter()
            .any(|op| matches!(op, Operation::DeleteToHole { .. } | Operation::Fill { .. })),
        "the move was reported as a delete plus an insert: {ops:#?}"
    );
}

#[test]
fn an_unchanged_program_has_an_empty_diff() {
    let (exp, names) = square_and_bump();
    let version = Version::new(exp, names);
    assert!(diff(&version, &version).is_empty());
}

#[test]
fn reformatting_is_invisible_to_the_diff_because_there_is_no_formatting() {
    use nothing_merge::text::{CANONICAL, WIDE, to_text};
    let (exp, names) = square_and_bump();
    let version = Version::new(exp, names);
    assert_ne!(to_text(&version, CANONICAL), to_text(&version, WIDE));
    assert!(diff(&version, &version).is_empty());
}

#[test]
fn wrapping_an_expression_is_reported_as_an_insert() {
    let names = NameTable::new();
    let inner = Exp::bin_op(Op::Mul, Exp::num(3), Exp::num(4));
    let before = Version::new(inner.clone(), names.clone());
    let after = Version::new(
        Exp::if_(Exp::bool_(true), inner.clone(), Exp::num(0)),
        names,
    );
    let ops = diff(&before, &after);
    assert_eq!(ops.len(), 1, "{ops:#?}");
    match &ops[0] {
        Operation::Insert { slot, .. } => assert_eq!(*slot, 1),
        other => panic!("expected an Insert, got {other:?}"),
    }
    let replayed = apply_all(&before, &ops);
    assert!(structurally_equal(&replayed.version.exp, &after.exp));
}

#[test]
fn unwrapping_an_expression_is_reported_as_a_delete() {
    let names = NameTable::new();
    let inner = Exp::bin_op(Op::Mul, Exp::num(3), Exp::num(4));
    let before = Version::new(
        Exp::if_(Exp::bool_(true), inner.clone(), Exp::num(0)),
        names.clone(),
    );
    let after = Version::new(inner, names);
    let ops = diff(&before, &after);
    assert_eq!(ops.len(), 1, "{ops:#?}");
    assert!(matches!(ops[0], Operation::Delete { slot: 1, .. }));
    let replayed = apply_all(&before, &ops);
    assert!(structurally_equal(&replayed.version.exp, &after.exp));
}

#[test]
fn changing_a_lambda_annotation_is_a_shape_operation() {
    let x = id(3);
    let names = NameTable::new();
    let before = Version::new(Exp::lam(x, Ty::Num, Exp::var(x)), names.clone());
    let after = Version::new(Exp::lam(x, Ty::Hole, Exp::var(x)), names);
    let ops = diff(&before, &after);
    assert_eq!(ops.len(), 1, "{ops:#?}");
    assert!(matches!(ops[0], Operation::SetAnn { .. }));
}
