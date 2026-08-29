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

Phase B1 — **documents of named definitions**. A program is no longer one
expression; it is an ordered list of top-level definitions, each with a uuid
identity, a display name in the name table, a type annotation, and a body.

- `core::doc` — `Doc`, `Def`, the document typing rule (every definition
  checks against its own annotation in a context holding all of them, so
  self- and mutual recursion are well-typed), `references`, and `vacate`.
- Six actions (`action::act`): `CreateDefinition`, `DeleteDefinition`,
  `SetDefAnn`, `MoveNextDef`, `MovePrevDef`, `MoveToDef`. A body refers to a
  definition with the existing `Var(Id)` — no new expression form — and
  `DeleteDefinition` rewrites every reference to the dropped definition into
  an empty hole in the same action, so a dangling reference cannot exist
  (`DECISIONS.md`, 2026-08-28, for both choices and their rationale).
- On-disk format version 2 (`FORMAT.md` §3.1, §11): a length-prefixed
  definition list of `id | ann | node_table | root_index`. Version 1 files
  still open — they migrate to a one-definition document named `main`, with
  fifteen committed v1 artifacts asserted to open and round-trip
  (`store/tests/migration.rs`).
- `nothing run` evaluates the definition named `main` and lists the
  document's definitions when there is none. Cross-definition calls resolve
  by id, so factorial is a self-referencing definition that computes 120
  without the Z-combinator (which still works, and is still tested).
- Incremental evaluation keys on transitive definition digests: editing a
  helper re-evaluates its dependents, editing an unused definition
  re-evaluates nothing.
- Three-way merge treats a definition as a unit (`merge::document`):
  renames and moves are detected and carried, two branches editing different
  definitions merge with zero conflicts, and a definition deleted on one side
  leaves holes rather than dangling references.
- The editor grew a definition-list pane and six bindings — `C-n`, `C-d`,
  `C-l`, `C-t`, `C-↑`, `C-↓` (`KEYS.md`) — and the agent protocol reports the
  whole document, per-definition provenance, and definition-aware hole
  contexts (`agentapi/PROTOCOL.md`).
- The five reference fixtures are documents now; `factorial` and `record`
  were rebuilt to say what they always meant, and the benchmark was re-run
  (`bench/RESULTS.md`, 2026-08-28).

Phase B2 — **strings**, the first feature added through the full-thread
checklist rather than as part of building the thread.

- `Ty::Str` and `Exp::Str(String)`: a string is a literal *value*, not a
  name. Its bytes live in the node and are hashed as meaning, where every
  other piece of user-visible text in the system is an identity in the name
  table (`FORMAT.md` §5, §6).
- `Op::Concat` (`++`), typed `Str × Str → Str`, and `Op::Eq` generalised
  from numbers to any single base type — `Num`, `Bool` or `Str`, chosen
  from whichever operand is not a hole. `1 == true` and `f == g` are still
  quarantined (`DECISIONS.md`, 2026-08-29, for the three rules that were
  considered and why unification was not one of them).
- One new action, `ConstructStr(String)`; the sensibility proptest passes
  at 10,000 cases over the enlarged 27-variant grammar, unweakened.
- A quote-delimited string run at the keyboard (`KEYS.md`, settled item 17,
  and column H of `tui/tests/matrix.rs`): `"` opens it, every printable key
  is one character of text, `"` closes it, `"` on a finished string re-opens
  it at the end, and `\` arms a single escape — the one keystroke in the
  editor that commits nothing, and the only one that had to. `&` is the
  join-text key, because `++` cannot be two keystrokes in a grammar that
  spends nothing on lookahead.
- On-disk format version 3 (`FORMAT.md`): four new tags (node `12`, `Ty`
  `5`, `Op` `5`, action `26`) and no layout change, so the version-2 body
  decoder is the version-3 body decoder. Fifteen committed version-2
  artifacts, generated by the unmodified version-2 encoder before it was
  touched, are asserted to open under version 3 (`store/tests/migration.rs`).
- Strings evaluate, in both the small-step and the incremental engines; a
  hole inside a concatenation still blocks cleanly with its environment.
  `nothing run` and `nothing check` needed no code at all.
- The beginner projection reads a string as `the text "hello"` and a join
  as `x followed by y`, with no operator symbols leaking into it.
- A sixth reference program, a greeting formatter (`bench/references.md`
  §6), with its own permanent Neovim baseline of 127 keystrokes, and a
  benchmark re-run at 0.41× (`bench/RESULTS.md`, 2026-08-29). The five
  existing fixtures are byte-identical. `DECISIONS.md` records what the
  feature cost, layer by layer, because measuring that was the point of
  adding it.

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
