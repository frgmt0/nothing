//! `nothing-core`: the AST, type grammar, and typing rules for the `nothing`
//! projectional language. This crate has no dependency on any editor or
//! rendering surface — it is the thing that stays true regardless of how a
//! program is viewed or edited.

pub mod ty;
pub mod exp;
pub mod ctx;
pub mod typing;
pub mod examples;
pub mod render;
