# DECISIONS.md

This file records deliberate design commitments and the deviations from them.
Do not relitigate a commitment mid-build; if one turns out wrong, add a dated
entry below with evidence and change it on purpose.

## Design commitments

- **Implementation language:** Rust. Workspace with separate crates so the
  core stays usable as a library.

  Rationale: keeping `core` a plain library with no editor or IO dependency
  is what lets it be embedded in a REPL harness, a TUI, a serialisation
  format, and eventually an agent protocol without any of those pulling
  `core` toward their own assumptions.

- **Editor surface v1:** terminal TUI via `ratatui`. Fast to iterate, forces
  keyboard-first design, no layout rabbit holes. Web/GPU surface comes
  later, in Phase 11, only once the action grammar is settled.

  Rationale: a terminal UI has no mouse and no pixel layout to fuss over, so
  every interaction has to be expressible as a keystroke from day one —
  which is exactly the constraint the action grammar needs to be judged
  honestly, before a GUI's affordances paper over a bad grammar.

- **Incompleteness is first-class.** Every program is well-typed at every
  instant, including mid-edit. There is no "broken" state. Holes are
  values.

  Rationale: if "the program doesn't typecheck right now" is ever a
  reachable state, every downstream consumer (evaluator, serialiser, diff
  engine) has to defend against it forever; ruling it out at the action
  layer means those consumers get to assume well-typedness as an invariant
  instead of a hope.

- **Names are not identity.** Every binder gets a stable ID. The display
  name is metadata. Rename is a metadata write.

  Rationale: identity-by-ID is what makes rename, multi-user name overlays,
  and content-addressed hashing (which must ignore names) all trivial
  instead of each needing its own bespoke string-matching logic.

- **Type discipline:** bidirectional typing (synthesis + analysis). Not
  Hindley–Milner, not fully annotated. Bidirectional gives the editor the
  thing it needs most — a *known expected type at the cursor* — and that
  expected type is what powers completion, agent edits, and error
  recovery.

  Rationale: an editor that always knows the expected type at the cursor
  can rank completions and reject-or-hole-wrap constructions locally,
  without a whole-program inference pass running after every keystroke.

- **No stringly-typed intermediate.** If you find yourself writing
  `format!` to build a program and re-reading it, stop. That is the
  failure mode this project exists to eliminate.

  Rationale: the entire premise of a projectional editor collapses the
  moment a text intermediate becomes load-bearing anywhere in the pipeline,
  because then that text format silently becomes the real source of truth
  and the AST becomes a derived, second-class view of it.

---

## Dated entries

### 2026-08-26 — `nothing-*` crate naming

`cargo` rejects `core` as a package name because it conflicts with Rust's
own built-in `core` crate (the no-std prelude crate every crate implicitly
depends on). The spec's workspace layout calls for a crate directory named
`core`; that directory name is kept exactly as specified, but the package
name inside `core/Cargo.toml` (and each sibling crate) is prefixed:
`nothing-core`, `nothing-action`, `nothing-eval`, `nothing-store`,
`nothing-tui`, `nothing-bench`. Directory names in the workspace
(`core/`, `action/`, `eval/`, `store/`, `tui/`, `bench/`) match the spec
verbatim; only the `[package] name` fields and inter-crate `path`
dependencies use the `nothing-` prefix. `cargo run -p nothing-bench` (not
`-p bench`) is the invocation going forward.

<!--
### YYYY-MM-DD — <decision title>

<what was decided, what evidence prompted it, and what changes as a result>
-->

### 2026-08-28 — `nothing` is the real name, not a placeholder

The spec's header calls `nothing` a working name to replace before the
name reaches serialised files. Decision (made explicitly, with the
serialisation phase about to start): keep it. The name has become the
identity — no parser, no text file, no syntax error, nothing between you
and the tree — and a rename now would churn every crate name, fixture,
and doc for zero technical gain. Version bytes in the on-disk format will
carry the name `nothing` deliberately.

### 2026-08-28 — Recursion is a fixpoint combinator, not a `letrec` form

Phase 6 offers the choice: "either a `letrec` form or a fixpoint
combinator — pick one, write down why". **A fixpoint combinator, written in
the surface the language already has. No new syntactic form.**

The reason is that the combinator is *already typeable here*, and that is
not true of most languages that face this choice. Self-application is
ill-typed in the simply-typed lambda calculus, which is why textbooks
reach for `letrec`. But this language is gradually typed: `Ty::Hole` is
consistent with everything, and `matched_arrow(?) = (?, ?)`, so at a `?`
annotation `x x` synthesises `?` and typechecks. The whole untyped lambda
calculus is sitting inside the Phase-1 surface, reachable through the
annotation the editor writes by default. The call-by-value fixpoint
combinator

```
λstep:?. (λself:?. step (λarg:?. self self arg)) (λself:?. step (λarg:?. self self arg))
```

synthesises `? -> ?` in the empty context today, with no change to
`core::exp`, `core::typing`, or anything downstream of them. It is built
by `nothing_eval::fixpoint::z_combinator`, and
`eval/tests/references.rs` fills the empty hole the factorial fixture
(`bench/fixtures/factorial.actions`) left where the recursive call belongs,
wraps the result in it, and evaluates `factorial(12) = 479001600`.

What `letrec` would have cost, measured against what is in the tree today:
a variant in `Exp`, a synthesis rule and an analysis rule, a `Frame` and
two `move_child` arms in the zipper, a `ConstructLetrec` action with its
own quarantine behaviour, a rendering case, a REPL script verb, a
keybinding taken out of the seven characters `KEYS.md` still holds in
reserve, and — the part that actually matters — the sensibility proptest
(10,000 cases) and the reachability proptest (1,000 pairs) would both be
re-establishing a property over a grammar one constructor larger, for a
feature that adds no expressive power over what `?` already permits. The
spec's own warning about Phase 1 ("each one multiplies Phase 2 and Phase 4
work") applies with full force to a form added in Phase 6.

The honest costs of choosing the combinator:

- **Ergonomics.** `letrec` is one keystroke; the combinator is roughly
  thirty, and it must be typed at every recursive definition. That is a
  real regression in the thing this project measures. It is not paid by
  the five reference programs (`factorial` is the only recursive one and
  its fixture predates recursion), so `bench/RESULTS.md` is untouched and
  honest. When it starts to hurt, the answer is a *library* — a named
  binding the completion list offers — or a template key, neither of which
  is a new syntactic form and neither of which changes the calculus.
- **Types.** A recursive function built this way has type `?`, not
  `Num -> Num`. Applying it is well-typed for the same reason
  self-application is, which means the editor can offer no expected type
  at its argument. A `letrec` with an annotation would type precisely.
  This is the sharpest argument against the choice, and it is the one to
  revisit — but the fix for it is polymorphism and recursive types, a
  Phase 12-scale decision, not a keyword.
- **Blame.** Gradual typing without casts means a genuine runtime type
  error (`(λf:?. f 1) 2` reduces to `2 1`) becomes a stuck term rather
  than a reported error. The evaluator classifies it as indeterminate
  with no hole to blame — `Outcome::is_stuck` — which is honest but
  uninformative. Casts and blame labels are the real answer and are out of
  scope for this phase.

The evaluator itself needed nothing for recursion. Substitution-based
small steps reduce the combinator's self-application without a special
case; the only guard added is fuel, because with recursion available a
program can now diverge and the editor evaluates on every keystroke
(`nothing_tui::live::LIVE_FUEL`).

### 2026-08-28 — Phase 12 path: build v0.1.0

The spec offered three exits: research artifact, merge service, agent SDK.
Decision: none of them alone — productise the language itself, following
`spec-build.md`, with the merge engine's git driver (spec-build B5) shipped
along the way as the commercial wedge. Reasoning: the merge result (13/16
vs git's 2/16) is the strongest measured evidence and monetises without
language adoption, but extracting it standalone forfeits the projectional
premise that produced it; the agent protocol's honest toy-scale loss says
its bet pays only at a scale the current surface cannot express, which
v0.1.0's data types and definitions are the prerequisite for. Building
v0.1.0 keeps all three exits open and is the only path that generates the
evidence the other two still lack. Revisit at the B7 release gate.
