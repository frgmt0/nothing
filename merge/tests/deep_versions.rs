use nothing_core::exp::Exp;
use nothing_core::names::NameTable;
use nothing_merge::apply::apply_one;
use nothing_merge::diff::{size, structurally_equal};
use nothing_merge::path::replace_at;
use nothing_merge::{Operation, Version, apply_all, diff};

const CI_STACK_BYTES: usize = 2 * 1024 * 1024;

fn on_a_ci_sized_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(CI_STACK_BYTES)
        .spawn(work)
        .expect("spawn the small-stack thread a CI runner would give a test")
        .join()
        .expect("the small-stack thread finished without overflowing")
}

fn long_list(n: i64) -> Exp {
    Exp::list((0..n).map(Exp::num))
}

#[test]
fn measuring_and_comparing_a_long_list_does_not_overflow() {
    on_a_ci_sized_stack(|| {
        let a = long_list(10_000);
        let b = long_list(10_000);
        assert_eq!(size(&a), 20_001);
        assert!(structurally_equal(&a, &b));
        assert!(!structurally_equal(&a, &long_list(9_999)));
    });
}

#[test]
fn replacing_a_cell_deep_in_a_long_list_does_not_overflow() {
    on_a_ci_sized_stack(|| {
        let list = long_list(10_000);
        let path: Vec<usize> = std::iter::repeat_n(1usize, 9_999).collect();
        let replaced = replace_at(&list, &path, Exp::Nil).expect("the path reaches the last cell");
        assert_eq!(size(&replaced), 19_999);
    });
}

#[test]
fn diffing_and_reapplying_two_long_lists_does_not_overflow() {
    on_a_ci_sized_stack(|| {
        let base = Version::new(long_list(1_000), NameTable::new());
        let mut items: Vec<Exp> = (0..1_000i64).map(Exp::num).collect();
        items[900] = Exp::num(-1);
        let other = Version::new(Exp::list(items), NameTable::new());

        let ops = diff(&base, &other);
        assert!(!ops.is_empty(), "one changed cell is a change");
        let applied = apply_all(&base, &ops);
        assert!(applied.dropped.is_empty(), "every operation applied");
        assert!(structurally_equal(&applied.version.exp, &other.exp));

        let single: Vec<Operation> = ops.into_iter().take(1).collect();
        assert!(apply_one(&base.exp, &single[0]).is_some());
    });
}
