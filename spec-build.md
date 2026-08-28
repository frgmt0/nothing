# spec-build.md

**Goal:** v0.1.0 — the first version of `nothing` a stranger can install, learn, and build a real program in, without ever meeting a syntax error.

`spec.md` proved the thesis: the calculus is sound, the editor beats Neovim on keystrokes, the merge engine beats git 13/16 to 2/16, and a model drove the editor 16-for-16 without producing an ill-typed state. This document is the other 80%: the unglamorous distance between a proven core and a usable language. It is stricter than spec.md, because productising is where projects rot — a research artifact can skip the boring parts; a v0.1.0 cannot.

---

## How to use this document

Same rules as spec.md: every task has a falsifiable **Done when**. Do not batch, do not skip test tasks, do not start a phase early. Phases are ordered by dependency. When a task turns out to be wrong, the fix is a dated DECISIONS.md entry, not silent drift.

**Timebox:** Phases B0–B6 are the core of v0.1.0 and should take roughly ten weeks at a real pace. B7 is the release gate. If B0–B6 exceed sixteen weeks, cut scope (drop tasks, not quality) and ship.

---

## Design commitments

Decided now, relitigated only via DECISIONS.md:

- **The invariants are non-negotiable.** No parser anywhere in the pipeline. Every program well-typed at every instant. Every edit an action. If a feature cannot be built without breaking one of these, the feature is out of v0.1.0.
- **The full-thread rule.** A language feature does not exist until it is threaded through *every* layer: type grammar, consistency, syn/ana, expression grammar, zipper, actions (with sensibility re-established at 10k cases), rendering, keys (KEYS.md updated), serialisation (FORMAT.md updated, version bumped), content addressing, evaluation (including around holes), incremental caching, diff/merge, the hole-context query, and provenance. A feature merged half-threaded is the project's definition of broken. No second feature starts while a first is half-threaded.
- **Gradual typing stands in for polymorphism in v0.1.0.** `map` types as `(? -> ?) -> List ? -> List ?` via holes, not via quantifiers. Parametric polymorphism is a v0.2 question. Write the revisit trigger into DECISIONS.md: if the stdlib accumulates ten functions whose types are all-`?` mush, the decision was wrong.
- **One binary.** `nothing` is a single installable executable (`cargo install nothing-lang`, plus prebuilt binaries). The workspace crates stay as libraries; the product is one command.
- **The TUI remains the only surface for v0.1.0.** No web editor. The agent protocol is the second interface, and it is a first-class product surface, not a debug tool.
- **Format stability starts now.** From the first B1 version bump, every format change ships with a migrating reader for the previous version. Files created by v0.1.0 must open in v0.2.0.

---

## Phase B0 — Ship infrastructure

Boring. First. A project that cannot release cannot be used.

- [x] **CI on every push.** GitHub Actions: `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo fmt --check`. **Done when** a push with a failing test or a clippy warning goes red.
- [x] **Fix the workspace to pass that CI.** Clippy and fmt have never been enforced; there will be a backlog. **Done when** CI is green on main with no allow-attributes added to silence real findings.
- [x] **The `nothing` binary.** One bin crate wrapping the existing surfaces as subcommands: `nothing edit <file>`, `nothing run <file>`, `nothing check <file>`, `nothing repl`, `nothing protocol`, `nothing merge <base> <a> <b>`, `nothing --version`. **Done when** every subcommand works against a file produced by `nothing edit`, and `--help` for each fits on one screen.
- [x] **Release automation.** Tagged builds produce macOS (arm64) and Linux (x86_64) binaries as CI artifacts. A `CHANGELOG.md` exists with an Unreleased section. **Done when** pushing a tag yields downloadable binaries a fresh machine can run.
- [x] **Write the full-thread checklist into `CONTRIBUTING.md`** as a literal copy-paste checklist for adding a language feature, with one checkbox per layer. **Done when** the checklist exists and B2 uses it verbatim four times.

---

## Phase B1 — Definitions

The largest structural gap: a program is currently one expression. A usable language is a set of named definitions. This changes the format — do it before the stdlib exists to be migrated.

- [ ] **Define the document model.** A document is a set of top-level definitions: id, name (in the name table), type annotation, body expression. References to definitions are by id, like variables. Recursion between definitions is permitted (this quietly retires the Z-combinator for everyday use; keep the combinator working — it is a regression test that gradual self-application still types). **Done when** the model is written up in FORMAT.md v2 before implementation, including the migration path for v1 single-expression files.
- [ ] **Thread definitions through the calculus.** New actions: create/delete definition, edit annotation, move cursor across definitions. Sensibility proptest re-established at 10k over documents, not expressions. **Done when** the proptest passes and deleting a definition that others reference leaves the referents as well-typed non-empty or empty holes, never a dangling id.
- [ ] **Thread definitions through evaluation.** `nothing run` evaluates a designated `main` definition; cross-definition calls resolve by id; incremental caching keys on definition body hashes so editing one definition re-evaluates only its dependents (counter-verified, same discipline as Phase 8). **Done when** a two-definition program (`main` calling `helper`) runs, and editing `helper` re-evaluates `main` but editing an unused definition re-evaluates nothing.
- [ ] **Thread definitions through merge and the editor.** Diff treats a definition as a move/rename-detectable unit; the TUI gets a definition list pane with keyboard navigation (KEYS.md updated). **Done when** two branches editing different definitions merge with zero conflicts, and the fixture programs are rebuilt as multi-definition documents with the keystroke benchmark re-run and recorded.
- [ ] **Migration test.** Every serialised file in the repo (fixtures, examples) written in format v1 opens in v2 and round-trips. **Done when** a test proves it for all of them.

---

## Phase B2 — The data types a real program needs

Four features, applied one at a time through the full-thread checklist. The spec.md prediction was that these get cheap once the calculus is solid; this phase is where that prediction is tested. Record the actual cost of each in DECISIONS.md — it is the honest measure of the architecture.

- [ ] **Strings.** Literal entry in the editor is the hard part (a quote-delimited literal-entry mode in KEYS.md, designed before code, same discipline as Phase 4). Concatenation and equality only; no interpolation. **Done when** full-thread checklist complete and the keystroke benchmark gains a string-using reference program with a recorded ratio.
- [ ] **Lists.** `List τ` type, nil/cons constructors, and a fold primitive as the sole eliminator (map and filter become stdlib, written in the language, in B4). **Done when** full-thread complete and `1 :: 2 :: ⦇⦈` evaluates to an indeterminate list that reports the hole.
- [ ] **Records.** Named fields — names as metadata, like binders: field identity is an id, display name lives in the name table, so renaming a field project-wide is one action that cannot fail and cannot conflict in a merge. This is the feature Kowo's Rules HQ pitch stands on; build it like it matters. **Done when** full-thread complete, and the Phase 9 benchmark gains a scenario where two branches rename and reorder fields of the same record and merge cleanly — with the same scenario shown conflicting under `git merge-file`.
- [ ] **Variants and match.** Sum types with id-identified constructors, and a `match` expression. Match is the action-grammar stress test: constructing a match on a variant type auto-generates one arm per constructor, each a hole — exhaustiveness by construction, not by warning. Adding a constructor to a variant inserts a hole arm into every match on it (the action does this; the invariant holds). **Done when** full-thread complete, a match with a missing-arm state is unrepresentable, and the sensibility proptest passes at 10k over the enlarged grammar.

---

## Phase B3 — Effects

A language that cannot print is a calculator. Smallest honest design, decided before built.

- [ ] **Write the effect design into DECISIONS.md before code.** Committed direction: `main` may have type `Cmd` — a value describing effects (print, read line, read file, args), executed only by the `nothing run` runtime. Pure evaluation, the live-values pane, and holes are untouched: a `Cmd` renders as a value in the editor and *executes* only under `run`. No effect polymorphism, no async. **Done when** the entry exists with the rejected alternatives (direct side-effecting builtins; monadic sugar) and why.
- [ ] **Implement `Cmd`.** Constructors: `print`, `readline`, `pure`, `bind` (or `seq`). Full-thread checklist applies. **Done when** `nothing run hello.n` prints text a stranger typed into the editor five minutes earlier.
- [ ] **Effects around holes.** Running a `Cmd` containing a hole executes up to the hole, then stops with the indeterminate report and the captured environment — the run-time twin of Phase 6. **Done when** a program that prints twice with a hole in between performs the first print, skips the second, and reports the hole.

---

## Phase B4 — The standard library

Written in `nothing`, stored in the binary format, shipped inside the binary. The stdlib is also the first real multi-definition dogfood.

- [ ] **Author the stdlib in the editor.** Arithmetic helpers, comparison, list map/filter/fold/length/append via the fold primitive, string helpers, pair utilities. Target: 25–40 definitions. Author them *through the TUI or the protocol* — the commit that adds a stdlib definition must be a serialised document plus its action log, proving it was built with the product. **Done when** the stdlib file loads, every definition is well-typed, and `nothing run` programs can reference it.
- [ ] **Doc metadata.** A doc-string table beside the name table (same id-keyed pattern; docs are metadata, never AST). The editor shows the doc for the completion candidate under the cursor. **Done when** every stdlib definition has a doc line and completion displays it.
- [ ] **`nothing doc`.** Renders the stdlib (or any document) as a static reference: name, type, doc, beginner-projection rendering of the body. **Done when** the generated stdlib reference is committed and legible.
- [ ] **Friction audit #2.** Two hours building a real program against the stdlib, frictions logged, top five fixed, benchmark re-run. Same rules as Phase 4: no fixing during the session. **Done when** the friction file exists with fifteen-plus entries and the five fixes land without benchmark regression.

---

## Phase B5 — Tooling that meets people where they are

- [ ] **Git integration.** `nothing merge-driver` and a diff textconv driver, with a documented `.gitattributes` recipe, so `.n` files diff structurally and merge through the Phase 9 engine *inside ordinary git workflows*. This is the Kowo wedge, productised. **Done when** a scratch repo with two branches editing one `.n` file shows a typed-operation diff in `git log -p` and merges cleanly through `git merge` where raw text would conflict.
- [ ] **Protocol v1 freeze.** PROTOCOL.md versioned; a `version` method; backwards-compatibility test pinning every method's request/response shape. **Done when** the compatibility test would fail on any breaking change.
- [ ] **MCP server.** `nothing mcp` exposes the editor to any MCP-speaking agent host: tools for state, hole-context, apply, script, save/load. **Done when** a Claude Code session with the server configured builds and saves a working program in one conversation, transcript committed under bench/.
- [ ] **Re-run the agent benchmark at post-B2 scale.** Same two conditions as Phase 10, ≥30 tasks, but on programs using strings, lists, records, and match — the scale where text plausibly starts failing. Report whatever the numbers say; AGENT.md was honest when the baseline won, and that is the only reason the next number will mean anything. **Done when** bench/AGENT.md has a dated second table and a paragraph comparing failure modes, with cursor-drift addressed via the interactive loop.

---

## Phase B6 — Onboarding

- [ ] **The in-editor tutorial.** `nothing tutorial`: a guided document where progress is checked structurally (the tutorial inspects the actual AST, not output text) — build a function, fill a hole, cause and repair a quarantine, rename, run. **Done when** completing it touches every core concept and takes under twenty minutes at a beginner's pace.
- [ ] **Rewrite README.md as a front door.** What it is, a 90-second animated-gif-equivalent session transcript, install, tutorial pointer, the three headline numbers (keystrokes, merge, agent) with links to the bench files. Keep the failure-mode guard section — it is part of the project's character. **Done when** a reader who has never heard of projectional editing can explain the hole concept after the first screen.
- [ ] **The beginner-projection test, actually run.** Show a program in beginner projection to three real people who do not program. Ask each to say what it does. Record verbatim answers in a file. This closes the item spec.md could not. **Done when** three transcripts exist and at least two of three correctly described the program — and if they did not, the projection is revised and re-tested rather than the bar lowered.
- [ ] **Five example programs shipped.** Real, runnable, stdlib-using, each under 30 definitions, each with a one-paragraph description: e.g. a unit converter, a grade calculator, a small state machine driven by input, a text game turn, a decision-table rule set (the Kowo demo). **Done when** all five run via `nothing run` and open cleanly in the editor.

---

## Phase B7 — The release gate

- [ ] **The stranger test.** One person who has never seen the project, a fresh machine, the README, no help. Target: installed, tutorial done, one example modified and re-run, inside 30 minutes. Watch silently; log every stumble as a friction entry. **Done when** the run succeeds inside the timebox, or the stumbles are fixed and a second stranger succeeds.
- [ ] **Cut v0.1.0.** CHANGELOG finalised, tag pushed, binaries published, format version frozen with the migration guarantee stated. **Done when** `nothing --version` on a downloaded binary prints 0.1.0.
- [ ] **Write the v0.1.0 postscript into ESSAY.md or its sequel.** What the full-thread rule actually cost per feature (the B2 numbers from DECISIONS.md), whether the "features get cheap after the calculus" bet paid, and the post-B5 agent benchmark verdict. **Done when** it is written with the same honesty standard as bench/AGENT.md.

---

## Guards

Re-read these when a phase drags.

- **The full-thread guard.** If any feature is half-threaded, nothing new starts. A half-threaded feature discovered later than one phase after its merge is a stop-the-line event.
- **The keystroke guard, tightened.** spec.md allowed 3×; the editor now runs 0.16×–0.51×. New guard: if any reference program — including the new string/list/record programs — exceeds **1.0×** Neovim, stop and fix the grammar before the next feature. The product's premise is that structure is *cheaper* to type than text, not merely tolerable.
- **The honesty guard.** Every number in bench/ comes from an executed run. A benchmark that cannot be re-run by a stranger with one command does not count as a benchmark.
- **The scope guard.** When behind schedule, cut B2's variants/match or B5's MCP server before cutting any test task or guard. Ship smaller, never looser.

---

## Things that will go wrong

- **Match will fight the action grammar.** Auto-generated arms interact with every movement and construction rule; expect the sensibility proptest to find real bugs for days. That is the proptest doing its job; do not weaken it.
- **The stdlib will scream for polymorphism.** The all-`?` types will feel embarrassing around the tenth list function. Hold the v0.1.0 line; write the irritation into DECISIONS.md as evidence for the v0.2 decision instead of redesigning mid-build.
- **Effects will tempt you to break purity in the evaluator.** The moment `print` executes inside the live-values pane, the editor becomes a footgun. `Cmd` is a value; only `run` performs it. Hold.
- **Format migrations will feel like wasted work.** They are the product keeping its first promise. A v0.1.0 that strands its own files has no users to strand twice.
- **The stranger test will hurt.** It finds the assumptions no benchmark can. Budget a week after it, not a day.
