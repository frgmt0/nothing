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

Phase B2 — **lists**, the second feature through the full-thread checklist
and the first one with *structure*.

- `Ty::List(τ)`, consistent with `List σ` exactly when `τ` is consistent
  with `σ`, and `matched_list` alongside `matched_arrow` and `matched_prod`.
- `Exp::Nil`, `Exp::Cons(head, tail)` — rendered right-associatively as
  `1 :: 2 :: nil`, at a precedence between comparison and addition — and
  `Exp::Fold(list, init, step)` as the sole eliminator, a primitive rather
  than a builtin definition because a builtin would need polymorphism the
  language does not have and would type as all-`?` mush (`DECISIONS.md`,
  2026-08-29). The step function is `elem -> acc -> acc` and the fold is a
  right fold, so B4 can write `map f = fold xs nil (λx. λacc. f x :: acc)`
  without a `reverse`.
- A cons chain settles on **one** element type, taken as the `join` of the
  head's type and the tail's element type, in both `syn` and `ana`; so
  `1 :: 2 :: ⦇⦈` knows the hole is a `List Num`, and `true :: 1 :: nil`
  quarantines rather than being refused.
- Three new actions — `ConstructNil`, `ConstructCons`, `ConstructFold` —
  and five new zipper frames. The sensibility proptest passes at 10,000
  cases over the enlarged 30-variant grammar, unweakened, and reachability
  is still constructive over the sixteen-variant expression grammar.
- Three keyboard bindings (`KEYS.md`, settled item 18): `:` is cons
  everywhere it previously did nothing (it still opens a written lambda's
  annotation slot), `/` is fold, and `[` is the `List` prefix inside an
  annotation slot. `nil` is a completion candidate, not a key, exactly as
  `true` and `false` are.
- Lists evaluate in both engines. `1 :: 2 :: ⦇⦈` is an indeterminate list
  that reports its hole; a fold folds every element it can reach before
  blocking on one; fuel still guards a fold over a long list; and the
  incremental engine walks the cons spine iteratively, with salted
  per-element digests, so a 1 200-element fold neither overflows the stack
  nor poisons the cache.
- On-disk format version 4 (`FORMAT.md`): node tags `13`–`15`, `Ty` tag
  `6`, action tags `27`–`29`, and no layout change. Sixteen committed
  version-3 artifacts, generated by the unmodified version-3 encoder before
  it was touched, are asserted to open under version 4; the reader now
  migrates v1, v2 and v3 (`store/tests/migration.rs`).
- Cons chains diff as spines: `merge/src/chain.rs`'s longest-common-
  subsequence is now generic and returns index pairs, and `diff_spine`
  aligns two chains by content hash, so appending is one `Insert` and
  splicing into the middle is one `Insert`. A new `Region::Cell` says that
  a spine link is not the sublist below it, which is what lets two branches
  growing the same list in different places merge cleanly.
- Reference program 2 (`list_map`) is a **real list map** at last —
  `fold x1 nil (λx2. λx3. x0 x2 :: x3)` instead of map-over-a-pair. The
  permanent Neovim denominator (114) is untouched; the ratio went *up*,
  0.25× to 0.39×, because the fixture is now the program the reference
  describes (`bench/RESULTS.md`, `bench/references.md` §2).
- The beginner projection reads a list as "1 in front of 2 in front of an
  empty list" and a fold as "combining …, starting from …, with …".
  `nothing run` and `nothing check` again needed no code.

Phase B3 — **effects**. `main` may now have a command type, and a command is
a *value*: the editor renders it, the live-values pane shows it, pure
evaluation treats a finished `bind` chain as a value and does not perform it.
Only `nothing run` performs one.

- `Ty::Cmd(τ)`, consistent with `Cmd σ` exactly when `τ` is consistent with
  `σ`, and `matched_cmd` alongside `matched_arrow`, `matched_prod`,
  `matched_list`, `matched_record` and `matched_variant` — failing open on
  `?` like every other member of the family.
- Four expression forms, primitives rather than builtins for the reason
  `fold` is a primitive: `print e : Cmd {}`, `readline : Cmd Str`,
  `pure e : Cmd τ`, and `bind x <- c in k`, spelled and bracketed exactly
  like `let` so B3 adds no new precedence constant (`DECISIONS.md`,
  2026-08-29). `bind` was chosen over `seq` because `seq` *is* `bind` with
  an unmentioned binder, and every checklist layer would have paid twice.
- Bidirectional rules that make the holes useful: at `bind ⦇⦈ in ⦇⦈` the
  command slot expects `Cmd ?` and the body slot expects the whole form's
  expected type, and inside the body the binder is in scope at whatever the
  command yields.
- Four new actions (`ConstructPrint`, `ConstructReadline`, `ConstructPure`,
  `ConstructBind`) and four new zipper frames. The sensibility proptest
  passes at 10,000 cases over the enlarged grammar, unweakened.
- Three keyboard bindings (`KEYS.md`, settled item 21): `$` is print, `'` is
  pure, `>` is bind on a node (and still the arrow inside an annotation
  slot), and `c` is the `Cmd` prefix in an annotation slot. `readline` is a
  completion candidate rather than a key, because the live-committed name
  run re-commits as `Delete` + candidate and that is only sound for a leaf —
  which is the rule that decides all four.
- On-disk format version 7 (`FORMAT.md`): node tags `20`–`23`, `Ty` tag `9`,
  action tags `44`–`47`, and no layout change. Seventeen committed
  version-6 artifacts, generated by the unmodified version-6 encoder before
  it was touched, are asserted to open under version 7; the reader now
  migrates v1 through v6 (`store/tests/migration.rs`).
- `nothing run [--fuel N] <file>` **performs** a `main` whose type is a
  command, dispatching on `join(annotation, syn(body))` so that a `main`
  annotated `Cmd {}` whose body is still a hole runs and reports the hole.
  `print` writes a line to stdout, `readline` reads one from stdin, `bind`
  sequences, `pure` yields; a `main` that is not a command evaluates and
  prints exactly as before. One shared fuel budget, default 200 000, covers
  both evaluation steps and performed commands, so an endless `bind` loop
  stops with a message naming the flag instead of hanging.
- Effects around holes: a run performs everything up to the first hole and
  then stops with the ordinary indeterminate report, whose residual is the
  part of the program that has *not* run — the pending continuations folded
  back around the blocked command. A program that prints, hits a hole, and
  would print again performs exactly the first print.
- Reference program 7 (`greeting_command`) — `bind line <- readline in
  print ("hello, " ++ line)` — the first reference that *does* anything, at
  0.45× its 66-keystroke Neovim denominator (`bench/references.md` §7,
  `bench/RESULTS.md`).
- An end-to-end proof (`cli/tests/authoring.rs`): the real `nothing edit`
  TUI is driven over a pty, a hello-world command is typed into it, ctrl-q
  saves, and `nothing run` on the saved file prints exactly the text that
  was typed.
- The beginner projection reads a command as "print the text …" and
  "…, then with the result named x, …". The state-machine projection is
  untouched — a command is not a state machine.

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

### Fixed

- **A long-enough list no longer aborts the process.** Every walker in the
  small-step evaluator recursed once per cons cell, so a 400-element fold
  overflowed a 2 MB Linux CI stack and a longer list would have aborted
  `nothing run` or the TUI live pane on a user's machine. `is_value`,
  `size`, the blocked-hole collector and the incremental engine's value
  walkers are now explicit worklists; `elaborate`, `subst`, `to_exp`,
  `step` and the `store` node-table builder and decoder walk cons spines,
  `let`/`bind` chains and operator spines iteratively; and every public
  entry point in `eval`, `core`, `store`, `merge` and `action`, plus
  `nothing`'s `main` and the TUI event loop, runs on a 256 MB worker thread
  via the new
  `nothing_core::stack::on_deep_stack` (re-entrant, so nested calls and run
  loops never spawn a second one). A 50,000-element list now evaluates,
  serialises and round-trips; `nothing run` survives even with a 2 MB main
  stack. New regression tests (`eval/tests/deep_programs.rs`,
  `store/tests/deep_documents.rs`, `merge/tests/deep_versions.rs`,
  `cli/tests/deep_programs.rs`) run their bodies on an explicit 2 MB thread
  so a macOS pass cannot mask a Linux abort. See `DECISIONS.md`,
  2026-08-29.
- `nothing_eval::perform_doc` / `perform_in` take `&mut (dyn Io + Send)`,
  and `nothing_tui::live::EngineHandle` holds an `Arc<Mutex<_>>` rather than
  an `Rc<RefCell<_>>`, since both now run on the deep-stack worker.

## Phase B4 — the standard library

### Added

- **A standard library, shipped inside the binary.** Thirty-seven
  definitions — `not`/`and`/`or`, `abs`/`negate`/`succ`/`min`/`max`/`clamp`,
  `lte`/`gt`/`between`, `is_empty`/`length`/`sum`/`append`/`reverse`/`map`/
  `filter`/`any`/`all`/`count`/`head_or`/`maximum`/`take`/`drop`/`range`,
  `concat_all`/`join`/`repeat_str`/`is_blank`, `swap`/`map_fst`/`uncurry`,
  and `print_labelled`/`ask`/`print_all` — every one of them well-typed,
  hole-free and documented. `stdlib/std.n` is a 54 KB format-v8 document
  `include_bytes!`d into the binary.
- **It was built with the product, and the file proves it.** `std.n` carries
  the 1,289-entry action log that produced it, written through
  `nothing protocol` with `--no-stdlib`. A test replays every entry from the
  empty program and asserts the result is the committed document, doc table
  and name table — then re-encodes and asserts the bytes match.
- **The library is an ambient prelude, not an import.** Its definitions are
  in the typing context and in the name and doc tables of every session by
  default, so completion offers them and `construct-var` resolves them; a
  saved document stores none of them, only ordinary `Var` nodes holding
  their fixed ids. `nothing protocol --no-stdlib` starts with an empty
  prelude, the way the library itself was built. See `DECISIONS.md`,
  2026-08-29, "The standard library is an ambient prelude, not an import".
- **Doc lines.** A `DocTable` beside the name table, keyed by definition id;
  `Action::SetDoc(id, line)` writes one, is total, and costs one log entry.
  Docs are metadata, never AST: no type, no hash, no node. Threaded through
  the store (format v8), the merge engine (a `CompetingDocs` conflict), the
  protocol (`state`, `hole_context`) and the TUI, where the highlighted
  candidate's doc gets a line of its own under the status line and stdlib
  candidates are marked `std·`.
- **`nothing doc`** — a new subcommand rendering a document as a Markdown
  reference: a summary table, then per definition its name, doc line, type
  in words, source, and beginner-projection reading. With no argument it
  renders the embedded standard library. `stdlib/REFERENCE.md` is generated
  by it and committed, with a test asserting the two match.
- **Format version 8** — a doc-table section between the name table and the
  action log, and action tag 48. Versions 1–7 still open; eighteen v7
  fixtures generated by the *unmodified* v7 encoder are committed under
  `store/fixtures/v7/`, with a test asserting none of them carries a doc
  line. See `FORMAT.md` §7.1 and §11.
- **`FRICTION2.md`** — the second dogfooding session: 286 accepted actions
  through the protocol building a seven-definition todo report that calls
  thirteen stdlib definitions and defines none of them; 22 friction points,
  five fixed.

### Changed

- **The protocol answers `ok: false` when a request did not do what it
  asked.** An action that does not apply, a step that stops a `script`, an
  `undo` with nothing to undo and a `redo` with nothing to redo all now
  report `ok: false` with an error, matching what an unresolvable name
  already did. Previously an unapplied action answered `ok: true` *with* an
  error string, which silently destroyed two subtrees during the friction
  session.
- **The protocol no longer repeats the standard library in every reply.**
  `state.names` and `state.docs` carry the document's own layer plus the
  prelude ids it actually references; `state.stdlib_count` says how many
  more there are, and a new `stdlib` method hands over the catalogue once.
  An empty document's `state` went from 17,555 bytes back to 838.
- **New protocol methods `stdlib` and `move_to_hole`.** `move_to_hole`
  walks to the next (or previous) hole or quarantine the way the TUI's `Tab`
  does — by the same code, now that `index_path` and `moves_between` live in
  `action::zipper` — logging ordinary moves rather than a hidden jump.
  `state` also gained `holes` (cursor paths) and document-wide hole counts.
- `nothing run`, `nothing check` and `nothing edit` all put the standard
  library in scope; `nothing check` reports how much of it is there.

## Phase B5 — tooling that meets people where they are

### Added

- **`.n` files diff and merge inside ordinary git.** Three new subcommands
  and a documented recipe in the new **`GIT.md`**:
  - `nothing merge-driver <base> <ours> <theirs> [<marker-size>] [<path>]`
    is a real git merge driver (`merge.nothing.driver`). It decodes all
    three sides, runs the Phase 9 structural three-way merge over them —
    name table and doc table included — checks the result is well-typed with
    the standard library in scope, writes it back over `%A` and exits 0. A
    conflict prints every conflict on stderr, leaves `%A` as git wrote it,
    and exits 1; so does an undecodable file or an ill-typed result. It
    never writes an ill-typed document and never panics on a garbled blob.
  - `nothing textconv <file>` is git's `textconv` hook
    (`diff.nothing.textconv`): a stable, deterministic structural rendering
    — one definition at a time, name and type, doc line, then the body one
    syntactic group per line — so `git diff`, `git show` and `git log -p`
    show ordinary hunks instead of `Binary files ... differ`.
  - `nothing diff-driver` is git's external diff command
    (`diff.nothing.command`): the typed `Operation`s the structural diff
    recovered, in three ordered sections (`names`, `documentation`,
    `definitions`), with added/removed/renamed/re-annotated/moved
    definitions named and each edited one followed by a line per operation.
    Handles the seven-argument form, the nine-argument rename form, the
    one-argument unmerged form, and a `/dev/null` or undecodable side.
- **`cli/tests/git_integration.rs`** drives real `git` in a scratch
  repository on every `cargo test`: the same merge conflicts *without* the
  driver and succeeds *with* it (so the test cannot pass vacuously),
  disjoint edits inside one definition merge, a rename against a body edit
  merges, an addition against an edit merges, two edits to the same node
  still conflict, `git log -p` shows the structural rendering, and the
  external driver prints typed operations under `--ext-diff`. It skips
  rather than fails when `git` is absent. `cli/tests/deep_programs.rs`
  gained small-main-stack cases for `textconv` and `diff-driver`.
- **The agent protocol is frozen at version 1.** A new `version` method
  answers `protocol_version` (the major number as a string), `protocol_major`
  and `protocol_minor` as integers, and `implementation_version` for the
  build. `agentapi/PROTOCOL.md` gained a "Protocol version 1" section
  stating what v1 guarantees and which changes are additive versus breaking,
  and was brought back in line with what the handler actually emits — the
  method table was missing `stdlib` and `move_to_hole`, and the `state` and
  `hole_context` examples were missing eleven fields between them.
- **A backwards-compatibility test that fails on any breaking change.**
  `agentapi/tests/protocol_v1_compat.rs` drives the real `protocol::handle`
  with a real `AgentSession` over 22 golden fixtures in
  `agentapi/fixtures/protocol/v1/` — one per method in `METHODS`, plus five
  error shapes. Fixtures pin the *shape* (a sorted `path: type` line per JSON
  path, array elements unioned) rather than values, since ids and timestamps
  are freshly generated. A removed path or a changed type fails and names
  itself; an added path is reported as an allowed additive change. Two
  further tests stop the fixtures rotting: every advertised method must have
  a fixture, and the directory must hold exactly the pinned cases.
  `NOTHING_UPDATE_FIXTURES=1` regenerates them.
- **`nothing mcp` — an MCP server over stdio.** JSON-RPC 2.0, hand-rolled on
  the existing `agentapi::json`, no new dependencies: `initialize` (protocol
  versions `2025-06-18`, `2025-03-26`, `2024-11-05`), `tools/list`,
  `tools/call`, `ping`, and notifications answered with silence as the spec
  requires. Fifteen tools mirror the agent protocol — `get_state`,
  `get_projection`, `hole_context`, `apply_action`, `apply_actions`,
  `save_document`, `load_document`, `typecheck`, `run`, `stdlib`,
  `action_grammar`, `undo`, `redo`, `reset`, `move_to_hole` — every one of
  them routed through `protocol::handle`, so the MCP layer owns no editor
  semantics of its own. A failing tool returns `isError: true` in the
  result; JSON-RPC errors are reserved for protocol-level failures.
- **`cli/tests/mcp.rs`** speaks MCP over stdio to the spawned binary: the
  handshake, that `notifications/initialized` produces no reply, the tool
  list, building a program through tool calls and saving it, reloading it in
  a *fresh* server process and confirming it is well-typed and hole-free,
  that `run` does not corrupt the stream, and that malformed lines and
  unknown tools are survivable.
- **`bench/MCP.md`** documents the server, the tool reference, the
  `claude mcp add` command and the raw JSON config, with a worked example
  labelled as the scripted run it is.
- **A second agent benchmark, at post-B2 scale.** `bench/AGENT.md` gained a
  dated 2026-08-29 section: 32 new tasks over programs using strings, lists,
  records and `match` (targets averaging 90 rendered characters against 17
  for the first set), with condition A driven as an interactive
  one-action-per-call loop and a third arm B2 giving the text baseline the
  same interactive treatment. 385 real model calls, transcript in
  `bench/agent-transcripts/post-b2-invalid-edit-rate.jsonl`.
  **The text baseline won again**: 0 invalid edits against the protocol's 9,
  and 30/32 and 31/32 targets against 23/32. The protocol's own rate fell
  from 11.4 % to 2.9 % on programs five times the size, and 0 of its 320
  steps left an ill-typed program. Reported as it came out.

### Changed

- `nothing merge -o` no longer drops the doc table. It built its
  `DocVersion`s with `DocVersion::new` and wrote the result with
  `Document::from_doc`, both of which discard doc lines; it now uses
  `DocVersion::documented` and `Document::documented`.
- The run and check logic shared by `nothing run`, `nothing check` and the
  MCP tools was factored into `run_cmd::perform_or_evaluate` and
  `check::check_document` so the CLI and the MCP server cannot disagree
  about what "well-typed" or "out of fuel" means.

### Human-required

- `bench/agent-transcripts/mcp-session.md` is a marked placeholder. Phase
  B5's done-when for the MCP server asks for a real Claude Code session
  transcript; an agent cannot start a Claude Code session against its own
  host, and inventing one would be a fabrication. `bench/MCP.md` carries the
  step-by-step instructions for a maintainer to run it and commit the
  result.

[Unreleased]: https://github.com/frgmt0/nothing
