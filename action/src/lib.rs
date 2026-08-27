//! `nothing-action`: the edit calculus (Phase 2).
//!
//! The central claim of this crate, following Hazelnut (Omar et al., POPL
//! 2017), is that an edit is a judgment
//!
//! ```text
//! (cursor, program) --action--> (cursor', program')
//! ```
//!
//! that carries well-typedness from left to right. Every action either
//! fails cleanly (`None`) or produces a well-typed program. There is no
//! intermediate "broken" state to recover from, because there is no way to
//! reach one.
//!
//! Module map:
//!
//! - [`zipper`] — the cursor. A focused subexpression plus a path of parent
//!   frames sufficient to reconstruct the whole program.
//! - [`act`] — the [`act::Action`] grammar and [`act::apply`].
//! - [`log`] — the action log (provenance, undo, structural diff read from
//!   it). Not yet implemented.
//! - [`cursor_render`] — cursor-aware projection. Not yet implemented.
//! - [`generate`] — a generator for arbitrary *well-typed* programs, used by the
//!   property tests throughout this crate and later phases.
//! - [`script`] — the textual encoding of an action stream, used by the
//!   Phase 3 REPL harness (`src/bin/repl.rs`) and the benchmark fixtures.

pub mod act;
pub mod cursor_render;
pub mod generate;
pub mod log;
pub mod script;
pub mod zipper;
