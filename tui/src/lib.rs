//! `nothing-tui`: the terminal editor surface (Phase 4).
//!
//! The editor is a projection of the AST, not a text buffer: there is no
//! string being parsed anywhere in this crate. A keystroke is a function of
//! what the cursor is on, it expands to primitive `nothing_action` actions,
//! and the screen is re-derived from the resulting program. `KEYS.md` at
//! the repository root is the authoritative grammar; this crate implements
//! it.
//!
//! # Architecture — why the terminal is the thin part
//!
//! ```text
//!   crossterm event ─▶ keys::handle_key(KeyEvent, AppState) -> AppState
//!                            │  pure: no I/O, no globals, no clock
//!                            ├─▶ app::AppState::apply_actions(&[Action])
//!                            │        the one door to the calculus
//!                            └─▶ render::program_line / status_line
//!                                     pure text, then ratatui lays it out
//! ```
//!
//! [`term`] is the only module that touches a terminal, and it contains no
//! decisions. Everything else is a pure function of values, so the whole
//! keyboard grammar is tested headlessly — arrows and slots in
//! [`keys`]'s and [`app`]'s test modules, and the *visible* result through
//! `ratatui`'s `TestBackend` in [`render::render_to_string`] and
//! `tests/movement.rs`.
//!
//! # Module map
//!
//! - [`app`] — [`app::AppState`], the editor as a value: the zipper cursor,
//!   the binder [`app::Slot`], the live token run, the undo history, and the
//!   editor-level movement and climbing that expand to primitive actions.
//!   Read its module docs before adding state.
//! - [`keys`] — the pure key handler. One arm per binding, and
//!   `KEYS.md`'s printable-character matrix.
//! - [`complete`] — the candidate list behind a name run: in-scope binders
//!   plus `true`/`false`, filtered by prefix and ranked by type consistency
//!   with the expected type at the cursor.
//! - [`annot`] — the annotation slot's type entry, where every prefix of
//!   what you have typed is a real type.
//! - [`history`] — per-keystroke undo, as truncate-and-replay over the
//!   primitive action log.
//! - [`keyscript`] — the `.keys` fixture format: one keystroke token per
//!   line. The benchmark counts its lines; the tests replay them.
//! - [`render`] — the projection with the cursor in it, the status line
//!   `KEYS.md` requires, and the viewport: the projection is wrapped here
//!   rather than by `ratatui`, so the window onto it can be the one that
//!   contains the cursor's line.
//! - [`term`] — raw mode, the event loop, and the panic hook that undoes
//!   both.

pub mod annot;
pub mod app;
pub mod complete;
pub mod history;
pub mod keys;
pub mod keyscript;
pub mod render;
pub mod term;

pub use app::{AppState, Slot};
