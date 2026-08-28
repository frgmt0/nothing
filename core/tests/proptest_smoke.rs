use proptest::prelude::*;

proptest! {
    #[test]
    fn adding_zero_is_identity(x: i64) {
        prop_assert_eq!(x + 0, x);
    }
}
