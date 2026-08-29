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

### 2026-08-29 — Fold is a primitive, not a builtin definition

Lists need one eliminator. The two candidates were a node in the grammar,
`Exp::Fold(list, init, step)`, and a *builtin definition* — a name in every
document's scope that the evaluator knows how to apply, so that `fold` is
looked up the way `map` will be in B4.

**Chosen: the primitive.** The builtin loses on three counts, each of which
would have cost more than the node did.

1. **It cannot be typed.** A builtin is a `Var` and a `Var` gets one type
   from the context. `fold`'s type is `List α -> β -> (α -> β -> β) -> β`
   and this language has no type variables — `spec-build.md` defers
   polymorphism past v0.1.0 on purpose. The only monotype that admits every
   use is all-`?`, which means `fold xs nil f` synthesises `?`, the seed's
   type never reaches the accumulator, the step function's holes expect
   nothing, and the one thing this project is *for* — a hole knowing what
   belongs in it — stops working exactly where the new feature is. The
   primitive gets a real bidirectional rule instead (below), and at
   `fold xs 0 ⦇⦈` the hole says `Num -> Num -> Num` without anyone
   declaring it.
2. **It needs machinery nothing else needs.** A three-argument builtin is
   applied one argument at a time, so both evaluators would need partial
   application of a thing that is not a `Lam`: a builtin-closure value in
   `Dyn`, a matching one in `incr::Value`, arity tracking, and a rule for
   what `fold xs` *is* when nobody applies it further. The primitive is a
   3-ary node, which is the shape `If` already has, so every frame,
   traversal and diff arm was a copy of an existing one.
3. **It is not actually cheaper at the keyboard.** `fold` as a name is a
   completion candidate, three characters and a `space` for each argument;
   `fold` as a form is one key (`/`) that lays out all three holes.

**The rule, written for gradual types.** Synthesis:

```
syn(fold e_l e_i e_s) = β   when  syn(e_l) ▷List α
                              and syn(e_i) = β
                              and ana(e_s : α -> β -> β)
```

and analysis against an expected `β` skips the seed's synthesis entirely:

```
ana(fold e_l e_i e_s ⇐ β)  when  syn(e_l) ▷List α
                            and  ana(e_i ⇐ β)
                            and  ana(e_s ⇐ α -> β -> β)
```

`▷List` is `matched_list`, the third member of the `matched_arrow` /
`matched_prod` family: `List τ ▷List τ` and `? ▷List ?`, everything else
fails. So a fold over a hole is still well-typed with `α = ?`, and the seed
still fixes the accumulator; only the element type goes unknown, which is
the correct amount of ignorance.

**The step is `α -> β -> β`, element before accumulator, and it is a right
fold.** Both halves of that were chosen for what B4's standard library will
have to write:

```
map f xs    = fold xs nil (λx. λacc. f x :: acc)
filter p xs = fold xs nil (λx. λacc. if p x then x :: acc else acc)
```

A *left* fold would produce these reversed and the library would need
`reverse` — which is itself a fold — before it could define `map`. And
element-first matches the order the arguments appear in the expression the
user is building (`x` is the thing they just took out of the list), so the
lambda reads in the order it was typed. `foldr`'s associativity is also the
one that composes with the cons chain the evaluator already walks: the
small-step rule is one line, `fold (h :: t) i s ↦ s h (fold t i s)`, and it
is why `fold` over a list with a hole in its tail computes every element it
*can* reach before blocking (see `step.rs`,
`a_fold_runs_until_it_reaches_the_hole_in_the_list`).

### 2026-08-29 — A cons chain settles on one element type, and quarantines the rest

`Cons` is the first two-child form whose children's types are *not*
independent: `Pair`'s components can be anything, but `1 :: true :: nil`
must not typecheck. The question is where the element type comes from when
the two halves disagree, and the answer had to hold in both directions
because a projectional editor builds these left to right *and* fills holes
inside them.

**Synthesis takes the join.** `syn(h :: t)` computes `syn(t) ▷List α`, then
`syn(h) = β`, then `join(β, α)` — the same greatest-lower-bound the `if`
rule uses to reconcile two branches — and re-analyses the *tail* against
`List (join)`. So `1 :: nil` is `List Num` (the `?` from `nil` gives way to
the number), `⦇⦈ :: nil` is `List ?` (nothing has decided yet), and
`1 :: 2 :: ⦇⦈` is `List Num` (the hole in the tail is `List Num`, which is
the payoff: type `1`, `::`, and the editor already knows what the rest of
the list is).

**Analysis pushes the expected element down and re-joins.** `ana(h :: t ⇐ τ)`
takes `τ ▷List α`, joins `α` with `syn(h)` and analyses both halves against
the refined type. That second join is what makes the *editor* behave: at a
hole expecting `List Num`, `⦇⦈ :: ⦇⦈` still fits, and the head hole then
expects `Num` rather than `?`.

**Disagreement quarantines rather than refusing.** `join(Bool, Num)` fails,
so `true :: 1 :: nil` is not well-typed, and the editor's `place` wraps
whatever does not fit: typing `t` `:` `1` `:` `n` gives
`true :: ⦇1 :: nil⦈`, and no `Enter` will ever finish it, because a list of
numbers is not a list of booleans. **The element type is fixed by the first
element written, not by a declaration** — which is the same discipline the
rest of the language uses (a `let`'s binder takes its bound expression's
type, an `if`'s type is the join of its branches) and needs no new concept.

One consequence worth stating out loud: typing a literal list left to right
at a hole that already expects a list produces *nested* quarantines, one
per element, because each element lands where a list was wanted. Every one
of them finishes with a single `Enter` once the cell around it is written,
and this is not new behaviour — it is exactly what typing `1` at a
`Num * Num` hole has always done. It is the price of "the editor never
refuses a keystroke", paid in a place where it is more visible than usual.

### 2026-08-29 — No bracket sugar: a list projects as its cons cells

`[1, 2, 3]` is how every other language writes this, and it is not how
`nothing` renders it. The projection is `1 :: 2 :: 3 :: nil`.

The reason is a property, not a preference. `action/src/cursor_render.rs`
builds the cursor projection **frame by frame** from the zipper, and
`stripping_markers_reproduces_the_plain_projection` asserts that deleting
the two cursor markers from that string yields exactly what `core::render`
produces for the whole program. That property is what lets the TUI, the
agent protocol and the beginner voice share one renderer and one set of
precedences; it is checked on 1 000 generated programs.

Bracket sugar cannot satisfy it. A cursor on the second cell of `[1, 2, 3]`
addresses the subtree `2 :: 3 :: nil`, and there is no substring of
`[1, 2, 3]` that is the rendering of that subtree — the sugar is a
*whole-list* decision, and the zipper hands the renderer one frame at a
time with no way to know whether the cell above it is the head of a
literal chain or the tail of something else. Making it work would mean
either a second renderer that the marker-strip property does not cover, or
teaching every frame about its ancestors, which is the thing the frame
representation exists to avoid.

Given that, `1 :: 2 :: 3 :: nil` is the better of the two available
answers rather than a consolation prize: every cons cell is addressable and
visibly so, which is the whole reason a structure editor renders anything.
The sugar is a *display* question and can come back as a projection in the
`C-p` family (`tui/src/projection.rs`), where a projection is allowed to be
a different surface with its own key handling — the state-machine table
already is one. Two projections did get a list voice for free: the beginner
one reads "1 in front of 2 in front of an empty list", and the state
machine one was untouched because it matches on `if`-chains.

### 2026-08-29 — A cons cell is a spine link, and the merge engine now says so

Two branches that append different elements to *different places in the
same list* commute, and the first implementation conflicted them. Both
edits are an `Insert` at a path, one at `[1]` and one at `[1,1,1]`, and
`regions_overlap` treats two `Region::Node` paths as overlapping whenever
one is a prefix of the other — which is right for a replace inside a
rewritten subtree, and wrong here, because inserting a link into a chain
does not disturb the chain below it.

Two changes, both small:

1. **`merge/src/chain.rs` was generalised, and it did fit.** Its LCS was
   over binder `Id`s; it is now `common_subsequence_indices<T: PartialEq>`
   returning index pairs, with the old `longest_common_subsequence` written
   in terms of it. `diff_spine` in `merge/src/diff.rs` flattens a cons chain
   the way `chain_of` flattens a `let` chain and aligns the two element
   lists by **content hash**. What did *not* transfer is the identity part:
   a `let` chain's bindings have ids, so a reordering is detectable and gets
   a `MoveBinding`; cons cells have no identity at all, so an element that
   moves is a delete and an insert, and there is no list analogue of
   `Region::Order`. Appending is one `Insert`, splicing is one `Insert`,
   dropping an element is one `Delete`, and an edit *inside* an element is
   an ordinary recursive diff at that element's path.
2. **`Region::Cell`.** An `Insert` or `Delete` whose node is a `Cons` and
   whose slot is the tail is a spine-link edit. Two of them overlap only at
   the *same* link; a `Node` or `Shape` region overlaps one when it contains
   it (so replacing the whole list still conflicts with splicing into it);
   an edit to an element does not overlap the links around it. The existing
   deepest-path-first application order does the rest, and
   `1 :: 2 :: 3 :: nil` grown at the end by one branch and in the middle by
   the other merges to `1 :: 9 :: 2 :: 3 :: 4 :: nil` with no conflict.

### 2026-08-29 — What the first *structured* type cost

Strings were the first Phase B2 feature and the entry above records their
bill, ending on a prediction: "the next value type would be nearly free. A
type with *structure* would not be, and nothing here is evidence that it
would." Lists are that type. Here is the bill.

**Files touched: 56 modified, 17 added** (`store/src/v3.rs` and the sixteen
committed `store/fixtures/v3/` artifacts). By layer:

| layer | files | what changed |
|---|---:|---|
| type grammar | 1 | `Ty::List`, `matched_list`, one consistency row, one `Display` arm, one `join` row |
| expression grammar | 1 | `Exp::Nil`, `Exp::Cons`, `Exp::Fold`, an `Exp::list` helper |
| typing | 1 | `step_ty` and four rules — `syn`/`ana` for `Cons` and `Fold` |
| zipper | 1 | **five new frames**, where strings needed none |
| actions | 4 | three constructions, three expected-type frame rules, generators, script |
| rendering | 2 | `PREC_CONS` inserted mid-table, so every precedence above it moved |
| serialisation | 8 (+1 new) | v3→v4, `store/src/v3.rs`, 16 committed v3 fixtures |
| evaluation | 3 | `Dyn`/`Value` cases, the two fold rules, an iterative spine walk |
| diff/merge | 5 | `diff_spine`, `Region::Cell`, a generic LCS, a scenario |
| hole context | 1 | three candidates, no templates |
| provenance | 2 | three `shallow_key` rows, three JSON tags |
| TUI | 6 | `:`/`/`/`[`, the `nil` candidate, the beginner voice, the matrix |
| benchmark | 4 | reference 2 rebuilt as a real list map |
| docs | 5 | `KEYS.md`, `FORMAT.md`, `bench/references.md`, `bench/RESULTS.md`, this file |

**Tests: 27 added**, 10 honestly adapted, 0 deleted. The proptest generators
grew a `List` type arm and a `Fold` form, the action alphabet went from 27
variants to 30, and sensibility still passes at 10 000 cases over the
enlarged grammar.

**Adapted, each for a real reason.** Three were counts that are now bigger
and are asserted rather than assumed: the reachability form survey (13 → 16
`Exp` variants), the action-variant table (27 → 30), and the fixture
inventory. Two completion tests changed because `nil` is a new candidate
and therefore appears in the ranked lists they assert on — in both cases
after everything that fits, which is the ranking working. One matrix column
changed because `n` at a hole now commits `nil` instead of leaving the hole
alone. Three key-line and layout assertions moved because the status line
gained two characters (see below). One evaluator reference test —
`reference_two_list_map_maps` — now applies the doubling function to
`3 :: 4 :: 5 :: nil` and expects `6 :: 8 :: 10 :: nil`, because reference 2
is a real list map now and no longer a map over a pair. None of them
changed because a claim stopped being true.

**What was free.** Auto-quarantine (the `place` fallback needed no list
case). Undo, the action log, content addressing, the incremental cache's
invalidation, the state-machine projection, the agent protocol's transport.
The CLI again needed nothing but a test.

**What fought back, in order of how much:**

1. **The renderer's precedence table.** Cons sits *between* comparison and
   addition, which no existing operator did — every previous addition went
   on an end. Inserting `PREC_CONS = 2` renumbered `PREC_ADD`, `PREC_MUL`,
   `PREC_APP` and `PREC_ATOM`, and those constants are shared by three
   renderers (`core::render`, `action::cursor_render`,
   `agentapi::provenance`) and the TUI's climb rule. Nothing broke, because
   they are constants and not literals — but that is the design paying off,
   not the change being small.
2. **The evaluator's stack.** Adding the `Fold` arm inline to `step_in`
   grew that function's frame enough that a *pre-existing* test —
   `a_definition_that_diverges_runs_out_of_fuel_rather_than_hanging`, which
   builds a 500-deep `+` chain and then walks it — began overflowing the
   stack in debug builds. The fix was to lift the arm into `step_fold`, and
   the lesson is that a small-step interpreter written as one big recursive
   `match` has a frame-size budget that new grammar spends. The incremental
   engine has the same hazard from the other direction: a naive recursive
   fold recurses once per element, so `fold_value` walks the cons spine into
   a `Vec` first and folds right-to-left over that, with salted per-element
   digests so the cache keys stay sound. There is a test that folds a
   1 200-element list to prove it.
3. **The merge engine.** See the `Region::Cell` entry above. This is the
   first feature that needed a new *conflict* concept rather than a new row
   in an existing table, and it is exactly the thing strings did not need.
4. **The zipper.** Five frames. Strings needed zero, because a `Str` is a
   leaf. Every frame is a rebuild arm, a child index, a parent arity, a
   `move_child` arm, a `cursor_render` arm, a beginner-voice arm and an
   expected-type rule: seven places each, thirty-five in total, all
   mechanical and none of them free.
5. **The 80×12 status line, again.** Adding `:` and `/` to the key hint
   line wrapped it onto a third row and broke the definition-pane layout
   test, exactly as `"` and `&` did in the strings entry. Two features in a
   row have paid this; the line is now within four characters of full and
   the next feature should expect to shorten something.

**Verdict on the prediction: it held, and the caveat held too.** Strings
predicted that another *value* type would be nearly free and that a
*structured* type would not be. Lists cost roughly the same number of files
as strings (56 against 50) but the work was differently shaped: strings
were dominated by one hard design question (a delimited literal at the
keyboard) with mechanical consequences, while lists were dominated by
*breadth* — five zipper frames, three constructions, a precedence
insertion, a new conflict region — with the design questions (fold's form,
the element-type join) settled on paper in an afternoon. Nine of the
fourteen layers were still a match arm or a table row. The two places the
architecture genuinely did not absorb the change were the merge engine's
notion of a conflict region and the evaluator's stack budget, and both were
about *sequences* specifically rather than about structure in general. A
record type — the other thing B4 wants — would pay the zipper and renderer
costs again and none of the sequence ones.

---

### 2026-08-29 — A record's fields are identities, and that decides everything else

Records are the third Phase B2 feature. The checkbox asks for "named fields
where field identity is an `Id` and the display name lives in the name
table — exactly like binders — so renaming a field project-wide is one
action that cannot fail and cannot conflict in a merge." Every choice below
falls out of taking that literally.

`Ty::Record(Vec<(Id, Ty)>)` and `Exp::Record(Vec<(Id, Exp)>)` /
`Exp::Field(Box<Exp>, Id)`. Fields are ordered, because the projection has
to print them in *some* order and the order the user typed them in is the
only one that is not a lie. Nothing else depends on the order.

**Consistency is by field set, order-insensitive, and exact.**
`{x: A, y: B}` is consistent with `{y: B', x: A'}` when the two carry the
same set of field ids and each pair is consistent. Two decisions in one
sentence:

- *Order-insensitive* is forced, not chosen. Reordering a record's fields is
  one of the two operations this feature exists to merge cleanly. If
  consistency were order-sensitive, a branch that reordered fields would
  make every use site of that record ill-typed, and the merge would repair
  by quarantining half the program. Order is display; the type is a set.
- *Exact* — no width subtyping. A record with more fields than expected is
  inconsistent. Gradual typing already has one direction of "I know less
  than you do" (`?`), and adding subtyping gives consistency a second
  direction with different algebra; the two interact in ways that are a
  research paper, not a phase. Exactness also keeps `is_consistent`
  symmetric, which an existing proptest asserts and which the whole
  quarantine story leans on. The cost is recorded: you cannot pass
  `{x: Num, y: Num}` where `{x: Num}` is expected, and you will not want to
  until modules arrive.

`join` follows: same id set or nothing, field-wise, and the result keeps the
**left** operand's order, because `join` is called from `If` where the
then-branch is the one the reader met first.

**`matched_record` fails open per field.** `matched_record(?, f) = ?` and
`matched_record({…, f: τ, …}, f) = τ`; anything else fails. So `p.x` where
`p : ?` synthesises `?` and is well-typed, which is the gradual rule every
other `matched_*` judgment already follows, and it is what makes a record
parameter writable at all: `λp:?. p.x` is a function over records, and the
annotation slot never had to learn a new grammar for it. Projecting a field
the record *does* have but at the wrong type is not a special case — the
field's type is whatever the record says.

**A record type is inferred and displayed, never spelled.** There is no `{`
in the annotation slot and no record syntax in `script::parse_ty`. A record
type is a list of field *identities*; an annotation slot is free text
re-parsed on every keystroke of a commit-live run. To accept `{x: Num}`
there the slot would have to either resolve `x` against fields that already
exist — making a *type* depend on program content elsewhere, the thing
§Rejected item 5 of `KEYS.md` refuses — or mint a fresh id per keystroke,
strewing the name table with fields no record has and making the annotation
inconsistent with every construction site by construction. The decisive
argument is smaller and harder: `action_name`/`parse_step` round-trip is a
tested property, `SetAnn` carries a `Ty`, and a `Ty` carrying uuids cannot
round-trip through a name. So `Ty::Record` is a type the checker computes,
`core::render::render_ty` prints with display names, and `Display for Ty`
prints with short ids for logs. If a later phase wants record annotations,
the honest route is a field *picker* in the slot, not a parser.

**Duplicate display names are fine; a duplicate `Id` is not.** Two fields
called `x` in two different records are two fields, exactly as two binders
called `x` are two binders (settled item 13 of `KEYS.md`). The same `Id`
twice in one record is ill-formed: no action can build it (`AddField` mints
a fresh id), and `syn` returns `None` for it anyway, so a corrupted file
fails to typecheck rather than typechecking ambiguously.

**`RemoveField` rewrites every projection of that field to `⦇e⦈`.** This is
B1's delete-definition decision (2026-08-28) with one refinement. B1
replaces a reference to a dropped definition with an empty hole because a
`Var` has nothing worth keeping. A projection `e.f` does: `e`. So the field
goes, the projection becomes a quarantine around the subject, the document
is well-typed before and after, one `C-z` puts it all back, and the user's
work is still on screen wearing the editor's own "this does not fit any
more" marker. This is precise rather than over-eager because a field id
belongs to exactly one record: no action adds an *existing* field id to a
second record, so the projections of `f` are exactly the sites the removal
breaks. Removing the last field leaves `{}`, which is a perfectly good type
— unlike a document, a record has nothing to be the last of.

**`AddField` appends.** Not "insert after the cursor": append is where you
can predict it will land without looking, and `C-←`/`C-→` move it from
there in one keystroke each.

**Field ids are identity-relevant and hash raw.** This is the one place the
"names are excluded from the hash" principle looks like it should apply and
does not. Bound variables are canonicalised de-Bruijn-style because a
binder travels with the term it binds: `λa. a` and `λb. b` are the same
function. A record's field identity does not travel with anything — it is
document-global, like a definition's id and like a *free* variable's, which
`build_rec` already hashes raw. Canonicalising field ids by position would
make `{x = 1}` and `{y = 1}` hash equal, and the merge engine's move
detector matches subtrees by content hash: it would cheerfully report that
one record had *moved* to where a differently-typed record now is, and
splice two incompatible types together without a conflict. So field ids are
hashed as they are, a `Field` node hashes its field id, and what is excluded
is the thing that was always excluded — the display name, which lives in the
name table and never reaches `hash_node`. That exclusion is the whole
feature: renaming a field changes no node hash, so a rename has zero
structural footprint and cannot collide with any structural edit.

**Field order is an `Order`-shaped region of its own.** `Region::FieldOrder(path)`
overlaps an edit *at or above* the record and nothing inside a field's value,
overlaps another `FieldOrder` only at the same path, and never overlaps a
`Name`. So: reorder commutes with rename (different region kinds), reorder
commutes with edits inside field values (strictly-below paths do not
overlap), and two branches reordering the same record's fields is a real
conflict, which it should be. `Operation::ReorderFields { path, from, to }`
permutes whatever fields are at that path when it is applied, by id, so it
composes with the other branch's edits to the values rather than overwriting
them.

Two coarsenesses are recorded rather than hidden. A record whose *field set*
differs between two versions diffs as a whole-node `Replace`, not as an
insert-and-delete over the field list, and a projection whose *field id*
changed does too, exactly as a `Proj` whose side changed already did. Both
could be finer. Neither is worth being finer yet: a changed field set is a
changed type, so the values under it are being re-typed anyway, and the
sites where two branches independently add different fields to the same
record are sites where the type they agree on is the thing in dispute — a
conflict is the honest answer there, not a merge. `ReorderFields` is the
one field-list edit that gets a footprint of its own, because it is the one
that provably changes nothing about the program.

**Keys.** `KEYS.md` settled item 19 has the full argument. In short: `{`
constructs, `.` projects and never climbs, `C-n`/`C-d` generalise from "one
more / one fewer definition" to "one more / one fewer row of the list you
are in", `C-←`/`C-→` reorder, and there is one field slot that renames a
record's field and picks a projection's field, because both of those are
"name a field".

**`ConstructRecord` adopts the expected record's fields.** Every other
construction action writes a fixed shape and lets the checker quarantine it
if it does not fit. A record cannot: its shape *is* a set of identities, and
a freshly minted identity is inconsistent with any record type that is
already known, so a mint-one-field `ConstructRecord` is quarantined in
exactly the positions where the user most obviously wants a record — a
definition annotated with a record type, a field of an enclosing record, the
hole `AddField` just made. So the action reads the expected type first: a
record expectation on an empty hole writes that record's field ids with hole
values and puts the cursor in the first; a record expectation on a filled
focus wraps the focus as the first field's value if it analyses against that
field's type and holes the rest; no record expectation mints one fresh field
and names it. The reachability suite found this, not the design review — the
"any well-typed program reaches any other" property failed because a record
in an annotated position was unreachable, which is the strongest possible
argument that the mint-always version was wrong rather than merely awkward.

---

### 2026-08-29 — What the first *named* type cost

Records are the third and last Phase B2 feature. The lists entry above ends
on a prediction: "A record type — the other thing B4 wants — would pay the
zipper and renderer costs again and none of the sequence ones." Here is the
bill, and the prediction was wrong in both directions.

**Files touched: 62 modified, 19 added** (`store/src/v4.rs`, the seventeen
`store/fixtures/v4/` artifacts, and one proptest regressions file that is
itself part of this entry — see *what fought back*, item 1). By layer:

| layer | files | what changed |
|---|---:|---|
| type grammar | 1 | `Ty::Record`, `matched_record`, `matched_record_fields`, an order-insensitive consistency row, a field-wise `join`, a `Display` arm |
| expression grammar | 1 | `Exp::Record`, `Exp::Field`, `Exp::record`/`Exp::field`, `field_ids` |
| typing | 1 | `syn`/`ana` for both forms, field-wise |
| document | 3 | `Doc::field_ids`, the name table's field names, examples |
| zipper | 1 | **two frames, one of them variable-arity** — `RecordField(others, index, id)` carries its siblings |
| actions | 3 | eight actions, the expected-type rules, the generator, the script vocabulary |
| rendering | 2 | `{x = e}` and `e.x`, both at `PREC_ATOM`, so **the precedence table did not move** |
| serialisation | 8 (+1 new) | v4→v5, node tags 16–17, `Ty` tag 7, action tags 30–37, `store/src/v4.rs`, 17 v4 fixtures |
| evaluation | 3 | `Dyn`/`Value` cases, the projection rule, `step_field`/`step_record` |
| diff/merge | 9 | `ReorderFields`, `Region::FieldOrder`, `diff_record`, `RepairKind::Reidentified`, the scenario, the harness prose |
| hole context | 1 | a record construction and one projection per field in view |
| provenance / encode | 2 | two `shallow_key` rows, two JSON tags, per-field marker paths |
| text baseline | 1 | a full recursive type parser and record syntax for `measure::text_parse` |
| TUI | 6 | `{`, `.`, the two field-slot flavours, `C-n`/`C-d`/`C-←`/`C-→`, the beginner voice, the live line |
| CLI | 1 | an exhaustiveness arm, and a test |
| benchmark | 6 | reference 3 rebuilt as a real record, both fixtures, the merge scenario, `MERGE.md`, `RESULTS.md`, `references.md` |
| docs | 3 | `KEYS.md` item 19, `FORMAT.md` v5, this file |

**Tests: 43 added**, 9 honestly adapted, 0 deleted. The count was 775
before and is 818 after. The action alphabet went
from 30 variants to 38, the `Exp` grammar from 16 to 18, and sensibility
still passes at 10 000 cases over the enlarged grammar and at every cursor
position of a generated *document*.

**Adapted, each for a real reason.** Three were counts asserted rather than
assumed: the reachability form survey (16 → 18 `Exp` variants), the
action-variant table (30 → 38), and the v4 fixture inventory. One key-hint
assertion moved because the hint line gained `{` and `.`; it is still three
rows at width 60. The keystroke matrix gained a `{` row — one probed
expectation in each of its eight columns — and its safety alphabet gained
`{` and `}`; the `.` row already there did not move, because none of the
eight contexts holds a record for `.` to name a field of. Three benchmark
assertions moved with reference 3 — the fixture, its expected rendering, and
its keystroke script — and `reference_three_record_constructs_and_accesses`
in the evaluator now builds `{x = 3, y = 4}` and reads `.x` where it used to
build `(3, 4)` and read `fst`, because reference 3 is a record now. One
migration test lowered a fixture-count floor from 17 to 16, because the v4
fixture generator now *skips* any program a version-4 build could not have
written, and the record fixture is one. None of them changed because a claim
stopped being true.

**What was free.** Undo, redo, the action log's replay, the incremental
cache's invalidation, the content-addressed node table's *framing* (it has
always written a varint child count, so the first variable-arity node needed
no format shape change), the state-machine projection, the agent protocol's
transport, and auto-quarantine — `place`'s fallback needed no record case.
The CLI again needed one match arm and a test.

**What fought back, in order of how much:**

1. **The reachability suite, which found a design bug the design review had
   not.** `any_well_typed_program_reaches_any_other` failed, three seeds
   deep, because `ConstructRecord` minted a fresh field id unconditionally
   and a fresh identity is *never* consistent with a record type that is
   already known — so a record was unreachable in exactly the positions
   where a user most wants one. The fix is recorded in the entry above:
   the action reads the expected type first. This is the first time a
   property test has rejected a *design*, not an implementation, and the
   five saved seeds in `action/tests/reachability.proptest-regressions` are
   the receipt.
2. **The sensibility suite's vacuity check, for a new reason.**
   `every_action_succeeds_somewhere_in_the_search_space` reported that
   `SetField` never applied in 867 566 attempts. Nothing was broken: the
   exhaustive action list was built from constants, and `SetField` carries
   an *identity*, so the two ids it offered belonged to no record any
   generated document contained. Every previous action was enumerable from
   constants because its payload was a number, a bool, an operator or a
   side. This one is not, and `one_of_every_action_in` now takes the
   document's real field ids. The general lesson: an action whose payload is
   an identity cannot be exhaustively enumerated without reading the program
   it will be applied to, and a suite that does not do that passes
   vacuously.
3. **The evaluator's stack budget, again, and for a compounding reason.**
   `a_definition_that_diverges_runs_out_of_fuel_rather_than_hanging` builds
   a 500-deep term and began overflowing in debug builds — the same failure
   the lists entry recorded for `Fold`, with an extra cause: `Ty::Record`'s
   `Vec` made `Ty` 24 bytes larger, which made `Exp` and `Dyn` larger, which
   made every `step_in` frame larger *before* the new arms were added. Same
   fix (`step_field`, `step_record`), but the lists entry's lesson needs
   strengthening: a small-step interpreter written as one recursive `match`
   has a frame budget that new grammar spends **twice** — once for the arm
   and once for the enum growth.
4. **The text baseline could no longer read what the editor writes.**
   `agentapi::measure::text_parse` exists to give the "how many keystrokes
   would this cost in a text editor" comparison a real parser, and it
   round-trips generated programs through `render`. Records broke it
   immediately: the renderer prints `{x = 1}` and `λp:{x: Num}. p.x`, and
   the parser's lambda rule scanned to the first `.` and delegated the
   annotation to `script::parse_ty`, which by design has no record syntax
   and never will (see the entry above). So the measure parser grew a full
   recursive type parser of its own and a name→field-id map, so that the
   `x` in an annotation and the `x` in a projection are one identity and
   the reparsed program is well-typed. This is the cost of the "record types
   are never spelled" decision landing somewhere that *has* to spell them,
   and it is the right place for it: a measurement baseline is allowed a
   syntax the editor does not offer.
5. **One slot, two flavours, and a ranking bug behind it.** `KEYS.md` item
   19(d) was written claiming one field slot "because it names a field in
   both places it is reached". The implementation cannot: a projection can
   itself be the value of a record's field, so the cursor alone does not say
   whether the buffer should rename that field or pick a different one. The
   flavour has to follow how the slot was *entered*, which is two internal
   variants wearing one label, and item 19(d) now says so. The same
   blindness had a sharper consequence one layer down:
   `script::fields_in_view` ranks "the fields of the subject of the
   projection you are on", which is right in the pick slot and wrong at the
   moment `.` is pressed on a projection — there the subject-to-be is the
   focus itself. Uncorrected, `p.x` then `.` quarantined instead of reading
   `p.x.y`, falsifying item 19(b). The TUI's completion layer, which is the
   only layer that knows the slot, now re-ranks accordingly.
6. **The 80×12 status line, a third time — and it did not break.** `{` and
   `.` went onto the key-hint line and it is still three rows at width 60.
   The lists entry warned the line was "within four characters of full and
   the next feature should expect to shorten something". It was two
   characters, and nothing had to be shortened. The warning stands for the
   feature after this one.

**Verdict on the prediction: wrong twice, in opposite directions.** Lists
predicted records would "pay the zipper and renderer costs again and none of
the sequence ones."

- **The renderer cost was not paid at all.** A record is delimited and a
  projection is postfix, so both are `PREC_ATOM` and the precedence table
  did not move by one number. The insertion pain lists suffered was about
  *infix* syntax, not about structure, and nothing in the lists entry
  noticed that distinction.
- **The sequence costs were paid in full.** A record's field list is a
  sequence, so the merge engine needed a new ordering region
  (`Region::FieldOrder`) for precisely the reason the cons spine needed
  `Region::Cell`, `ReorderFields` had to be replayed in the same late phase
  as `MoveBinding`, and the evaluator's stack budget bit again. "None of the
  sequence ones" was wrong because it treated *sequence* as a property of
  the list type rather than of any variable-arity node, which a record is —
  the first one in this language.
- **The zipper cost was paid, and under-estimated.** Two frames against
  lists' five sounds cheap until `RecordField` has to carry its siblings and
  its index, because rebuilding a variable-arity parent needs both. Every
  fixed-arity frame in this codebase is a tuple of the *other* children;
  this one is a `Vec` and a position, and every consumer — rebuild, arity,
  child index, cursor render, beginner voice, expected type — has to do
  arithmetic instead of pattern matching.

The thing the prediction had no word for is the one that mattered most, and
it is the checkbox's own claim: **an identity-shaped payload changes what
"exhaustive" means.** Three of the six things that fought back — the
reachability failure, the sensibility vacuity, and the field-slot ranking —
are all the same fact wearing different clothes: a field id is a *reference*
to something elsewhere in the document, so any code that enumerates,
generates, or ranks must read the document first. Strings and lists never
asked that. Definitions did, in B1, which is why `ConstructVar` was already
handled that way and why the fixes were short once the failures were
understood. The payoff is the one the checkbox asked for and it is real:
`action/tests/names.rs::renaming_a_field_renames_its_construction_site_and_every_projection_across_the_document`
renames a field used in one definition's construction site and five
projections in another, in **one** log entry, with the document tree
byte-identical before and after; and the merge benchmark's new scenario has
one branch renaming a field while the other renames a second field *and*
reorders all three, merging clean and well-typed while `git merge-file`
conflicts on the same three versions' rendered text.

---

### 2026-08-29 — A variant is a set of constructors, and a match is its shape

Variants are the fourth and last Phase B2 feature. The checkbox asks for "sum
types with id-identified constructors, and a match expression. Constructing a
match on a variant type auto-generates one arm per constructor, each a hole —
exhaustiveness by construction, not by warning." Records decided everything
about identity already (2026-08-29, above); this entry decides the things a
*sum* asks that a product does not.

`Ty::Variant(Vec<(Id, Ty)>)`, `Exp::Inj(Id, Box<Exp>)` and
`Exp::Match(Box<Exp>, Vec<(Id, Id, Box<Exp>)>)` — an arm is a constructor id,
a payload binder id, and a body. Constructors are ordered for the same reason
fields are: the projection has to print them in *some* order, and the order
they were written in is the only one that is not a lie.

**Every constructor carries exactly one payload, and a nullary one carries
`{}`.** Four spellings were on the table: `(Id)` with no payload at all,
`(Id, Option<Ty>)`, `(Id, Ty)` with `Ty::Hole` meaning "no payload", and
`(Id, Ty)` with the empty record meaning it. The first two put a second shape
into every match, every rebuild, every hash and every migration, to save
writing two characters. The third is wrong twice over: `?` already means "not
known yet", so a nullary constructor would be indistinguishable from one whose
payload the user has not decided, and `matched_variant` would have to fail open
on a case that is closed. The fourth costs nothing, because records arrived one
feature ago and `{}` *is* the unit type this language already has — an empty
record is a type of its own, consistent with nothing but itself, with exactly
one value. So `` `None {} `` is spelled out, and the projection does **not**
elide the `{}`: eliding a payload position would give two distinct cursor
positions the same rendering, which
`cursor_moves_produce_visibly_distinct_output_at_every_position` forbids and
which would make the payload of a nullary constructor unreachable by `→`. The
beginner voice does say "carrying nothing" rather than "carrying an empty
record", but that is prose about a node, not a cursor-addressable projection,
and it is the only place the `{}` goes unsaid.

**`join` is union**, and that is the one place variants are genuinely dual to
records rather than merely analogous. Records must agree because a record has to
supply every field either branch supplies; a variant has to *accept* every
constructor either branch produces. So `if b then `Red {} else `Green {}`
synthesises `[Red: {}, Green: {}]`, joining payloads where the two overlap and
appending where they do not, keeping the left operand's order for the same
reason `join` on records does.

Union-join is not a nicety — without it the language has no multi-constructor
variants at all. `syn(Inj(c, e))` is the singleton `[c: τ]`, because an
injection knows only its own case; and no annotation can spell a variant (see
below), so if joining were exact, every variant in the language would have
exactly one constructor and the feature would be a one-case record with worse
ergonomics.

**Consistency is by overlap, not by constructor set, and a property test is why.**
The first draft made it character for character the record rule — same
constructor set, consistent payloads — for the symmetry the record entry argued
for. `delete_preserves_well_typedness_everywhere` refused it inside a minute
(seed 4886890018093993446, kept in `action/proptest-regressions/act.txt`). The
counterexample is short: `λf:[Red: ?, Green: ?] → ?. …` applied to
`if b then `Red {} else `Green {}` is well typed, and deleting the `else`
branch replaces it with a hole, whose join with `[Red: {}]` is `[Red: {}]` — a
*narrower* variant, and under the exact rule no longer consistent with the
annotation. The whole program stops typechecking because a subterm was deleted.

That is a violation of an invariant this language depends on everywhere and had
never had to name before: **`syn` is monotone under deletion** — replacing any
subterm with a hole may only make types less informative, never inconsistent.
Records never tripped it because a record's join demands equal field sets, so a
record type cannot narrow. A variant's join is union, so it can, and the
consistency rule has to be one that narrowing preserves. It is: **two variants
are consistent unless they disagree about a constructor they both have.** `[Red]`
is consistent with `[Red, Green]`; `[Red: Num]` is not consistent with
`[Red: Str]`. Symmetric, reflexive, still not transitive — the same three
properties `?` already has, and the tests that check them still pass unchanged.

The analytic rule needed the same treatment and for the same reason. The draft
`ana(Inj(c, e), τ)` succeeded only when `matched_variant(τ, c)` was `Some`;
seed 11933971229440477142 showed an `Inj(Red, …)` analysed against a narrowed
`[Green]` failing where it had succeeded before the deletion. So a constructor
the expectation has never heard of now *widens* rather than fails:
`ana(Inj(c, e), τ)` checks the payload against `matched_variant(τ, c)` when
there is one and against `syn` otherwise. `ana(Inj(…), Num)` still fails, so a
non-variant expectation still quarantines, which is the behaviour that key
matters. This is an *analytic* rule, not width subtyping — it is what lets one
arm of an `if` produce `Red` where the whole `if` is known to be
`[Red, Green, Blue]`.

Both changes were forced by a proptest and neither was in the design an hour
earlier, which is the second time in this phase (see the record-field
`Reidentified` repair) that a property discovered a rule instead of confirming
one. Worth saying plainly: the argument for exact consistency was *correct about
records* and wrong here, and no amount of reasoning by analogy was going to
catch that. Ten thousand cases did.

**`matched_variant` fails open on `?`, and so does the constructor set.**
`matched_variant(?, c) = ?`, and `variant_constructors(?)` is the *empty*
requirement rather than no answer at all. So `λc:?. match c { … }` is a
function over variants that typechecks with any arms at all, exactly as
`λp:?. p.x` is a function over records — the gradual rule every `matched_*`
judgment in this language already follows.

**A variant type is inferred and displayed, never spelled.** The record entry's
argument transfers unchanged and gets sharper: a variant type is a list of
constructor *identities*, `SetAnn` carries a `Ty`, and `action_name`/
`parse_step` round-trip is a tested property that a `Ty` full of uuids cannot
satisfy. There is no `` ` `` in the annotation slot and no variant syntax in
`script::parse_ty`. One consequence is worth stating out loud because it makes
the rest of the design tractable: **a concrete variant type never crosses a
definition boundary**, because the only thing a caller knows about a definition
is its annotation, and an annotation cannot name one. Exhaustiveness is
therefore a property of a single definition's body, which is where the checker
enforces it and where the editor can maintain it.

**Missing arms are unrepresentable; extra arms are legal.**
`syn(Match(e, arms))` requires the arm ids to be distinct and to *cover*
`variant_constructors(syn(e))`. Covering, not equalling: an arm whose
constructor is not in the scrutinee's type is dead, gets payload type `?`, and
is checked like any other. That asymmetry is the whole editing story. A dead arm
is how a constructor comes into being — you add the arm first and inject into it
afterwards — and it is what makes the sequence "add a case, then write the case"
possible at all without a moment where the program is not a program. Since every
action preserves well-typedness and `syn` refuses a match with a missing arm, no
sequence of actions can produce one. `action/tests/exhaustive.rs` asserts this
directly over 10 000 random action walks rather than trusting the argument.

**`AddArm` is the only way to add a constructor, and it sweeps the document.**
It mints a constructor id and a payload binder, appends `(c, x, ⦇⦈)` to the
focused match, **and appends the same arm to every other match in the document
whose arm set is the same set** — one action, one log entry, one `C-z`. This is
`RemoveField`'s document sweep (2026-08-29) and B1's `DeleteDefinition`
(2026-08-28) a third time. Two matches over the same variant are kept in step by
construction, which is the only way "exhaustiveness by construction" can survive
a second use site. Matches are identified by arm set rather than by scrutinee
type because a scrutinee of type `?` has no type to compare, and two matches on
the same `?`-typed parameter of two different functions are exactly the pair the
sweep exists for.

**`RemoveArm` is the same sweep in reverse, and it is refused rather than
repaired.** It removes the arm from every match with that arm set and keeps the
result only if the whole document still typechecks — which it does not if any
scrutinee still injects the constructor. So the refusal is not a special case in
the action; it is `keep_if_well_typed` reporting that the invariant would break.
This is the one place variants decline where records repaired: `RemoveField`
could quarantine every projection and keep going, because a projection has a
subject worth keeping, but there is no coherent quarantine for "this match can
no longer answer" — the arm bodies that remain are fine, and the one that would
have run is the one you just deleted. Refusing is the honest answer, and the
user's route is the ordinary one: delete the injections first, then the arm.

**`SetConstructor` re-aims an arm as well as an injection, and reachability is
why.** The action was written for injections, where the keyboard drives it from
the constructor slot's pick flavour. `reachability.rs` then produced a match
with two *dead* arms over a `?` scrutinee — a legal state, and the state the
`RemoveArm` vacuity check needs to exist — and could not build it: `AddArm`
mints its own identity, so the two constructor ids in the target appeared
nowhere else in the program and nothing could point the arms at them. Records
had answered this exact question already: `AddField` mints, `SetFieldId`
re-identifies, and the reachability recipe uses both. So `SetConstructor` reads
the cursor the same way the constructor slot does — the focused injection, or
the arm the cursor is in — and the match recipe becomes the record recipe with
different nouns. It is guarded by `keep_if_well_typed` like everything else, so
re-aiming an arm onto a case a sibling arm already answers, or off a case the
scrutinee still injects, is refused rather than repaired. It has no key, for the
reason `SetArmBinderId` has none: the keyboard has never set an identity it did
not mint.

**Ordinary `Delete` on an arm body holes the body and keeps the arm.** No
special case, and this is why `RemoveArm` needs to exist at all: the two
operations a text-shaped editor conflates — "empty this case" and "this case is
gone" — are different edits here, and only one of them can change the shape of
the match.

**`ConstructMatch` reads the scrutinee, and a `?` scrutinee gives a zero-arm
match.** On a focus whose type is `[c₁: τ₁, …, cₙ: τₙ]` it writes
`match e { c₁ x₁ -> ⦇⦈ | … | cₙ xₙ -> ⦇⦈ }` with fresh binders and the cursor in
the first arm's body — the checkbox, literally. On a focus whose type is `?` it
writes `match e {}`, which is exhaustive (the requirement is empty), synthesises
`?`, and grows by `C-n`. A zero-arm match is not an error state and does not need
to be: it is the shape of "I am about to case-split on something I do not know
yet", which is precisely what an editor for an unfinished program should be able
to hold. On a focus that is neither — a `Num`, say — the scrutinee is
quarantined, as every other construction key does.

**Constructor ids hash raw; arm binders canonicalise.** The record entry's rule,
applied to a node with both kinds of id in it. A constructor id is
document-global — it is the same identity in an injection here and an arm there —
so it hashes as it is, exactly like a field id and a free variable. An arm's
payload binder binds only that arm's body, so it is pushed on the de-Bruijn stack
around the body and canonicalises like a lambda's, which is what makes
`match e { c x -> x }` and `match e { c y -> y }` the same node. Display names
reach neither: renaming a constructor is one `Rename`, zero node hashes change,
and it cannot conflict with any structural edit in a merge.

**Two branches adding a constructor to the same variant is a conflict, and it
should be.** `AddArm` mints a fresh id, so two branches doing it produce two
different constructors and two arm lists that differ in set, not in order. A
changed arm set is a changed type, so the match diffs as a whole-node `Replace`
— the same coarseness the record entry recorded for a changed field set, for the
same reason, and here it is not merely acceptable but right: the two branches
disagree about what values the type has, which is the definition of a conflict.
What *does* merge, and is the scenario the benchmark gained, is two branches
editing **different arms** of the same match: those are disjoint subtree paths,
so they commute exactly as two edits to different fields of a record do.

**No arm reordering.** Records got `C-←`/`C-→` and `Operation::ReorderFields`
because a record's field order is observable — it is what the projection prints
and what `git` would conflict on. A match's arm order is not: arms are looked up
by constructor id, the evaluator never scans in order, and two orderings of the
same arms are the same program. Adding a reorder action would mean inventing an
observable difference in order to have something to merge, which is the opposite
of the exercise. Recorded as a deliberate omission, not an oversight.

**Keys.** `KEYS.md` settled item 20 has the argument. In short: `` ` `` injects
and `|` matches, taking the last two characters off the reserve list; `m` is
rejected because no letter is ever a verb here; `C-n`/`C-d` generalise a third
time to arms; there is one constructor slot with the field slot's two flavours;
and there is no binding for an arm's payload binder identity, because the
keyboard has never set an identity it did not mint.

---

### 2026-08-29 — What the last Phase B2 feature cost, and what B2 cost altogether

Variants are the fourth and last feature of Phase B2, so this entry is two
things: the bill for one feature, and the verdict on the bet the phase was
run to test.

**Files touched: 57 modified, 20 added** (`store/src/v5.rs`, the seventeen
`store/fixtures/v5/` artifacts, `action/tests/exhaustive.rs`, and the
`action/proptest-regressions/act.txt` that is itself part of this entry). By
layer:

| layer | files | what changed |
|---|---:|---|
| type grammar | 1 | `Ty::Variant`, **overlap** consistency, `matched_variant`, `variant_constructors`, a union `join`, `variant`/`unit`, `Display` |
| expression grammar | 1 | `Exp::Inj`, `Exp::Match`, `Exp::inj`/`match_`/`unit` |
| typing | 1 | `syn`/`ana` for both forms, `arm_payload_ty`, `arms_cover`, the widening analytic rule — and four arms lifted into helpers to fit the stack |
| document | 3 | `Doc::constructor_ids`, constructor names, examples |
| rendering | 1 | `` `C e `` at `PREC_APP`, `match e { c x -> b \| … }` with the scrutinee at `PREC_ATOM`; **no new precedence constant** |
| zipper | 1 | three frames, one of them variable-arity (`MatchArm` carries the scrutinee, the other arms, and the index) |
| actions | 3 | six actions, the expected-type rules, the generator, the script vocabulary |
| cursor rendering | 1 | three `assemble` arms and the two precedence rows the plain renderer has |
| serialisation | 7 (+1 new) | v5→v6, node tags 18–19, `Ty` tag 8, action tags 38–43, `store/src/v5.rs`, 17 v5 fixtures, the migration suite |
| evaluation | 5 | `Dyn`/`Value` cases, `step_inj`/`step_match`, the incremental engine, two test files |
| diff/merge | 5 | `diff_match`, the path and repair walkers, two scenarios |
| hole context / provenance / encode / text baseline | 4 | injections offered at a variant-typed hole, the arm binder in scope, two JSON tags, `[C: τ \| D: σ]` and `` `C e `` in the measure parser |
| TUI | 10 | `` ` ``, `\|`, the two constructor-slot flavours, `C-n`/`C-d` in a match, completion, the beginner voice, **the state-machine projection**, the matrix, the reference fixtures |
| CLI | 2 | an exhaustiveness arm, and a test |
| property suites | 4 | the reachability recipe and its two new regression seeds, the sensibility generator's dead-arm flavour, the cross-document rename test |
| benchmark | 5 | reference 4 rebuilt as a real variant and match, both of its fixtures, `MERGE.md`, `RESULTS.md`, `references.md` |
| docs | 3 | `KEYS.md` item 20, `FORMAT.md` v6, this file |

Those seventeen rows account for the fifty-seven modified files exactly:
fourteen of them are layers of `CONTRIBUTING.md`'s full-thread checklist, and
the last three are what the checklist produces rather than what it lists — the
property suites, the benchmark, and the documents.

**Tests: 49 added**, 33 honestly adapted, 0 deleted. The count was 818
before and is 867 after. The action alphabet went from 38 variants to 44, the
`Exp` grammar from 18 to 20, and sensibility still passes at 10 000 cases over
the enlarged grammar and at every cursor position of a generated *document*.

**Adapted, each for a real reason**, and the thirty-three group into five
reasons, not thirty-three:

- **Nine** are `tui/tests/matrix.rs` — its eight columns plus the alphabet
  test. The matrix is a table of every printable character against eight
  cursor situations, so a feature that takes two characters off the reserve
  list adapts all nine by construction. This is the matrix doing its job, not
  going stale.
- **Six** follow benchmark reference 4, which is a match now rather than a
  chain of `if`s: `beginner::snapshot_state_machine_fixture`, the two
  `state_machine` projection tests, the two cross-projection tests in
  `tui/tests/projections.rs`, and
  `eval/tests/references.rs::reference_four_state_machine_transitions`, which
  used to assert `transition(0) == 1` and now asserts
  `` `Idle {} `` ↦ `` `Running {} ``. The recognition test gained a sibling,
  `the_chain_of_equality_tests_is_still_a_state_machine`, so the shape it used
  to recognise is still asserted — a program written before variants existed
  does not stop being a state machine.
- **Three** are the migration suite, where `every_v5_program()` now filters
  variant-carrying programs out of the v5 corpus for the reason the v4 corpus
  filters record-carrying ones, and two v4/v5 assertions moved with it.
- **Four** are the counts that are asserted rather than assumed: the
  reachability form survey (18 → 20 `Exp` variants), the action-variant table
  (38 → 44), and the two sensibility generators that enumerate them.
- **The remaining eleven** are single assertions widened by a grammar that got
  bigger: the vacuity check's flavoured generator, the fresh-name streams, the
  hole census in `core/src/examples.rs`, the projection-precedence test, and so
  on.

The key-hint line is *not* on this list, and that is worth one sentence:
`` ` `` and `|` genuinely did not fit in eighty columns, and the fix was to
shorten the line (`C-n/d add/drop` → `C-n/d ±row`) so that
`definitions.rs::the_definition_pane_is_on_screen_next_to_the_program`
still passes unchanged. See item 7 below.

**What was free.** Undo, redo, the action log's replay, the incremental
cache's invalidation, the node table's framing (a second variable-arity node
needed no format shape change, for the third time), auto-quarantine, the agent
protocol's transport, and the precedence table — a `match` is delimited and an
injection binds like an application, so not one constant moved. The CLI again
needed one match arm and a test.

**What fought back, in order of how much:**

1. **The consistency relation was wrong, and a property test said so in a
   minute.** The full argument is in the design entry above. The short version
   is that the exact rule records use makes `syn` *non-monotone under deletion*
   once `join` is a union, and a variant's `join` has to be a union or the
   language has no multi-constructor variants at all. Two seeds
   (4886890018093993446 and 11933971229440477142, both kept) forced two rules
   that were not in the design an hour before: consistency by overlap, and a
   widening `ana(Inj)`. This is the second consecutive feature where a property
   rejected a *design* rather than an implementation, and the first where the
   rejected design was one this file had already argued for in print.
2. **Reachability could not build a dead arm, and `SetConstructor` grew a
   second reading.** Also in the design entry. The shape of the problem is the
   one records met: `AddArm` mints an identity, so a target program's arm
   constructors appear nowhere the recipe can point at. Records had already
   answered it with `SetFieldId`; the fix was to let `SetConstructor` read the
   arm the cursor is in as well as the injection it is on, and the match recipe
   became the record recipe with different nouns.
3. **The vacuity check, for the third distinct reason in four features.**
   `RemoveArm` never applied once anywhere in the vacuity check's search
   space — three hundred generated documents, every cursor position of each,
   one of every action at each. Nothing was broken: the
   generator only produced matches whose scrutinee's type covered every arm, so
   removing any arm always broke coverage and the action always, correctly,
   refused. The search space had no *dead* arm in it. The generator gained a
   one-in-three flavour that builds a `?`-scrutinee match with two dead arms —
   a state the design explicitly blesses — and the check passes. Strings needed
   nothing here, lists needed a form, records needed the document's real ids,
   and variants needed a program *shape*. The general lesson is now three
   deep: **a vacuity check is only as honest as the generator behind it, and
   every new refusal rule is a new way for the generator to be too polite.**
4. **The scrutinee's precedence was decided by two tools that read the
   projection, not by the design.** `match e { … }` is only unambiguous if `e`
   cannot itself end in a brace or swallow one: `match f {} { … }` could be an
   application of `f` to `{}` or a match on `f`. The measure baseline's
   recursive-descent parser (`measure::text_parse`) resolves it by reading the
   scrutinee with `atom()`, and `cursor_render`'s
   `stripping_markers_reproduces_the_plain_projection_on_generated_programs`
   caught the other half — the cursor renderer printed
   ``match `_61d3ba90 2 {…}`` where the plain renderer printed
   ``match (`_61d3ba90 2) {…}`` — because the two had picked different minimum
   precedences for the same position. Both settle on `PREC_ATOM`: anything
   bigger than an atom is parenthesised. That is a *language* decision arrived
   at through a measurement tool and a property test rather than through the
   design, and `KEYS.md` item 20(b) had written the rule down without noticing
   it was load-bearing — the item's worked example had to be corrected to
   `match ⦇1 + 2⦈ {}` to match what the editor actually does.
5. **The stack budget, a third time, and in a function that had never been
   over it.** The lists entry blamed `step_in`'s arms; the records entry added
   that enum growth spends the budget twice. Both were true again — ten of
   `step_in`'s arms are now helper functions (`step_ap`, `step_bin_op`,
   `step_if`, `step_let`, `step_pair`, `step_proj`, `step_cons`, `step_hole`,
   `step_inj`, `step_match`) — but lifting them was not enough, and the
   remaining overflow was in `nothing_core::typing::syn`, which had never been
   the culprit before. A throwaway probe test was needed to find out which
   recursion was actually deep; four more arms (`syn_cons`, `syn_fold`,
   `syn_record`, `syn_match`) came out of `syn`. The rule to carry forward:
   **every deeply recursive `match` in this workspace is on the same debug
   stack budget, not just the evaluator's**, and the way to find out which one
   is over it is to measure, not to guess.
6. **The constructor pick slot could not pick, and only the mandated test
   found it.** `` ` `` left the cursor on the payload hole, `SetConstructor`
   wanted it on the injection, and `injected_constructor_id` read the focus —
   so every keystroke in the pick slot took the "does not fit here" branch and
   the whole flavour was inert. Nothing else noticed: the matrix rows were
   written from observation and the observation was of a broken slot. It was
   found by writing
   `complete::tests::the_constructors_of_the_expected_variant_outrank_the_rest_of_the_document`,
   which `KEYS.md` item 20 had named before the code existed. This is the
   clearest argument this project has produced for writing `KEYS.md` first:
   the document named a test, the test named a bug, and the bug was in the
   half of the feature a keystroke matrix cannot see.
7. **The 80×12 key-hint line, and this time it did break.** The records entry
   said "the warning stands for the feature after this one", and it was right:
   `` ` `` and `` | `` did not fit, and `C-n/d add/drop` had to become
   `C-n/d ±row` to make room. Three rows at width 80 again. The line is now
   genuinely out of slack, and the next feature will have to drop a hint, not
   shorten one.
8. **The reference-4 write-up claimed more than the fixture delivers, and its
   own test caught it.** The first draft of `bench/references.md` §4 said
   `transition` "can no longer be handed a 7". Rewriting
   `eval/tests/references.rs::reference_four_state_machine_transitions` to
   assert exactly that failed: the parameter is annotated `?`, because a
   variant type cannot be spelled in an annotation, and `?` accepts a number.
   What actually changed is the *behaviour* — `transition 7` used to fall
   through the catch-all `else` and answer `Idle`, and now gets stuck, which
   is the honest thing for a function with no case for its argument to do.
   Both `references.md` and `RESULTS.md` were corrected to say the
   number/state distinction is still owed to the nominal-type debt. Small, but
   it is the fourth time in this phase that writing the test changed the
   claim rather than confirming it.

---

### 2026-08-29 — The Phase B2 retrospective: was the bet right?

`spec-build.md` bets that features get cheap once the thread is wired end to
end, and B2 is four features run under the same rule — *no feature is done
until every layer of the thread is done* — precisely so the bet could be
measured rather than asserted. The four bills, side by side:

| feature | files modified | files added | tests added | tests adapted | test count after | new `Exp` variants | new actions |
|---|---:|---:|---:|---:|---:|---:|---:|
| strings (2026-08-29) | 50 | 5 | 38 | 6 | 748 | 1 | 1 |
| lists | 56 | 17 | 27 | 10 | 775 | 3 | 3 |
| records | 62 | 19 | 43 | 9 | 818 | 2 | 8 |
| variants | 57 | 20 | 49 | 33 | 867 | 2 | 6 |

**The bet was wrong as stated and right in the way that matters.**

Wrong as stated: the number of files a feature touches did not fall. It rose
for three features running and only came back down when the fourth needed one
fewer serialisation file than the third. That is not noise, and the reason is
structural: the full-thread rule means the cost of a feature is proportional to
the *number of layers*, and the number of layers only grows. Strings did not
have a merge scenario to write because the merge benchmark was smaller;
variants had two, plus a state-machine projection, plus a text baseline that
did not exist in Phase 1. **A thread that is wired end to end is not a thread
that is cheap to add to; it is a thread that is impossible to add to
halfway**, which is a different and better property.

The *adaptation* column rose sharply at the end — 6, 10, 9, then 33 — and it
is the one number in the table that would be easy to read as rot. It is not.
Fifteen of the thirty-three are two things: the nine tests of the keystroke
matrix, which gains a row per new key and is therefore adapted by
construction, and the six that follow benchmark reference 4, which this
feature rewrote from an `if`-chain into a real match. Neither group went
stale; both were doing their job. The real signal in that column is that the
number of places asserting a *count* — of `Exp` variants, of actions, of
fixtures — grows with the grammar, which is the same structural fact one
paragraph up, seen from the test suite's side.

Right in the way that matters, on three counts.

- **Nothing was skipped, and nothing rotted.** Four features, sixteen layers
  each, zero deferred work, zero `TODO`, and every earlier feature's tests
  still passing unadapted except where a claim genuinely changed. The
  serialisation layer is the sharpest evidence: format versions 3, 4, 5 and 6
  each shipped with a migrating reader and committed byte fixtures generated
  by the *unmodified* previous encoder, so six versions still open and are
  tested against real old bytes rather than a hypothesis. That does not happen
  to a codebase where features are allowed to land at 80%.
- **The expensive part moved from the middle to the edges.** In strings the
  core cost the most; by variants the core cost almost nothing —
  `Ty::Variant` and `Exp::Inj`/`Exp::Match` are three enum variants, and the
  typing rules are twenty lines — while the *benchmark, the projections and
  the property suites* cost the most. That is the shape of a thread that
  works: the language grammar is a small thing at the centre and the
  measurement apparatus around it is where the work goes, which is exactly
  backwards from a codebase where the type checker is where you are afraid to
  touch.
- **The property suites found four design errors that review did not.** The
  record-field `Reidentified` repair, `ConstructRecord` reading its expected
  type, consistency-by-overlap, and the widening `ana(Inj)`. Every one was a
  *design* fault, caught in under a minute, by a test that existed before the
  feature did. The cost of the full-thread rule is that the suites have to be
  extended every time; the return is that they get sharper every time, and by
  the fourth feature they were rejecting arguments this file had already made
  in print.

The honest summary for B7 to cite: **the full-thread rule did not make
features cheaper, it made them finishable** — and the per-feature cost was
flat-to-rising in files, flat-to-rising in tests, and sharply falling in *the part of
the system you have to be careful in*. If B7 wants a number, the number is
that four features in one phase touched between 50 and 62 files each and added
between 27 and 49 tests each, with no trend downward and no feature left
half-built.


---

### 2026-08-29 — A command is a value, and only `run` performs it

Phase B3 is effects. The phase brief committed the shape before the phase
started: `main` may have type `Cmd`, a **value describing effects**, executed
only by the `nothing run` runtime. Pure evaluation, the live-values pane and
holes are untouched. A `Cmd` renders in the editor the way `[1, 2]` renders —
as a thing you are looking at, not a thing that happened while you looked. This
entry settles the questions the brief left open and records the two designs
that were rejected to get here.

**`Ty::Cmd(Box<Ty>)` is a new type.** `Cmd τ` is a command that, when
performed, yields a τ. It has to be a real type rather than a convention on top
of an existing one, because `bind` needs it:

    bind : Cmd a -> (a -> Cmd b) -> Cmd b

Without a `Cmd` constructor there is nothing for the binder's type to come
from, and the hole in `bind ⦇⦈ x in ⦇⦈` would inherit `?` on both sides. With
it, the second hole is analysed against `Cmd ?` and the binder `x` enters the
context with the element type of whatever the first hole eventually becomes.
That is the whole reason gradual typing has a `matched_` family, and `Cmd`
joins it:

    matched_cmd(Cmd τ) = Some(τ)
    matched_cmd(?)     = Some(?)
    matched_cmd(_)     = None

failing open on `?` exactly as `matched_arrow`, `matched_list`,
`matched_record` and `matched_variant` do. Consistency and `join` are
structural and congruent: `Cmd σ ~ Cmd τ` iff `σ ~ τ`, and `Cmd` is consistent
with `?` and nothing else.

**The four constructors are expression forms, not builtin definitions.** This
is the third time this workspace has taken that fork and the third time it has
gone the same way, for the reasons written down for `fold` (2026-08-29, "A
fold is a primitive, not a library function"): a builtin is a `Var` bound to
one monotype, so `print` would have to be `Str -> Cmd {}` and `bind` would have
to be `Cmd ? -> (? -> Cmd ?) -> Cmd ?` — all-`?` mush that destroys precisely
the hole expectations the previous paragraph is about; a partially applied
builtin is not a `Lam`, so both evaluators would grow closure machinery for a
shape the language does not otherwise have; and it is not cheaper at the
keyboard, because a builtin still has to be *reached* by name. So:

    Exp::Print(Box<Exp>)             print e
    Exp::Readline                    readline
    Exp::CmdPure(Box<Exp>)           pure e
    Exp::CmdBind(Box<Exp>, Id, Box<Exp>)   bind x <- c in k

`bind` and not `seq`. The brief offered the choice. `seq` — perform this, then
perform that, discarding the first result — is `bind` with a binder nobody
mentions, and the editor already has a binder slot with a completion path, a
rename action and a provenance story (`Let`). Adding `seq` as well would be a
second form that means a strict subset of the first, and every layer of the
full-thread checklist would pay for it twice. A program that wants `seq` writes
`bind` and ignores the name; the beginner projection even says so out loud
("then, ignoring the result").

**`bind x <- c in k`, spelled like `let`.** The bound command sits at
`PREC_CMP` and the body at `PREC_BINDER`, and the whole form is `PREC_BINDER`,
which is character-for-character the treatment `let x = e in body` already
gets. This is the entire reason no new precedence constant appears in B3: the
trailing `in` terminates the bound command, so the parenthesisation rules that
exist already are sufficient and the text-baseline parser needed one keyword
and one production, not a new level. `print e` and `pure e` are `PREC_APP` with
their operand at `PREC_ATOM`, like `fst` and `fold`. `readline` is a keyword
atom, like `nil`.

**Typing.** Synthesis where the shape determines the type, analysis where it
does not:

    syn(print e)        = Cmd {}      with  ana(e, Str)
    syn(readline)       = Cmd Str
    syn(pure e)         = Cmd τ       where syn(e) = τ
    syn(bind x <- c in k) = Cmd β     where matched_cmd(syn c) = Some α,
                                            x:α ⊢ syn(k) = γ,
                                            matched_cmd(γ) = Some β

    ana(print e, τ)     if matched_cmd(τ) = Some υ and υ ~ {}
    ana(readline, τ)    if matched_cmd(τ) = Some υ and υ ~ Str
    ana(pure e, τ)      if matched_cmd(τ) = Some υ and ana(e, υ)
    ana(bind x <- c in k, τ)  if matched_cmd(syn c) = Some α
                              and x:α ⊢ ana(k, τ)  when matched_cmd(τ) is Some

`print` yields the empty record, not a fresh unit type. The empty record is
already the thing this language uses for "no information" — it is what a
nullary constructor's payload is (2026-08-29, variants) — and a unit type would
be a fourth base type earning its keep on one expression form.

**`readline` yields `Str`.** The brief asked and the answer is the only honest
one: a line of input is text. Anything else would be a parse, and this language
does not have one.

**Pure evaluation treats every command form as a value.** This is the guard the
brief wrote in capital letters and it is enforced in one function:

    is_value(Print(v))         = is_value(v)
    is_value(Readline)         = true
    is_value(CmdPure(v))       = is_value(v)
    is_value(CmdBind(c, _, _)) = is_value(c)

A `bind` chain reduces its *arguments* — `print (concat "a" "b")` becomes
`print "ab"` in the live-values pane — and then stops. The continuation of a
bind is a **scope**, not a subterm to reduce, in exactly the sense a lambda
body is: it is not entered by the stepper, its holes are not collected as
blocked holes, and the incremental evaluator carries it as
`Value::CmdBind(Box<Value>, Id, Arc<Exp>, IncrEnv)` — a closure by another
name. The live pane on a hello-world shows `bind x <- print "hi" in pure {}`
and never writes to the terminal, which is the property that makes the editor
safe to type in.

**The runtime dispatches on `main`'s type.** `nothing run` computes
`join(ann, syn(body))` for `main` — the most precise type known about it — and
performs it as a command exactly when that type is a `Cmd`. Anything else
evaluates and prints as it has since Phase 6. The alternative, dispatching on
the *shape* of main's body, was rejected because it makes
`main : Cmd {} = ⦇⦈` — the very first state of a program a beginner is
building — evaluate-and-print rather than run-and-report-the-hole, which is
the wrong answer at the only moment the difference matters.

**Effects around holes are the runtime twin of evaluation around holes.** The
executor keeps a stack of pending continuations, pure-evaluates the current
command, and dispatches on the value it gets. When pure evaluation returns
`Indeterminate` — because a hole sits where a command should be — the executor
stops, rebuilds the residual by folding the pending continuations back around
it, and reports it with the same `Outcome::Indeterminate { result, blocked }`
vocabulary Phase 6 already prints. The prints that happened *before* the hole
have happened. They are on the terminal. That is not a bug to be tidied away:
it is the truthful report that this program does two things and only the first
one is finished.

**Fuel is one budget and one flag.** `nothing run --fuel N`, default 200 000,
the same default `eval_doc` has used since Phase 6. Every pure evaluation step
spends one unit and every command performed spends one unit, from the same
counter. The two-budget design — steps for evaluation, commands for the
runtime — was rejected because two numbers require two flags and a user
staring at "out of fuel" would have to know which one ran out to know which one
to raise. One counter, one flag, one message naming the steps and the commands
it got through. An infinite `bind (print "x") _ in main` terminates with exit
status 3 after printing what it printed, which is the same contract a runaway
shell loop offers and a better one than hanging.

#### The two designs this replaces

**Direct side-effecting builtins** — `print : Str -> {}` that writes when it is
applied — is what most languages do and it is unavailable here for a structural
reason, not a stylistic one. This editor evaluates the program *continuously*.
The live-values pane re-evaluates on every keystroke; the incremental engine
caches results keyed on node hash and environment fingerprint and reuses them;
the sensibility proptest applies ten thousand generated actions to generated
documents and the merge benchmark evaluates both sides of twenty-one scenarios.
A side-effecting `print` means every one of those writes to the terminal, that
the number of writes depends on cache hits, and that typing a character into
the middle of a string prints eleven prefixes of it. There is no version of
"just don't evaluate in the editor" that saves this, because evaluating in the
editor is the product. `Cmd`-as-value is the only design where the pane can
show you what your program *will* do without doing it.

**Monadic sugar** — do-notation, an `Effect` type class, `>>=` as an operator,
any of the machinery that makes this ergonomic in a language with a parser —
was rejected because every piece of it is polymorphism, and this language has
none. `bind : Cmd a -> (a -> Cmd b) -> Cmd b` is written above with type
variables for readability but it is not a polymorphic constant in the
implementation; it is a typing *rule* on a syntactic form, which is how a
monomorphic language gets the same expressiveness without a type-variable
grammar, unification, or an instance table. Do-notation specifically is sugar
over a parser, and there is no parser in the pipeline: the layout of `bind` on
screen is a projection decision, and if a flat one-per-line rendering of a bind
chain is wanted later it is a projection, not a syntax.

Sequencing is therefore explicit and visible: four keystrokes per step, one
form per effect, and a program that does three things has three `bind`s in it
where you can see them.

---

### 2026-08-29 — What effects cost, and the one thing that fought back

The first Phase B3 feature, and the first feature since the checklist was
written that adds something the *runtime* has to do rather than something the
type system has to know.

**Files touched: 56 modified, 23 added** (`eval/src/perform.rs`,
`store/src/v6.rs`, the seventeen `store/fixtures/v6/` artifacts,
`cli/tests/authoring.rs`, and reference 7's three fixtures). By layer:

| layer | files | what changed |
|---|---:|---|
| type grammar | 1 | `Ty::Cmd`, consistency, `matched_cmd`, `cmd`, `Display` — the smallest type-grammar row any feature has had, because `Cmd τ` is `List τ` with a different word |
| expression grammar | 1 | `Exp::Print`, `Readline`, `CmdPure`, `CmdBind`, and their constructors |
| typing | 1 | `syn`/`ana` for all four, `join`, and `syn_cmd_bind`/`ana_cmd_bind` lifted out to fit the stack the way the variant arms were |
| document | 2 | the scope walk and the examples |
| rendering | 1 | `print e`/`pure e` at `PREC_APP`, `readline` at `PREC_ATOM`, `bind x <- c in k` bracketed exactly as `let`; **no new precedence constant** |
| zipper | 1 | four frames, and the `ctx_in` arm that puts the binder in scope at `matched_cmd` of what the command yields |
| actions | 3 | four actions, the expected-type rules that make `bind ⦇⦈ in ⦇⦈` useful, the generator, the script vocabulary |
| cursor rendering | 1 | four `assemble` arms and the two precedence rows the plain renderer has |
| serialisation | 8 (+18 new) | v6→v7, node tags 20–23, `Ty` tag 9, action tags 44–47, `store/src/v6.rs`, 17 v6 fixtures, the migration suite |
| evaluation | 6 (+1 new) | `Dyn`/`Value` cases, three `step_in` arms, `is_value`, the incremental engine's `CmdBind` closure, `run_in_counted`, and **`eval/src/perform.rs`, the runtime** |
| diff/merge | 4 | the path walker, the repair walker, `structurally_equal` |
| hole context / provenance / encode / text baseline | 4 | a command offered where a command is expected, four JSON tags, `bind`/`print`/`pure`/`readline`/`cmd` in the measure parser |
| TUI | 8 | `$`, `'`, `>`, the `c` annotation prefix, the `readline` candidate, the beginner voice, the key-hint line, the matrix |
| CLI | 4 | `nothing run --fuel N`, the command dispatch, `StdIo`, the help |
| property suites | 4 | the action-variant tables, the form survey, the reachability recipe |
| benchmark | 5 (+3 new) | reference 7, its two fixtures, its keyscript, `RESULTS.md`, `references.md` |
| docs | 4 | `KEYS.md` item 21, `FORMAT.md` v7, `CHANGELOG.md`, this file |

**Tests: 47 added**, 25 honestly adapted, 0 deleted. The workspace went from
867 tests to 914.

**Adapted, each for a real reason**, and the twenty-five group into six
reasons, not twenty-five:

- **Nine** are `tui/tests/matrix.rs` — the eight columns plus the alphabet
  test. Three new form keys and one new annotation letter adapt all nine by
  construction, which is the matrix doing its job rather than going stale.
- **Two** are `tui/src/complete.rs`, where `readline` joins `nil` in every
  unfiltered candidate list. Each adaptation appends one name to an expected
  ordering; neither changed a *rank*, because `readline`'s `Cmd Str` is
  inconsistent with `Num` and `Bool` and so sorts exactly where `nil` does.
- **Five** are the counts asserted rather than assumed: `Exp`'s constructor
  survey (20 → 24), the two sensibility generators that enumerate the action
  alphabet (44 → 48), the reachability target survey, and the generator's
  own hole census.
- **Three** are the migration suite, where `every_v6_program()` filters
  command-carrying programs out of the v6 corpus for the reason the v5 corpus
  filters variant-carrying ones, and
  `a_version_six_file_carries_a_variant_no_earlier_version_could` now encodes
  with `encode_document_v6` and asserts the version byte rather than asserting
  `VERSION_MAJOR == 6`, which stopped being true the moment the constant moved.
- **Two** are the benchmark: `there_are_six_reference_programs` became
  `…seven…`, and the reference list in `tui/tests/references.rs` grew a row
  that four tests read.
- **The remaining four** are single assertions widened by a grammar that got
  bigger: the hole census in `core/src/examples.rs` (two of them), the
  quarantine-join case in `action/src/act.rs`, and the annotation slot's
  every-prefix-parses sweep, which now sweeps `c`, `cs`, `cmd`, `c[n`, `[cn`
  and five more.

**What was free.** Six layers needed no thought at all. `nothing check`
needed no code. The state-machine projection needed no code — a command is
not a state machine, and the recognizer already refuses everything that is
not a chain of equality tests. The precedence table needed no new constant,
because `bind` was deliberately shaped as `let` (see the design entry above)
and `print`/`pure` were deliberately shaped as `fst`. Content addressing
needed one `stack.push` in the `CmdBind` encoder, because a bind's binder
binds only its body, which is exactly what a `Let` already did. The name
table needed nothing. And `Ty::Cmd` is the first type constructor whose
consistency rule, `matched_` rule, `join` and `Display` were all *four* lines
long, because `List` had already paid for the shape.

**What fought back.** Three things, in ascending order of how long they took.

**One:** the annotation slot's `c`. `KEYS.md` had `c Cmd` written into the
grammar screen before the code existed, and the code that was supposed to
implement it did not — `accept` returned `Exit` for `c`, so typing it left
the slot and started a name run. That is the kind of gap a hand-written
matrix row catches and a hand-written prose claim does not, which is the
argument for the matrix. Fixing it turned up a second, real problem: `tokens`
suppresses a letter that follows a letter, so that `list` does not read as
`Str` at the `s`, and under that rule `cs` read as one word and meant `Cmd ?`
rather than `Cmd Str`. The fix is a rule, not a special case — **a type
prefix ends the word** — and it is right for the same reason `[` never had
the problem: a prefix is complete when you have typed it and what follows is
a new operand. `cmd` still spells `Cmd`, because `m` and `d` are not tokens.

**Two:** `structurally_equal` in `merge/src/diff.rs` ends in `_ => false`, so
adding four `Exp` variants did not produce a compiler error — it produced a
diff engine that could not reproduce *any* branch containing a command.
Nothing in the checklist would have caught this by reading; what caught it was
`replaying_a_diff_onto_its_own_ancestor_reproduces_the_branch`, a proptest
whose generator had just learned to make commands, failing at case 1345768255
with a program containing `pure false` and `Cmd Bool`. This is the strongest
argument the property suites have made for themselves so far: the wildcard arm
is invisible to `rustc` and to a careful reader, and a generator that quantifies
over the whole grammar found it in one run. The lesson is recorded here rather
than fixed structurally, because the alternative — exhaustive matches
everywhere, no wildcards — is a rule this codebase has not adopted and would
have to adopt everywhere or nowhere.

**Three, and the real cost of the feature:** the runtime itself. Every other
layer in the table is a translation of the same idea into another vocabulary,
and `eval/src/perform.rs` is 185 lines of something the codebase did not have:
a loop that alternates between *evaluating* and *doing*. Its shape is settled
by two constraints that pull in opposite directions. Pure evaluation must run
to a value before anything is performed, or `print ("a" ++ "b")` would write
`"a" ++ "b"`; and the residual of a run that stops at a hole must be the part
of the program that has *not* run, which means the pending continuations have
to be folded back around the blocked command instead of being discarded. The
executor is therefore a stack of `(binder, continuation)` pairs and a `rebuild`
that unwinds it, and every early return in the loop goes through `rebuild`.
That is the whole of the difficulty, and it is worth naming because the naive
version — recurse on the continuation — passes the hello-world test and
produces a residual that names no hole at all.

**Verdict: worth it, and the cheapest structured feature yet.** Fifty-six
modified files sounds like the variant bill, but the layer table is doing
different work: fourteen of the seventeen rows are one or two files, and the
mass is in serialisation (a whole extra version's worth of fixtures, which is
the format's own policy) and in the runtime, which is genuinely new capability
rather than a translation. The keyboard cost three keys and had three spare;
the benchmark gained a reference program whose ratio (0.45×) is second-worst
in the table for the honest reason that it is the shortest program in it. The
prediction the variants entry made — that the next feature would have to
retire a key hint — is now literally true rather than nearly true: the hint
line is at 160 of its 160 columns, pinned by a test.

---

### 2026-08-29 — The standard library is an ambient prelude, not an import

**Decision.** The standard library's thirty-seven definitions are in scope in
every session and every program, by default, without being written anywhere.
They are in the typing context, in the name table and in the doc table, so
completion offers them, `construct-var` resolves them, and `nothing run`
finds them — and a saved document contains **none** of them: not their
bodies, not their names, not their doc lines. A program that calls `min`
stores an ordinary `Var` node holding `min`'s id, and nothing else.

**The alternatives, and why not.**

- **Copy the definitions into the document** (a "prelude paste"). This is
  what a language with no module system usually does, and it is the one
  option that needs no new concept anywhere. It is also wrong in the way that
  matters most here: two documents that both call `map` would carry two
  copies of `map`, with two different ids, and the merge engine — which
  matches definitions by identity — would see an add-and-delete pair rather
  than the same function. Content addressing would give the copies the same
  *hash* and different *identities*, which is the worst of both.
- **An import form** (`use min`, or a node that names a library). This is the
  honest general answer and it is a whole feature: a new `Exp` variant or a
  new document section, a resolution rule, a name-collision rule, an error
  state for an import that does not resolve, a merge rule for competing
  import lists, and a format version to carry it. v0.1.0 has one library and
  it is shipped inside the binary. An import form would be paying the price of
  a package system to describe a set that never varies.
- **Make stdlib references a new node** (`StdRef(name)`). Rejected for
  exactly the reason `DefRef` was rejected in the 2026-08-28 entry
  "Definition references are `Var`": a definition is a binder, a reference to
  one is a variable, and a second kind of reference means every walker in the
  codebase — typing, rendering, diff, repair, content addressing, the
  zipper — grows an arm that does the same thing as the arm beside it.

**How it works.** `core::prelude::Prelude` holds the definitions, a name
table, a doc table, and a cached `Ctx`. `EditState` carries one as an
`Arc<Prelude>`; `EditState::under(prelude)` overlays the prelude's names and
docs *beneath* the document's own layer and tells the fresh-id stream to
observe the prelude's ids. Typing runs `Doc::ctx_in(prelude.ctx())`, so a
prelude definition is a binder in scope like any other. Saving calls
`NameTable::own()` and `DocTable::own()`, which return the top layer only —
which is why nothing of the prelude reaches the file.

**Consequences, recorded because they are the interesting part.**

- **Stdlib ids are fixed constants.** They live in `stdlib/std.n`, which is
  `include_bytes!`d into the binary. Two people who call `min` on two
  machines reference the same uuid, so the merge engine matches their calls
  and content addressing hashes them identically. The ids are not derived
  from names: renaming `min` in a future version would keep every existing
  reference working, and *replacing* `min` with a different function under
  the same name would require a new id, which is the correct amount of
  friction for that change.
- **Content addressing is untouched.** A `Var` node hashes its id (or its
  de-Bruijn index, when the binder is inside the term). A stdlib reference is
  a free variable of the document, so it hashes as its id — the same rule
  that already applied to a reference to another definition in the same file.
  No hash changed, no fixture moved.
- **A document may shadow a prelude name, and its own definition wins.**
  `Prelude::extend` drops any prelude definition whose id the document
  redefines, and a document definition with the same *name* and a different
  id simply shadows in the name table. This is the right rule — a file must
  be able to mean what it says — and `FRICTION2.md` point 15 records that it
  currently happens in total silence, which is not.
- **No stdlib id is `MAIN_ID`.** Enforced by a test, so `main` can never
  collide with something the library shipped.
- **`--no-stdlib` exists**, because the standard library itself had to be
  built with nothing in scope.

### 2026-08-29 — Doc lines are metadata beside the tree

**Decision.** A definition's documentation is one line of text, stored in a
`DocTable` keyed by the definition's `Id`, beside the AST. It is not a node,
it has no type, it does not participate in content addressing, and no edit to
it can make a program ill-typed. `Action::SetDoc(id, line)` writes one; like
`Rename` it is total, costs exactly one log entry, and cannot fail. Setting
the empty string removes the entry, so "undocumented" has exactly one
representation.

**Why not a node.** The tempting alternative is a `Doc(String, Box<Exp>)`
wrapper, the way a comment is a node in some structure editors. It would put
documentation where it is written, which is nice, and it would break three
things that matter more. It would change every hash, because the tree would
be different. It would give the cursor somewhere to stand that is not part of
the program, which the zipper's whole design says it should not have. And it
would make "add a doc" an edit that can fail — a wrapper has to go somewhere,
and somewhere is a position in a tree that is not always available.

**Why `DocTable` rather than a field on `NameTable`.** They are the same
shape — layered overlays over an id-keyed map, flattened before writing — and
a single table of `(name, doc)` pairs would halve the code. It would also
mean that every `Rename` has to carry a doc line and every `SetDoc` has to
carry a name, or that one of them can clear the other by accident. Two tables
with the same shape and no shared state is the version where neither action
can damage the other's data, and `merged_docs` mirrors `merged_names` line
for line rather than sharing a code path that has to branch.

**Consequences.** Format v8 inserts a doc-table section between the name
table and the action log (`FORMAT.md` §7.1); v1–v7 files decode with an
*empty* doc table, which is the honest reading of a file written before doc
lines existed. The merge engine gained one conflict kind,
`CompetingDocs` — a doc line merges exactly the way a display name does. The
protocol reports a definition's doc in `state`, and a binding's doc in
`hole_context`. The TUI shows the highlighted candidate's doc on a line of
its own under the status line. And nothing in the evaluator, the diff, the
content addressing or the type checker knows doc lines exist at all, which is
the test of whether "metadata" was the right word.

### 2026-08-29 — The `?` mush passed its revisit trigger, with evidence

The design commitments name the trigger for revisiting the
no-polymorphism-in-v0.1.0 decision: "ten functions of `?` mush". The standard
library passed it on its first day. This entry is the evidence, as the
commitments require — **not** a redesign, and not a proposal.

**The count.** Seventeen of thirty-seven stdlib signatures contain a `?`.
Fifteen of those are `?` because the function is genuinely generic; two
(`print_labelled`, `print_all`) are `Cmd ?` because there is no unit type,
which is a different missing feature.

```
is_empty   List ? -> Bool           map        (? -> ?) -> List ? -> List ?
length     List ? -> Num            filter     (? -> Bool) -> List ? -> List ?
append     List ? -> List ? -> …    any        (? -> Bool) -> List ? -> Bool
reverse    List ? -> List ?         all        (? -> Bool) -> List ? -> Bool
take       Num -> List ? -> List ?  count      (? -> Bool) -> List ? -> Num
drop       Num -> List ? -> List ?  head_or    ? -> List ? -> ?
swap       ? * ? -> ? * ?           map_fst    (? -> ?) -> ? * ? -> ? * ?
                                    uncurry    (? -> ? -> ?) -> ? * ? -> ?
```

**The cost is not aesthetic.** A `?` in a signature is not a weaker way of
saying "any type"; it is a hole in the checking. Concretely, driven through
the protocol during the B4 friction session:

```
probe : List ? = map not (todo_report nil)
well_typed: true    quarantines: 0
```

`not : Bool -> Bool` mapped over a `List Str`, accepted with nothing
quarantined, because `map : (? -> ?) -> List ? -> List ?` has no way to say
that its second `?` is the first one's *codomain* or that its third is the
element type of its second. `not` is consistent with `? -> ?`; a `List Str`
is consistent with `List ?`; the composition is consistent with everything.
The library's most-used function does not check its own argument, and neither
does `filter`, `any`, `all`, `count`, `map_fst` or `uncurry`.

**Two related observations, so the record is complete.**

- **The annotation is doing all the work.** `todo_report : List Str -> List
  Str = λxs:List Str. map todo_bullet (filter todo_kept xs)` is well-typed
  because `List ?` is consistent with `List Str`, not because anything
  checked that `todo_bullet` returns a `Str`. Every generic call site is
  re-anchored by the enclosing annotation, which works exactly as long as
  there is one.
- **Completion cannot rank what it cannot distinguish.** Ranking is by
  consistency with the expected type; a signature made of `?` is consistent
  with everything, so all seventeen sort into the same bucket. The candidate
  list gets longer and no more useful the more generic the library gets.

**What this entry does not do.** It does not propose parametric polymorphism,
does not sketch an inference algorithm, and does not schedule the work.
v0.1.0 ships gradual typing and a `?`-shaped standard library, and the
programs in `bench/references.md` and the B4 friction session all type-check
and all run. The trigger has fired; the evidence is written down; the
decision to act on it belongs to whoever plans v0.2.0, and they now have a
number (17 of 37), a reproduction (`map not xs`), and a named consequence
(annotations, not signatures, are what hold generic code together today).

### 2026-08-29 — What the standard library cost

Phase B4 is the first phase since the checklist was written whose main
deliverable is **not** a change to the language. Nothing was added to the
grammar: no node tag, no type constructor, no operator, no key. What it adds
is a document written in the language that already existed, plus the
machinery to have that document in scope, plus one piece of metadata.

**Files touched: 36 modified, 29 added.** By layer:

| layer | files | what changed |
|---|---:|---|
| type grammar | 0 | nothing — B4 adds no type |
| expression grammar | 0 | nothing — B4 adds no form |
| document | 3 (+2 new) | `core/src/docs.rs`, `core/src/prelude.rs`, `Doc::ctx_in`/`is_well_typed_in`, `NameTable::own` |
| actions | 3 | `Action::SetDoc`, `EditState::{docs, prelude, under, doc_line}`, the `set-doc` step, the variant tables |
| serialisation | 6 (+19 new) | v7→v8, the doc-table section, action tag 48, `store/src/docs.rs`, `store/src/v7.rs`, 18 v7 fixtures, the migration suite |
| merge | 2 | `DocVersion::documented`, `merged_docs`, `CompetingDocs`, three tests |
| protocol | 5 | docs in `state` and `hole_context`, `SetDoc` json, provenance stamps, the `stdlib` and `move_to_hole` methods |
| TUI | 3 | `Origin::Stdlib`, `Candidate::doc`, the `std·` marker, the doc row under the status line |
| CLI | 6 (+2 new) | `nothing doc`, the prelude in `run`/`check`/`edit`/`protocol`, `cli/tests/stdlib.rs` |
| the library itself | 2 (+4 new) | the workspace `Cargo.toml`/`Cargo.lock`, `stdlib/Cargo.toml`, `stdlib/src/lib.rs`, **`stdlib/std.n`**, `stdlib/REFERENCE.md` |
| docs | 5 (+1 new) | `FORMAT.md` §7.1 and §11, `KEYS.md` Phase B4, `CHANGELOG.md`, `bench/RESULTS.md`, this file, and `FRICTION2.md` |

**Tests: 51 added**, 4 honestly adapted, 0 deleted. The workspace went from
914 to 965 (`cargo test --workspace`: 50 suites, 965 passed, 0 failed, 2
ignored — the two `agentapi/tests/live_model.rs` tests that call the real
`claude` CLI, ignored before this phase and ignored still).

**Adapted, each for a real reason.** Four. Three are the same reason a version
bump always produces:
`a_version_seven_file_carries_a_command_no_earlier_version_could` now encodes
through `encode_document_v7` and asserts the version byte rather than
asserting `VERSION_MAJOR == 7`, exactly as its v6 predecessor was adapted in
B3; and the two sensibility generators that enumerate the action alphabet went
from 48 variants to 49. The fourth is the friction fix:
`an_action_that_does_not_apply_answers_ok_but_not_applied` asserted the
behaviour `FRICTION2.md` point 1 is about, so it was renamed to
`an_action_that_does_not_apply_answers_not_ok` and its assertion inverted —
the one test in this phase whose *expectation* changed rather than its
scaffolding.

**What the authorship rule actually cost.** The spec's rule for this phase is
that the stdlib must be *built with the product* — "a serialised document
plus its action log, proving it was built with the product". That is not a
documentation requirement, it is a test requirement, and it is the single
most valuable constraint in the phase. `stdlib/std.n` carries 1,289 action-log
entries, and
`stdlib::tests::the_committed_action_log_replays_to_the_committed_document`
replays every one of them from `EditState::empty()` and asserts the resulting
document, doc table and name table are the committed ones — then re-encodes
and asserts the bytes match. The library cannot drift from its own history.

Building it that way also found the bug that a hand-written library would
have shipped: the first authoring run produced twenty-two definitions with
quarantined operands, because `construct-binop`/`construct-cons` wrap the
*focus* as the left operand, and I was writing the operand first. That is a
fact about the editor that only writing 1,289 actions through it teaches.

**What fought back.** Two things.

**One: the format layering.** Version 8 is the first version since 2 to change
the *shape* of the body rather than add a tag, which means the v7 fixtures had
to be generated by an encoder that did not know about doc tables — and had to
keep decoding identically afterwards. The answer is that `encode_defs` /
`decode_defs` (defs + names + log) are frozen for v2–v7 and `encode_defs_v8` /
`decode_defs_v8` are a separate pair that inserts the doc section. Eighteen v7
fixtures were generated with the unmodified encoder *before* the doc table
existed and are committed under `store/fixtures/v7/`, with a test asserting
none of them carries a doc line — the same discipline the v3, v4, v5 and v6
corpora follow, and the reason a migration path is exercised against bytes an
older build really produced rather than against a hypothesis.

**Two: the friction session found a defect in the protocol that had been there
since Phase 8.** An action that does not apply answered `ok: true` with an
error string beside it, while an unresolvable *name* answered `ok: false`. A
client that keys on `ok` — which is what `ok` is for — is wrong exactly at the
most common failure. It destroyed two subtrees in the session before I
understood it (`FRICTION2.md` point 1) and it is now fixed, along with four
others. This is the second phase running in which the friction audit, not the
test suite, found the worst bug of the phase; the pattern is worth naming.

**Verdict: worth it, and the cheapest phase per unit of capability so far.**
Two of the seventeen checklist layers are empty for the first time, and the
mass is where it should be — serialisation (a version's worth of fixtures,
which is the format's own policy) and the library document itself. The
keyboard gained no binding and the benchmark did not move a row, which is
what "this phase adds no language" should look like when it is true. What
the phase actually bought is that `map`, `filter`, `fold`-derived helpers,
`join` and `repeat_str` are now things a program *has* rather than things a
program must first define — which the friction session demonstrates by
building a seven-definition program that calls thirteen of them and defines
none of them.
