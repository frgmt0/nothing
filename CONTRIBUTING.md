# Contributing to `nothing`

## The invariants

No parser anywhere in the pipeline. Every program well-typed at every
instant. Every edit an action. If a feature cannot be built without
breaking one of these, the feature does not belong in this project.

## The full-thread rule

A language feature does not exist until it is threaded through *every*
layer of the system. A feature merged half-threaded is this project's
definition of broken. No second feature starts while a first is
half-threaded; a half-threaded feature discovered later than one phase
after its merge is a stop-the-line event.

When adding or changing a language feature, copy the checklist below into
the pull request description and check off each item as it is done —
not as it is planned.

### Full-thread checklist

- [ ] **Type grammar** — the feature's types are defined in `core::ty`.
- [ ] **Consistency** — the gradual-typing consistency relation handles the
      new types, including their interaction with `Hole`.
- [ ] **Syn/ana** — bidirectional typing (`core::typing::syn` /
      `core::typing::ana`) has a case for every new expression form.
- [ ] **Expression grammar** — the feature's expressions are defined in
      `core::exp`, reachable from constructor functions.
- [ ] **Zipper** — the zipper (`action::zipper`) can focus every new
      expression position, and `zip(unzip(z)) == z` still holds.
- [ ] **Actions + sensibility re-established at 10k** — every new
      construction/edit action exists, and the sensibility proptest (any
      well-typed program plus any action either fails cleanly or stays
      well-typed) passes at 10,000+ cases over the enlarged grammar,
      unweakened.
- [ ] **Rendering** — the text projection (and any other active
      projection) renders the new forms legibly, including their hole
      states.
- [ ] **Keys (`KEYS.md` updated)** — the keyboard grammar documents a
      binding for every new action, designed before the code, and still
      fits on one screen.
- [ ] **Serialisation (`FORMAT.md` updated, version bumped)** — the binary
      format encodes and decodes the new forms, `FORMAT.md` documents the
      change, the version number is bumped, and a migrating reader for the
      previous version exists.
- [ ] **Content addressing** — new node kinds hash over their structure and
      children's hashes, excluding names, with the existing
      structural-equality property intact.
- [ ] **Evaluation, including around holes** — the evaluator handles the
      new forms, and a hole inside a new construct still blocks evaluation
      cleanly with a captured environment rather than panicking.
- [ ] **Incremental caching** — the dependency graph and cache correctly
      key on and invalidate the new node kinds; an edit re-evaluates only
      what it dirties.
- [ ] **Diff/merge** — structural diff, move detection, and three-way merge
      handle the new forms, and the merge-preserves-well-typedness proptest
      still passes.
- [ ] **Hole-context query** — the query returns correct expected types,
      bindings, and constructions at holes involving the new forms, never
      offering a construction that would produce a non-empty hole.
- [ ] **Provenance** — nodes of the new kind carry author and timestamp
      through the action log like every other node, and the provenance
      filter still works with them present.

## Style

- No code comments in `.rs` files, with the two spec-mandated doc comments
  on `Exp::EmptyHole` / `Exp::NonEmptyHole` in `core/src/exp.rs` as the sole
  exception. Prefer names and structure that make comments unnecessary.
- `cargo fmt` is authoritative; run it before committing.
- `cargo clippy --workspace -- -D warnings` must be clean. Do not add
  `#[allow]` to silence a real finding — fix the code. An `#[allow]` is
  acceptable only when the lint is genuinely wrong for the code in
  question, and the pull request says why.
- `cargo test --workspace` must stay green. Do not weaken an existing
  property test to make it pass; fix the code it caught a bug in.

## Before you open a pull request

Run, and make sure all four exit 0:

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
```
