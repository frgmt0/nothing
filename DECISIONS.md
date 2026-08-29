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
