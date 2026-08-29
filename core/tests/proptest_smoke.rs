use proptest::prelude::*;

proptest! {
    #[test]
    fn adding_zero_is_identity(x: i64) {
        let sum: i64 = [x, 0].into_iter().sum();
        prop_assert_eq!(sum, x);
    }
}
