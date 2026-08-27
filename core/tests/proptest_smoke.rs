//! Smoke test proving the `proptest` harness works in this crate. This is
//! deliberately trivial — the point of Phase 0 is to get the plumbing
//! working before Phase 2 needs it for real.

use proptest::prelude::*;

proptest! {
    #[test]
    fn adding_zero_is_identity(x: i64) {
        prop_assert_eq!(x + 0, x);
    }
}
