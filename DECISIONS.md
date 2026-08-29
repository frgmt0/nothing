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

### 2026-08-28 — Definition references are `Var`, not a new `DefRef`

Phase B1 turns a program from one expression into a document of named
top-level definitions, and a body has to be able to name another
definition. The spec left the mechanism open: "references to definitions
are by id, like variables" — either a new `Exp::DefRef(Id)` variant, or
reuse `Exp::Var(Id)` with definitions entering the typing context.

Decision: **reuse `Var`.** The top level is a mutually recursive binding
group; its members are variables. The context a body is checked in is
`{id ↦ annotation}` for every definition in the document, extended by the
body's own local binders as the zipper descends. A definition's
annotation is what its callers see, which is precisely the property that
makes mutual recursion typeable in one pass: check every body against the
same context of all the annotations, and the fact that `even` mentions
`odd` before `odd` is defined is not a special case, it is just a
variable.

What reuse buys, concretely:

- No new node tag, so `FORMAT.md`'s node table and every reader keep
  their shape (the format still had to go to v2 for the document
  container, but §5's tag table is unchanged).
- No new zipper frame and no arity change, so the sensibility proptest
  keeps its meaning rather than acquiring a new leaf case.
- No new construction action. `ConstructVar(id)` already exists, already
  refuses ids that are not in scope, and already checks the referent's
  type against the expected type — so `ConstructVar(some_definition)`
  works the day definitions enter `ctx_at`, and every downstream consumer
  (completion ranking in the TUI, `candidate_actions` in the hole-context
  query, the script's `construct-var`) inherits definition references for
  free with the guarantee they already had.
- Content addressing already does the right thing. `FORMAT.md` §6
  canonicalises a *bound* `Var` to its de Bruijn depth but writes a
  *free* `Var`'s 16 literal bytes, on the reasoning that for a free
  variable the `Id` *is* the meaning. A definition reference is a free
  variable of the body, so `main` calling `helper` hashes to the same
  digest in any document where `helper` has that id — which is what makes
  the incremental engine's dependency tracking and the merge engine's
  move detection work across definitions without new machinery.
- Shadowing has an obvious rule with no new syntax: a local binder with
  the same id shadows, exactly as nested lambdas already do. Ids are
  uuids, so this is a theoretical case, not an ergonomic one.

What it costs: a base context has to be threaded through the places that
previously assumed the empty one — `is_well_typed` gained
`is_well_typed_in(ctx, exp)`, `Zipper::ctx` gained `ctx_in`, and
`ctx_and_expected_ty_at` gained `..._in`. That is one afternoon of
plumbing paid once.

Rejected, `Exp::DefRef(Id)`: the argument for it is that it makes a
reference to a *definition* syntactically distinguishable from a
reference to a *binder*, so a renderer or a linter never has to consult
the document to know which it is looking at. That is real but small, and
it is buyable at any time by asking the document. Against it: a new node
tag in the format, a new zipper leaf, a new construction action with its
own sensibility obligations, a duplicate scope-checking path in `syn`,
and — the deciding one — two ways to say the same thing in a calculus
whose whole claim is that the surface is small. `Var` is the mechanism
the language already has for "this name means that binding". Definitions
are bindings.

The one consequence to be honest about: **a dangling reference is
ill-typed, not a hole.** `Var(id)` where `id` is neither a definition nor
a local binder does not synthesise, so the document does not typecheck.
That is a deliberate choice and the next entry is about living with it.

### 2026-08-28 — Deleting a definition rewrites its references to empty holes

If definitions are variables, deleting one can strand references to it.
Two ways out: make a dangling reference synthesise `Ty::Hole` (so
"nothing knows what this is" degrades gracefully, the way an empty hole
does), or rewrite every reference at delete time.

Decision: **`DeleteDefinition` rewrites every `Var` mentioning the
deleted id, in every remaining definition, into a fresh `EmptyHole`, as
part of that single action's effect.** One action, one log entry, one
undo step; the log stores no rewrite list because replay rebuilds it
deterministically from the document the log has produced so far.

The empty hole is the only well-typed landing place. A `NonEmptyHole`
cannot wrap the reference — `syn` on `NonEmptyHole(h, e)` requires `e` to
synthesise, and a `Var` with no binding synthesises nothing, so
quarantining a dangling reference would produce an ill-typed quarantine.
An empty hole synthesises `Ty::Hole`, is consistent with whatever the
position expected, and reads to the user as exactly what happened: the
thing that was here is gone, and the shape of the program is still
intact, with a hole where the call was.

Rejected, dangling refs synthesise `Hole`: it is one line in `syn` and it
never fails, which is its whole appeal. But it severs the invariant that
does the most work in this system — **well-typed implies every reference
resolves**. Once a dangling `Var` typechecks, "the program is well-typed"
stops meaning "the program means something": the editor could hand the
evaluator a term with a free variable and no binding, and the only place
that could report it is the evaluator, at run time, as a stuck term with
nothing to blame. That is the class of failure the whole project exists
to make unrepresentable. The type system should refuse to call a broken
reference fine.

Two smaller decisions fall out of the same reasoning:

- **The last definition cannot be deleted.** `DeleteDefinition` on a
  one-definition document fails cleanly (the action returns `None`, the
  editor beeps). A document with zero definitions has no cursor, and the
  cursor is total everywhere else in the calculus; an empty document
  would put an `Option` into every function that reads the focus. The
  gesture the user wants there is "clear this definition", which is
  `Delete` on the body — already available and already one keystroke.
- **Renaming a definition is `Rename`, the existing action.** A
  definition's display name lives in the name table under the
  definition's id, exactly like a lambda binder's. There is no
  rename-definition action because there is nothing for it to do that
  `Rename(id, name)` does not already do, and adding one would mean two
  log tags that must stay in sync forever.

### 2026-08-28 — The document cursor is one zipper plus a split definition list

The zipper had to extend across definitions. Three shapes were on the
table: a second zipper level (`DocZipper` framing the `Exp` zipper), a
`(document, index, zipper)` triple, or splitting the definition list
around the definition being edited.

Decision: **`EditState` holds `before: Vec<Def>`, the current definition's
`id` and `ann`, the `Zipper` into its body, and `after: Vec<Def>`.** It is
the same shape as the expression zipper one level up — the focus, and the
context around it — so `MoveNextDef` and `MovePrevDef` are the list
equivalents of `MoveNextSibling` and `MovePrevSibling`: pop from one side,
zip the current body back into a `Def`, push it to the other. `doc()`
reassembles the whole document; `scope()` produces the `DefScope` (the
context of every definition's annotation, plus the current definition's
expected type) that every action is checked against.

Rejected, a second zipper level: it would have made the document a
recursive structure it is not — a document is a flat list, and framing a
flat list is ceremony. Rejected, `(document, index, zipper)`: the index
and the zipper can disagree, and every action would have to re-derive the
current definition from the document by index before touching it. With
the split list there is exactly one current definition and it is not
addressable by a number that could be stale.

Two things fell out of the shape rather than being decided separately.
`all_document_positions` is the document-era `all_positions`: every cursor
of every definition, which is what the sensibility proptest now quantifies
over. And typing is threaded by *base context* rather than by a new
judgement — `is_well_typed_in`, `Zipper::ctx_in`,
`ctx_and_expected_ty_at_in` all take the context to start from, so the
single-expression functions are the same functions applied at
`Ctx::empty()`, and no typing rule was duplicated for definitions.

### 2026-08-29 — Equality compares at one base type, and `?` is not one of them

Phase B2 added `Str`, which forced a decision that Phase 1 had been able to
duck: `Op::Eq` typed `Num × Num → Bool` because `Num` was the only thing
worth comparing. With `Bool` and `Str` in the language that rule is
arbitrary — `"a" == "b"` is the most ordinary thing a beginner will write.

Three shapes were on the table.

1. **`Eq : ? × ? → Bool`.** One line, and unsound in the way that matters
   here: `?` is consistent with everything, so `1 == true` would typecheck
   and the auto-quarantine would never fire. The whole point of this
   editor is that a nonsense comparison lands visibly in `⦇⦈` instead of
   being waved through, and this rule waves everything through.
2. **A per-operand type variable** (`Eq : α × α → Bool`). Correct, and it
   is the first thing in this language that would need unification. There
   are no type variables anywhere in `core::ty`; adding one for a single
   operator would put an inference engine underneath a calculus that has
   deliberately been kept to consistency and matched judgements.
3. **Compare at a single *base* type, chosen from the operands.**

Decision: **(3).** `typing::is_comparable` admits `Num`, `Bool`, `Str` and
`Hole`. `operand_ty(Eq, l, r)` synthesises both sides, requires both to be
comparable, and takes τ to be the first of the two that is not `?`; both
operands are then analysed against τ, so consistency does the rest. An
operand that does not synthesise, or synthesises a function or a product,
makes the comparison ill-typed and the editor quarantines it. `⦇⦈ == ⦇⦈`
is well-typed at τ = `?`, which is what an editor that lets you write the
operator before either side requires.

Concretely: `1 == 2`, `true == false`, `"a" == "b"` and `⦇⦈ == 1` are
well-typed; `1 == true`, `f == g` and `(1,2) == (1,2)` are not. Nothing
that was well-typed under the old `Num`-only rule became ill-typed — the
new rule is strictly more permissive — so this could not break an existing
document, only accept more of them.

The cost is that `Eq` is the only operator whose operand type depends on
its sibling, and that shows up in exactly three places, all of which now
take the operator: `typing::operand_expectation` (what the editor expects
at a hole next to an `==`), `act::lhs_of_a_binop_fits` (whether
`ConstructBinOp` wraps the focus or quarantines it first), and
`repair::ensure_comparable` (what a merge does when it joins two branches
into a comparison that cannot be made). One test changed behaviour and was
adapted rather than deleted:
`keys::operators_climb_so_left_to_right_typing_means_what_it_says` typed
`1<2=3` and expected `⦇1 < 2⦈ == 3`, because `1 < 2 : Bool` was not
comparable; it is now, so the same keystrokes give `1 < 2 == ⦇3⦈` — the
quarantine moved from the left operand to the right, which is the correct
answer and a better one.

`Concat` needed no such thought: `Str × Str → Str`, unconditionally.

### 2026-08-29 — What the first Phase B2 feature actually cost

`spec-build.md` predicts that features get cheap once the thread is wired
end to end. Strings are the first test of that prediction, so here is the
bill, recorded whether or not it flatters the prediction.

**Files touched: 50 modified, 5 added.** By layer:

| layer | files | what changed |
|---|---:|---|
| type grammar | 1 | `Ty::Str`, one consistency row, `Display` |
| expression grammar | 1 | `Exp::Str(String)`, `Exp::str_`, one reachability count |
| typing | 1 | `syn` for `Str`, `Concat`, and the `Eq` rule above |
| zipper | 1 | **no new frames** — a `Str` is a leaf, like `Num` |
| actions | 4 | `ConstructStr`, op-aware wrapping, generators, script |
| rendering | 1 | `escape_str`/`quote_str`, one precedence row |
| serialisation | 8 (+2 new) | v2→v3, `store/src/v2.rs`, 15 committed v2 fixtures |
| evaluation | 3 | `Dyn::Str`, `Value::Str`, the op split |
| diff/merge | 4 | one `structurally_equal` row, `ensure_comparable`, a scenario |
| hole context | 1 | one candidate, one template |
| provenance | 1 | one `shallow_key` row |
| TUI | 6 | the string run, the status line, the beginner voice, the matrix |
| benchmark | 3 (+3 new) | the sixth reference program |
| docs | 4 | `KEYS.md`, `FORMAT.md`, `bench/references.md`, this file |

**Tests: 38 added**, 6 honestly adapted, 0 deleted. The count was 710
before and is 748 after. Added, by crate: core 9, eval 3, store 5, merge 4,
agentapi 5, tui 11, cli 1. The proptest generators grew a `Str` arm and the
sensibility suite went from 26 to 27 action variants; it still passes at
10 000 cases.

**Adapted, each for a real reason:** four tests asserted that the *string*
`"Str"` was not a parseable type (in `script.rs`, `encode.rs`,
`text_parse.rs`) — it is one now, so they assert on `Text` instead and each
gained string cases of its own. Two asserted the format's version number as
a literal `2` and now read `VERSION_MAJOR`. One —
`operators_climb_so_left_to_right_typing_means_what_it_says` — changed
answer, for the reason given in the entry above.

**What was free.** The zipper (no new frame: a leaf needs none). Evaluation
around holes (the `Str` arms are three lines and the indeterminate cases
were already generic). The CLI (`run` and `check` needed *nothing* — the
first feature in this project to reach the command line without a line of
code). Auto-quarantine, completion ranking, undo, the projections other
than the beginner one, incremental caching, content addressing.

**What fought back, in order of how much:**

1. **The keyboard.** A string is the first thing in this language that is
   *delimited* — every other literal ends when a non-matching key is
   pressed, and a string cannot, because every printable key belongs to it.
   That is a run with an explicit close, and the escape character needs one
   keystroke of lookahead (`\` arms; the next key is taken literally),
   which is the single non-committing keystroke in an editor whose whole
   invariant is that keystrokes commit. It is documented as such in
   `KEYS.md` rather than hidden. Designing this on paper first, before any
   code, was the right order and would have been the right order even if
   the task had not required it.
2. **`Eq`.** See the entry above. The only place where a new *value* type
   forced a change to an existing *rule* rather than an addition to it.
3. **The format.** v2→v3 is four new tags and no shape change, so the
   reader is the same reader; the work was generating the v2 fixture set
   from the genuinely-unmodified v2 encoder (`git stash`, generate, pop)
   *before* touching the encoder, so the v3 reader is tested against real
   v2 bytes instead of bytes produced by the code under test.
4. **The 80×12 status line.** Adding `"` and `&` to the key hint line
   wrapped it and broke an unrelated layout test. Cosmetic, but it is the
   kind of thing that only shows up because the tests are pixel-honest.

**Verdict on the prediction: mostly held.** Nine of the fourteen layers
were a row in a table or a match arm. The two that were not — the keyboard
and `Eq` — were not expensive because the architecture resisted them; they
were expensive because a delimited literal and a polymorphic comparison are
genuinely new *design* questions that no amount of prior wiring answers for
you. The next value type (a character, a byte string) would be nearly free.
A type with *structure* would not be, and nothing here is evidence that it
would.
