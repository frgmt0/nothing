# Changelog

All notable changes to `nothing` are recorded here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

Phases 0–11 of `spec.md` are complete: a bidirectionally-typed core calculus
(`core`) with numbers, booleans, functions, `if`, `let`, and pairs, where
every program is well-typed at every instant, including mid-edit; a
Hazelnut-style action calculus (`action`) with a zipper cursor, movement,
deletion, and one construction action per syntactic form, guaranteeing that
any well-typed program plus any action either fails cleanly or stays
well-typed (verified at 10,000+ proptest cases); a keyboard grammar and
`ratatui` terminal editor (`tui`) driving that calculus with in-scope name
completion ranked by type consistency; UUID-backed binder identity with a
layered, per-user name overlay so rename is a single non-conflicting
metadata write; a small-step evaluator (`eval`) that runs around holes,
producing indeterminate results with the captured blocking environment
instead of failing; a content-addressed binary serialisation format
(`store`, see `FORMAT.md`) with a debug JSON export; incremental evaluation
that caches by node hash and re-evaluates only what an edit actually
dirties; structural diff, move detection, and three-way merge over typed
operations (`merge`) that merges disjoint edits with zero conflicts and
reports true conflicts with both alternatives shown, proven to preserve
well-typedness over 5,000+ random branch pairs; a JSON-over-stdio agent
protocol (`agentapi`) exposing the action calculus, the hole-context query,
and per-node provenance to external drivers, measured against a text-patch
baseline in `bench/AGENT.md`; and alternate projections, including an
auto-selecting state-machine view and a beginner-readable projection, built
on a `Projection` trait the TUI is generic over.

This release starts v0.1.0 productisation work (`spec-build.md`), Phase
B0 — ship infrastructure:

### Added

- CI (`.github/workflows/ci.yml`): `cargo build`, `test`, `clippy -D
  warnings`, and `fmt --check` over the whole workspace on every push and
  pull request.
- Release automation (`.github/workflows/release.yml`): tagged pushes
  (`v*`) build `nothing` for macOS arm64 and Linux x86_64 and attach the
  binaries to a GitHub release.
- The `nothing` binary (`cli` crate, package `nothing-cli`): a single
  installable executable wrapping the existing surfaces as subcommands —
  `nothing edit <file>`, `nothing run <file>`, `nothing check <file>`,
  `nothing repl`, `nothing protocol`, `nothing merge <base> <a> <b>`,
  `nothing --version`.
- `CONTRIBUTING.md` with the full-thread checklist for adding a language
  feature, one checkbox per layer.

### Changed

- Cleared the clippy and `cargo fmt` backlog across the workspace (see
  `CONTRIBUTING.md` for the standing rule against `#[allow]`-silenced
  lints).
- `nothing_tui::term::run` now returns the final `AppState` instead of
  discarding it, so a caller can persist the edited document.
- The action-name REPL (`action` crate) and the JSON protocol runner
  (`agentapi` crate) are now callable library functions
  (`nothing_action::repl::run`, `nothing_agentapi::protocol::run_stdio`),
  with their existing `repl` and `protocol` binaries reduced to thin shims
  so the `cli` crate can reuse the same logic without duplicating it.

[Unreleased]: https://github.com/frgmt0/nothing
