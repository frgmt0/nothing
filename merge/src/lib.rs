pub mod apply;
pub mod bench;
pub mod chain;
pub mod diff;
pub mod merge3;
pub mod ops;
pub mod path;
pub mod repair;
pub mod scenarios;
pub mod text;
pub mod version;

pub use apply::apply_all;
pub use diff::diff;
pub use merge3::{Conflict, ConflictKind, MergeOutcome, merge};
pub use ops::{Operation, Region};
pub use repair::{Repair, RepairKind, repair};
pub use version::Version;
