# spec.md

**Working name:** `nothing` (placeholder — rename before Phase 3, the name shows up in serialised files)

A projectional programming language. The AST is the source of truth. There is no parser, no text file, no formatter, no syntax error. Editing is a sequence of typed tree transformations that provably preserve well-typedness. The editor and the language are one artifact.

---

## How to use this document

Every task is written to be executed to completion in one sitting. Each has a **Done when** line — a falsifiable criterion. Do not move to the next task until the criterion is met. Do not batch tasks. Do not skip the test tasks; they are the only thing preventing this project from rotting into a demo.

Phases are ordered by dependency, not by interest. Phase 4 is the fun one. Do not start it early.

**Timebox:** Phases 0–6 are the core and should take roughly six weeks at a real pace. Phases 7+ are open-ended and are where the project becomes a product.

---

## Design commitments

These are decided. Do not relitigate them mid-build; if one turns out wrong, note it in `DECISIONS.md` with evidence and change it deliberately.

- **Implementation language:** Rust. Workspace with separate crates so the core stays usable as a library.
- **Editor surface v1:** terminal TUI via `ratatui`. Fast to iterate, forces keyboard-first design, no layout rabbit holes. Web/GPU surface comes later, in Phase 11, only once the action grammar is settled.
- **Incompleteness is first-class.** Every program is well-typed at every instant, including mid-edit. There is no "broken" state. Holes are values.
- **Names are not identity.** Every binder gets a stable ID. The display name is metadata. Rename is a metadata write.
- **Type discipline:** bidirectional typing (synthesis + analysis). Not Hindley–Milner, not fully annotated. Bidirectional gives the editor the thing it needs most — a *known expected type at the cursor* — and that expected type is what powers completion, agent edits, and error recovery.
- **No stringly-typed intermediate.** If you find yourself writing `format!` to build a program and re-reading it, stop. That is the failure mode this project exists to eliminate.

---

## Phase 0 — Scaffolding and honesty infrastructure

Boring. Do it anyway. The instrumentation task in this phase is the single highest-leverage item in the whole document.

- [x] **Create the Cargo workspace.** Set up a workspace with crates: `core` (AST, types, typing), `action` (edit calculus), `eval` (evaluator), `store` (serialisation), `tui` (editor), `bench` (instrumentation). Each crate compiles and `cargo test` passes with zero tests. **Done when** `cargo build --workspace` and `cargo test --workspace` both exit 0.
- [x] **Write `DECISIONS.md` with the design commitments above copied in verbatim.** Add a dated entry template at the bottom. **Done when** the file exists and each commitment has a one-line rationale you wrote yourself, not copied from here.
- [x] **Set up `proptest` in the `core` crate with one trivial property.** Property-based testing is load-bearing later; get the harness working now so it is not an excuse. **Done when** a property test asserting `x + 0 == x` over arbitrary `i64` runs under `cargo test`.
- [x] **Write the keystroke benchmark harness in `bench`.** It takes a named reference program and a recorded sequence of editor actions, and reports the count. It does not need an editor yet — it counts actions applied programmatically. **Done when** `cargo run -p bench -- list` prints the (currently empty) set of reference programs.
- [x] **Choose and write down five reference programs.** Suggested: (1) factorial, (2) a list map, (3) a two-field record constructor plus accessor, (4) a small state machine with three cases, (5) a function with a nested conditional three levels deep. Write each as ordinary pseudocode in `bench/references.md`. **Done when** all five are written and you have counted, by hand, the keystrokes each takes to type in Neovim. Record those five numbers. They are your baseline forever.
- [x] **Write the failure-mode guard into the README.** State explicitly: if after four weeks the keystroke ratio versus Neovim exceeds 3×, the action grammar is wrong and the next sprint is spent fixing it rather than adding features. **Done when** the README says this in a section you will actually re-read.

---

## Phase 1 — Core calculus

The smallest language that is interesting. Resist adding features here. Every feature added in Phase 1 multiplies work in Phases 2, 4, and 6.

Target surface: numbers, booleans, variables, lambda, application, binary ops, `if`, `let`, pairs. That is all. No strings, no lists, no records, no polymorphism, no recursion (yet).

- [x] **Define the type grammar in `core::ty`.** Types are `Num`, `Bool`, `Arrow(Box<Ty>, Box<Ty>)`, `Prod(Box<Ty>, Box<Ty>)`, and `Hole`. **Done when** the enum exists, derives `Clone + PartialEq + Debug`, and has a `Display` impl rendering `Hole` as `?`.
- [x] **Implement type consistency.** Two types are consistent if they are equal, or either is `Hole`, or they are structurally compatible with consistent components. Note this relation is *reflexive and symmetric but not transitive* — that is correct and intentional, it is gradual typing's consistency relation. **Done when** `is_consistent` passes unit tests including the non-transitivity case (`Num ~ ? ` and `? ~ Bool` but `Num !~ Bool`).
- [x] **Implement the matched arrow / matched product judgments.** `matched_arrow(Hole) = (Hole, Hole)`, `matched_arrow(Arrow(a,b)) = (a,b)`, otherwise fail. Same shape for products. This is what lets you apply a function whose type is not yet known. **Done when** both functions exist with unit tests covering the hole case, the concrete case, and the failure case.
- [x] **Define the expression grammar in `core::exp`.** Variants: `Var(Id)`, `Lam(Id, Ty, Box<Exp>)`, `Ap(Box<Exp>, Box<Exp>)`, `Num(i64)`, `Bool(bool)`, `BinOp(Op, Box<Exp>, Box<Exp>)`, `If(Box<Exp>, Box<Exp>, Box<Exp>)`, `Let(Id, Box<Exp>, Box<Exp>)`, `Pair(Box<Exp>, Box<Exp>)`, `Proj(Side, Box<Exp>)`, `EmptyHole(HoleId)`, `NonEmptyHole(HoleId, Box<Exp>)`. **Done when** the enum compiles and every variant is reachable from a constructor function.
- [x] **Write the distinction between the two hole kinds into a doc comment.** An *empty hole* is a gap where an expression has not been written. A *non-empty hole* wraps an expression that is well-typed on its own but does not fit its context — it is the type-error-shaped hole, and it is what lets a program stay well-formed while containing a mistake. **Done when** the doc comment exists on both variants and you can explain the difference without looking.
- [x] **Implement the typing context.** A persistent map from `Id` to `Ty`. Use `im::HashMap` or roll your own; it must be cheap to clone because you will clone it constantly during traversal. **Done when** `Ctx::extend` returns a new context without mutating the original, verified by test.
- [x] **Implement synthesis: `syn(ctx, exp) -> Option<Ty>`.** Handles the variants whose type can be determined bottom-up: `Var`, `Ap`, `Num`, `Bool`, `BinOp`, `Let`, `Pair`, `Proj`, `EmptyHole` (synthesises `Hole`), `NonEmptyHole` (synthesises `Hole`). **Done when** every listed variant has a case and there is one unit test per variant.
- [x] **Implement analysis: `ana(ctx, exp, ty) -> bool`.** Handles `Lam` and `If` specially (they need the expected type pushed in), and falls back to the subsumption rule: analyse against `τ` succeeds if synthesis yields `τ'` consistent with `τ`. **Done when** analysing `λx. x` against `Num → Num` succeeds, against `Bool` fails, and against `?` succeeds.
- [x] **Write the well-typedness invariant as a single function `is_well_typed(exp) -> bool`.** It runs synthesis in the empty context and returns whether it produced anything. **Done when** the function exists and passes on a hand-built program containing both hole kinds.
- [x] **Build ten hand-written test programs as Rust constructors in `core::examples`.** Include at least two containing empty holes and one containing a non-empty hole. **Done when** all ten are well-typed by `is_well_typed` and the file is referenced from the test module.

---

## Phase 2 — The action calculus

This is the intellectual core of the project. Everything else is engineering around it. Read the Hazelnut paper (Omar et al., POPL 2017) before starting this phase — not for the proofs, for the shape of the action judgment.

The central claim: an edit is a judgment `(cursor, program) --action--> (cursor', program')` that carries well-typedness from left to right. If you get this right, syntax errors and type-errors-as-broken-states stop existing as categories.

- [ ] **Implement the zipper cursor type in `action::zipper`.** A zipper is a focused subexpression plus a path of parent frames sufficient to reconstruct the whole. Do not use indices into a flat arena for v1; use a real zipper — it makes the action rules obvious. **Done when** `zip(unzip(z)) == z` holds as a proptest over arbitrary well-typed programs.
- [ ] **Implement movement actions.** `MoveChild(n)`, `MoveParent`, `MoveNextSibling`, `MovePrevSibling`. **Done when** a proptest confirms movement never changes the underlying program, only the focus.
- [ ] **Implement `Delete`.** Deleting replaces the focused expression with an empty hole. It never removes a node without leaving a gap, because the gap is what preserves well-typedness. **Done when** deleting any subexpression of any example program yields a program that still passes `is_well_typed`.
- [ ] **Implement construction actions, one per syntactic form.** `ConstructNum(i64)`, `ConstructBool(bool)`, `ConstructVar(Id)`, `ConstructLam`, `ConstructAp`, `ConstructBinOp(Op)`, `ConstructIf`, `ConstructLet`, `ConstructPair`, `ConstructProj(Side)`. Each replaces the focused hole (or wraps the focused expression, for the operator-like ones) and leaves the cursor at the first new hole. **Done when** every construction action has a unit test asserting both the resulting shape and the resulting cursor position.
- [ ] **Implement the wrapping rule for `ConstructAp` and `ConstructBinOp` explicitly.** When the cursor is on a non-hole expression `e` and you construct an application, the result is `e ⟨hole⟩` with the cursor in the new hole — not a fresh application discarding `e`. This is the rule that makes typing `1 + 2` feel like typing text. **Done when** the test `construct_binop_wraps_focus` passes and typing the action sequence for `1 + 2` from an empty hole takes exactly three actions.
- [ ] **Implement `Finish`.** When the cursor is on a non-empty hole whose contents now typecheck in context, `Finish` unwraps it. **Done when** a program with a non-empty hole, edited so its contents fit, can be finished and the result is well-typed with no hole.
- [ ] **Implement automatic non-empty-hole insertion during construction.** If a construction would produce a type-inconsistent program, wrap the offending subexpression in a non-empty hole rather than rejecting the action. The user is never told "no"; they are told "not yet". **Done when** constructing `1 + true` succeeds, produces a well-typed program, and the `true` is inside a non-empty hole.
- [ ] **Write the sensibility proptest.** For any well-typed program and any cursor position and any action, either the action fails cleanly (returns `None`) or the resulting program is well-typed. Generate at least 10,000 cases. **Done when** the test passes at 10,000 cases with zero failures and you have not weakened the property to make it pass.
- [ ] **Write the reachability proptest.** For any two well-typed programs `a` and `b`, there exists a finite action sequence taking `a` to `b`. Implement this constructively: write a function that computes the sequence (delete to hole, then construct downward), and assert it works. **Done when** the test passes over 1,000 random pairs.
- [ ] **Write the action log type.** Every applied action is appended to a log with a timestamp and an author ID. This is not optional plumbing; provenance, undo, and structural diff all read from it. **Done when** applying 100 actions produces a log of 100 entries and replaying the log from an empty program reproduces the final program exactly.
- [ ] **Implement undo/redo over the action log.** Undo is not inverse actions; it is truncate-and-replay from a snapshot. Simpler and always correct. **Done when** any sequence of 50 random actions can be undone fully back to the empty program and redone to the same final state.

---

## Phase 3 — Text renderer and the throwaway harness

You need to see programs before you build the editor. This renderer is a *projection*, the first of several, and it is read-only.

- [ ] **Implement a plain-text projection of any expression.** Parenthesise minimally and correctly. Render empty holes as `⦇⦈` and non-empty holes as `⦇e⦈`. **Done when** all ten example programs render legibly and `render(parse_free_example)` matches a hand-written expected string in a snapshot test.
- [ ] **Implement cursor rendering.** The focused subexpression is delimited distinctly. **Done when** moving the cursor through a program produces visibly different output at every position.
- [ ] **Build a REPL harness binary that accepts action names on stdin and prints the rendered program after each.** No TUI, no keybindings, just `construct-lam`, `move-child 0`, etc. **Done when** you can build the factorial reference program by typing action names, and it renders correctly.
- [ ] **Record action sequences for all five reference programs using the harness.** Save them as fixture files. **Done when** all five fixtures exist and replay cleanly.
- [ ] **Run the keystroke benchmark for the first time and write the ratios into `bench/RESULTS.md` with today's date.** These will be terrible — you are counting verbose action names, not keystrokes. That is fine. The number exists now. **Done when** the file has five ratios and a note explaining they are pre-keybinding.

---

## Phase 4 — The keyboard grammar

The hard part. Not the compiler — this. Everything above is tractable computer science; this is design, and it is where the project lives or dies.

Read before starting: Vim's verb-object grammar, Kakoune's object-verb inversion, and Ryan Fleury's Dion Systems talks on structural editing ergonomics.

- [ ] **Write `KEYS.md` before writing any code.** Design the full grammar on paper first: modes (if any), verbs, objects, and the mapping from single keypresses to actions. Constrain yourself to what fits on one screen. If it does not fit on one screen it is too complicated. **Done when** the document specifies a binding for every action in Phase 2 and you have talked yourself out of at least three of them.
- [ ] **Design the literal-entry path specifically.** Typing digits should construct a number and keep accepting digits. Typing an identifier character should start variable/binder entry with live filtering over in-scope names. This path handles 80% of real keystrokes; design it first and design it well. **Done when** `KEYS.md` specifies exactly what happens on every printable character in every context.
- [ ] **Build the minimal `ratatui` shell.** Renders the text projection, handles a quit key, nothing else. **Done when** it opens, renders the factorial example, and exits cleanly without corrupting the terminal.
- [ ] **Wire movement keys.** **Done when** you can navigate every node of every example program using only the keyboard, with the focus visibly updating.
- [ ] **Wire construction keys.** **Done when** you can build all five reference programs in the TUI without touching the REPL harness.
- [ ] **Implement in-scope name completion at variable holes.** The context is already threaded through typing; surface it. Filter by typed prefix, and — critically — rank by type consistency with the expected type at the cursor. This is the payoff of bidirectional typing. **Done when** at a hole expecting `Num → Num`, a function of that type ranks above an unrelated `Bool`.
- [ ] **Re-run the keystroke benchmark and write dated ratios into `RESULTS.md`.** **Done when** the numbers are recorded. If any ratio exceeds 3×, stop and fix the grammar before continuing. This is the guard from Phase 0; honour it.
- [ ] **Use the editor to build something real, for at least two hours, without fixing it.** Note every friction point in a file as you go. Do not fix anything during the session. **Done when** you have a list of at least fifteen friction points.
- [ ] **Fix the top five friction points.** **Done when** all five are resolved and the benchmark has not regressed.

---

## Phase 5 — Names as identity

Small phase, large consequences. Do it before serialisation, because it changes the file format.

- [ ] **Replace `Id` with a UUID-backed opaque type and add a separate name table.** The AST stores IDs; the name table maps ID to display string. **Done when** the AST no longer contains any user-visible string and the projection reads names from the table.
- [ ] **Implement rename as a name-table write.** **Done when** renaming a binder used in forty places is a single operation, produces one action-log entry, and cannot fail.
- [ ] **Allow shadowing and duplicate display names without error.** Two distinct bindings may render identically; the editor disambiguates visually, but the program is unambiguous because identity is the ID. **Done when** a program with two bindings both displayed as `x` typechecks and evaluates correctly.
- [ ] **Add a per-user name overlay.** The name table can be layered, so one user's `xs` is another's `items` in the same program. **Done when** two overlays render the same AST with different names and both round-trip.

---

## Phase 6 — Evaluation with holes

A language you cannot run is a diagram. This phase makes it a language.

- [ ] **Implement a small-step evaluator for the hole-free fragment.** **Done when** all five reference programs evaluate to the expected values.
- [ ] **Add recursion.** Either a `letrec` form or a fixpoint combinator — pick one, write down why in `DECISIONS.md`. **Done when** factorial actually computes.
- [ ] **Implement evaluation *around* holes.** A program with holes does not fail to run; it runs until it needs the hole's value, then produces an *indeterminate* result that records what was blocked and in what environment. **Done when** `1 + ⦇⦈` evaluates to an indeterminate value that reports the missing hole rather than panicking.
- [ ] **Implement hole environment capture.** Each indeterminate result carries the environment at the point of blocking, so the editor can show "here is what was in scope when we got stuck". **Done when** evaluating a hole inside a lambda applied to `5` shows the binding of the parameter to `5`.
- [ ] **Render live values in the editor next to the expressions that produce them.** **Done when** editing an expression updates its displayed value without any explicit run command.

---

## Phase 7 — Persistence

- [ ] **Design the on-disk format and write it up in `FORMAT.md` before implementing.** Content-addressed nodes, a name table, and an action log. Specify version bytes. **Done when** the document is complete enough that someone else could implement a reader.
- [ ] **Implement serialisation and deserialisation.** Binary, not JSON — this is not a text format and pretending otherwise invites people to hand-edit it. Provide a debug JSON export separately. **Done when** every example program round-trips byte-identically.
- [ ] **Implement content addressing for nodes.** Hash each node over its structure and children's hashes, excluding names. Two structurally identical functions with different variable names hash the same. **Done when** that property holds as a test.
- [ ] **Write a fuzz test over serialisation.** Random well-typed programs, serialise, deserialise, compare. **Done when** 10,000 cases pass.

---

## Phase 8 — Incremental evaluation

- [ ] **Add a dependency graph over the AST keyed by node hash.** **Done when** you can query which nodes depend on a given node.
- [ ] **Implement invalidation on edit.** An action dirties its node and everything transitively depending on it, and nothing else. **Done when** editing a leaf in a hundred-node program re-evaluates fewer than ten nodes, verified by a counter.
- [ ] **Cache evaluation results by node hash.** Because names are excluded from the hash, a rename invalidates nothing. **Done when** renaming a variable causes zero re-evaluation, verified by the counter.

---

## Phase 9 — Structural diff and merge

This is where the project starts being a product. Everything here is enabled by the action log and content addressing.

- [ ] **Implement a structural diff between two program versions.** Output is a list of typed operations, not lines. **Done when** a diff of a program against itself-with-one-renamed-variable is a single rename operation.
- [ ] **Implement move detection.** A subtree that appears at a new path with an unchanged hash is a move, not a delete plus insert. **Done when** moving a function to a different position in the file produces a one-operation diff.
- [ ] **Implement three-way merge over operations.** Two edit sets against a common ancestor. **Done when** two branches that edit different fields of the same record merge with zero conflicts.
- [ ] **Implement conflict detection with typed explanations.** A conflict is two operations on overlapping nodes that cannot commute. The report says what and why, in terms of the program. **Done when** two branches changing the same expression to different values produce exactly one conflict with both alternatives shown.
- [ ] **Prove the merge preserves well-typedness, by test.** Any successful merge of two well-typed branches is well-typed. **Done when** a proptest over 5,000 random branch pairs passes.
- [ ] **Write the benchmark that justifies the product.** Generate scenarios that a line-based merge fails on but yours does not: reordering, renaming, reformatting, moving. Count. **Done when** `bench/MERGE.md` has a table with real numbers against `git merge` on equivalent text.

---

## Phase 10 — The agent edit API

The thesis, made operational. This is the piece that makes the language matter rather than just being clever.

- [ ] **Expose the action calculus as a serialisable protocol.** Actions in, program state and cursor out. JSON over stdio to start. **Done when** an external process can drive the editor through a full reference program.
- [ ] **Implement the hole-context query.** Given a cursor at a hole, return the expected type, the in-scope bindings with their types, and the set of constructions that would be well-typed there. **Done when** the query at a `Num`-expecting hole never returns a construction that would produce a non-empty hole.
- [ ] **Write a thin harness that lets a model drive the editor.** The model receives the hole-context query output and emits one action. Loop. **Done when** a model successfully constructs the factorial reference program through the protocol.
- [ ] **Measure the invalid-edit rate against a text baseline.** Same tasks, same model: once via generating text patches, once via the action protocol. Count how many edits produce a program that does not parse or does not typecheck. **Done when** `bench/AGENT.md` has both numbers over at least thirty tasks. This number is the product pitch.
- [ ] **Implement per-node provenance from the action log.** Every node knows which author created it and when. **Done when** you can render a program with model-authored nodes visually distinguished from your own.
- [ ] **Implement a provenance filter in the diff view.** Show only human-authored changes, or only agent-authored ones. **Done when** the filter works on a program with mixed authorship.

---

## Phase 11 — Alternate projections

Only start this once the core editor is something you use willingly.

- [ ] **Refactor rendering into a `Projection` trait.** The text renderer becomes one implementation. **Done when** the TUI is generic over the trait and still works.
- [ ] **Implement a second projection for one specific shape.** Pick a state machine or a decision table. It renders a restricted AST pattern in a genuinely different visual form, and it is *editable*, not just viewable. **Done when** an edit made in the second projection is visible in the text projection and vice versa.
- [ ] **Implement projection auto-selection.** The editor picks a projection based on the shape of the subtree, with manual override. **Done when** a state-machine-shaped function renders as a diagram without being told to.
- [ ] **Implement a beginner projection.** Same AST, verbose keyword-heavy rendering, no operator symbols. **Done when** a program written in the expert projection is legible to someone who has never seen the language, verified by showing it to an actual person.

---

## Phase 12 — Decide what this is

Do not skip this. A project without a decision point becomes a hobby that quietly ends.

- [ ] **Write the results essay.** Everything measured: keystroke ratios over time, merge benchmark table, agent invalid-edit rates, what turned out to be wrong. Publish it on the blog. **Done when** it is live.
- [ ] **Pick one of three paths and write down the choice.** (a) Research artifact — polish the calculus, write it up properly, stop. (b) Merge service — extract Phase 9 as a standalone product with a text-language frontend, since the merge engine is valuable without anyone adopting the language. (c) Agent surface — extract Phase 10 as an SDK and integrate it into Beckett. **Done when** `DECISIONS.md` has the choice, the reasoning, and the date.

---

## Prior art to read, one evening each

- **Hazel / Hazelnut** (Omar et al.) — the action calculus. Read this before Phase 2. Non-optional.
- **Unison** — content-addressed code, names as metadata. Read before Phase 5.
- **Lamdu** — the closest anyone has come to a pleasant structural editor. Steal the ergonomics.
- **JetBrains MPS** — projectional editing at industrial scale, and a catalogue of what makes it feel bad. Learn the failure modes.
- **Dion Systems** talks (Ryan Fleury) — the argument for structured editing from a performance and tooling angle.
- **Smalltalk / Self** environments — the live-values idea, done in the seventies, better than most modern attempts.

---

## Things that will go wrong

Written now so you recognise them later rather than interpreting them as evidence the project is bad.

- **Week two, the editor feels awful.** Expected. The grammar is wrong on the first try for everyone. The benchmark tells you how wrong; fix it rather than adding features.
- **You will want to add records, lists, strings, and polymorphism during Phase 1.** Do not. Each one multiplies Phase 2 and Phase 4 work. Add them after Phase 6, one at a time, and notice how much cheaper they are once the calculus is solid.
- **The zipper will feel clumsy around Phase 8.** That is when to consider an arena with parent pointers. Not before — the zipper is what makes the action rules clear while you are still discovering them.
- **Non-empty holes will feel like cheating.** They are not; they are the mechanism that makes "always well-typed" survivable in practice. Trust them.
- **Phase 10 is the temptation to skip to.** It is the most interesting and it is worthless without Phases 1–6. The measurement only means something if the calculus is actually sound.
