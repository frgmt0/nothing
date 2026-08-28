# `nothing`: a language with no source text, and what measuring it actually showed

I spent the last stretch building a programming language called `nothing`. The name is
literal and it is also the pitch: there is nothing between you and the tree. No parser.
No text files. No formatter. No syntax errors, not as a category — the state "this
program does not parse" is unreachable, because nothing ever parses anything.

The AST is the source of truth. Editing is a sequence of typed tree transformations, each
of which provably takes a well-typed program to a well-typed program. The editor and the
language are one artifact, because there is no file format a second editor could open. It
is a projectional editor, built on the action calculus from Omar et al.'s Hazelnut paper,
in Rust, driven from a terminal.

I wrote a twelve-phase spec before writing any code, and every phase had a falsifiable
"done when" line. This is the phase 12 write-up: everything I measured, including the
measurements that did not go the way I wanted.

## The thesis

Three ideas, and they only work together.

**Editing is a typed tree transformation.** An edit is a judgment
`(cursor, program) --action--> (cursor', program')` that carries well-typedness from left
to right. Movement actions, `Delete`, one construction action per syntactic form,
`Finish` — that is the whole edit vocabulary. An action either applies, leaving a
well-typed program, or it is refused and nothing changes. No third outcome, and that
absence is the product.

**Holes make incompleteness first-class.** An *empty hole* `⦇⦈` is a gap where an
expression has not been written yet. A *non-empty hole* `⦇e⦈` — a quarantine, in the UI —
wraps an expression that is well-typed on its own but does not fit its context. `1 + true`
is not an error; it is `1 + ⦇true⦈`, a well-typed program containing a visible mistake. So
the editor never has to say no: type a `Bool` where a `Num` goes, it lands quarantined, and
you keep working. Incompleteness is a value, not a failure state, and that is what makes
"always well-typed" survivable rather than tyrannical.

**Names are not identity.** Every binder is a UUID; the display name is metadata in a
separate table. Rename is a metadata write that cannot fail and cannot collide with a
structural edit, and content hashes exclude names, so two structurally identical functions
with different variable names hash the same. This sounds like bookkeeping. It is the
reason the merge results below are what they are.

The bet: make those three things true at the bottom and three fall out at the top for
free — structural merge that does not care about formatting, incremental evaluation that
a rename cannot invalidate, and an agent edit protocol where a model physically cannot
emit a broken program.

The artifact is eight crates and about 25,800 lines of Rust: AST and bidirectional typing,
the edit calculus and zipper, an evaluator that runs *around* holes, a binary format with
blake3 content addressing, a ratatui editor, structural diff and merge, a JSON-over-stdio
agent protocol, and the benchmark harness. The language itself is deliberately tiny:
numbers, booleans, lambda, application, five binary operators, `if`, `let`, pairs,
projections, and the two hole kinds. No strings, lists, records or polymorphism. The spec
told me I would want those in phase 1 and told me not to, because each multiplies the work
in the action calculus and the keyboard grammar. I will come back to what that cost.

## Keystrokes

The guard came before any of the editor. It is in the README:

> If, after four weeks of using the keyboard grammar, the keystroke ratio versus Neovim
> exceeds 3× on the five reference programs, the action grammar is **wrong** — not
> incomplete, not in need of more keybindings, wrong — and the next sprint is spent
> fixing it, not adding features.

Five reference programs: factorial, a list map, a two-field record constructor plus
accessor, a three-case state machine, and a three-deep nested conditional. I hand-counted
the Neovim keystrokes for each as `i` + content characters + `Esc`: **84, 114, 65, 151,
146**. Those numbers are permanent, because a denominator you are allowed to adjust is not
evidence of anything.

The first run, before the keyboard grammar existed, counted *actions* replayed through the
calculus. The second, after `KEYS.md` and the TUI, counted keystrokes driven through the
real key handler.

| Program | Neovim | actions | ratio | keystrokes | ratio | guard |
|---|---:|---:|---:|---:|---:|:--:|
| factorial | 84 | 16 | 0.19× | 16 | 0.19× | OK |
| list_map | 114 | 22 | 0.19× | 29 | 0.25× | OK |
| record | 65 | 30 | 0.46× | 33 | 0.51× | OK |
| state_machine | 151 | 24 | 0.16× | 24 | 0.16× | OK |
| nested_conditional | 146 | 35 | 0.24× | 31 | 0.21× | OK |

The spec predicted the first run would be terrible, because it counts verbose action
names rather than keystrokes. It came out the other way, and the numbers look better than
they are. An action is not a keystroke: `construct-if` builds an entire
`if ⦇⦈ then ⦇⦈ else ⦇⦈` skeleton in one action, and `set-ann Num -> Num` writes a whole
type. Four of the five fixtures also build a *smaller program* than the reference, because
phase 1 has no recursion, lists or `match`: factorial's recursive call is an empty hole,
`map` is map-over-a-pair, the state machine's `match` is a chain of `if`s. The denominator
still charges Neovim for typing the full reference. Only `nested_conditional` is
like-for-like, and notably it is not the best ratio. That is the number to trust.

The more useful finding in the first run was the composition, not the ratio. Construction
was 43% of all actions; movement was 39%. Cursor movement cost about as much as building
the program, almost all of it walking back up and across after finishing a subtree.
`record` was the worst case — 14 of its 30 actions were movement — and it is no
coincidence that it also had the worst ratio. That went straight into the grammar as a Tab
motion to the next unfilled hole and automatic advance after a construction. The other
cost it flagged, one `set-binder-id` and one `set-ann` per binder, is exactly the column
the grammar collapsed: `\ x 0 : n .` is five keystrokes for a whole `λx0:Num.` skeleton.

Worst case 0.51×, best 0.16×. Nothing this project has recorded has ever exceeded 1×, let
alone 3×. The guard was never in danger — a slightly deflating thing to report about the
mechanism you built specifically to catch yourself. Both tables are asserted in code, so a
regression fails `cargo test --workspace` rather than waiting for someone to re-read a
markdown file. And after the dogfooding session I fixed five friction points and re-ran:
every count and ratio identical, **no fixture edited**. That is the honest limitation as
much as the reassurance, and I will come back to it.

## Merge

This is the result I am least ambivalent about. Sixteen scenarios, each built as three
program versions — a common ancestor and two branches — merged twice: once structurally,
through typed operations diffed against the ancestor, and once by rendering all three to
real multi-line text and handing them to the actual `git merge-file -p --diff3`. The
projection is genuine formatted code with line breaks and indentation, not one long line,
because one long line would rig the comparison.

| | scenarios | clean | clean and correct | conflicts |
|---|---:|---:|---:|---:|
| `git merge-file` on rendered text | 16 | 2 | 2 | 14 |
| structural merge on typed operations | 16 | 13 | 13 | 3 |

Every structural merge result is well-typed: 16 of 16. By class:

| class | scenarios | git clean and correct | git conflicts | structural clean |
|---|---:|---:|---:|---:|
| reordering | 3 | 0 | 3 | 3 |
| renaming | 3 | 0 | 3 | 3 |
| reformatting | 4 | 0 | 4 | 4 |
| moving | 2 | 1 | 1 | 2 |
| control | 4 | 1 | 3 | 1 |

The control class exists to keep me honest. Two branches that rename the same binder
differently, move the same subtree to two destinations, or set the same literal to two
values are *real* disagreements, and an engine that calls those clean is broken, not
clever. Those three are exactly the three structural conflicts. So 13 of 16 clean is 13 of
the 13 that should be clean, and the 3 refusals are the 3 that should be refused; the
fourth control scenario, both branches making the identical edit, is clean on both sides.

The mechanism is two structural facts, not cleverness. A rename's footprint is a binder
identity rather than a region of the tree, so it cannot collide with a structural edit —
that is the entire renaming row. Reformatting produces *zero* operations, because
indentation is not in the tree — that is the entire reformatting row. And subtree identity
is the content hash, so a subtree at a new path with an unchanged hash is a `Move`, and an
edit made inside it on the other branch is rebased onto the new path rather than declared
a conflict.

Conflicts are reported in terms of the program rather than of lines: the competing-rename
report names the binder and both alternatives, and says the two edits touch the same nodes
and do not commute. Where a merge would land ill-typed anyway — one branch retypes a
parameter while the other adds a call site — it repairs itself the way the language repairs
everything, by quarantining the offending subterm, reported and never silent.

## The agent numbers, which did not go my way

This was supposed to be the product pitch, so I am going to report it straight.

Thirty-two tasks in four families, one model (`claude-haiku-4-5`), two conditions, one
call per task, 64 real model calls, zero failed calls, zero retries. Condition A gives the
model the program with the cursor in it, the hole-context query and the action grammar; it
answers with a sequence of actions, and **each action counts as one edit**. Condition B
gives it the program as text plus a syntax legend; it answers with the whole edited
program, and **the whole program counts as one edit**. An edit is invalid if it fails to
parse or fails to typecheck.

| | edits | invalid | invalid-edit rate | reached target |
|---|---:|---:|---:|---:|
| A — action protocol | 132 | 15 | **11.4 %** | 20 / 32 |
| B — text baseline | 32 | 0 | **0.0 %** | 30 / 32 |

The text baseline did not produce a single invalid edit. The action protocol produced
fifteen. Per task rather than per edit: 6 of 32 (18.8 %) versus 0 of 32.

I could dress this up. The denominators are genuinely not the same unit, and the tasks are
genuinely one-to-six-node programs in a language with five operators and no library. But
the correct reading is the simple one: **at this scale a competent model does not need the
protocol to stay syntactically and type-correct in text.** There was nothing to reduce.

What the protocol did buy is measurable and is not the headline rate. The harness checked
well-typedness after every step in condition A: 132 actions emitted, 117 applied, 15
refused, and **0 of the 132 steps left an ill-typed program**. Every refusal cost one
action and changed nothing. Condition B has no such guarantee, only a lucky run — its
failure mode is all-or-nothing, because the unit of edit is the whole program, so one bad
reply discards everything. On these tasks that never happened; on a program of any size it
is the failure that matters.

And every one of condition A's fifteen invalid edits was a *navigation* error, not a type
error. Eight were movements off the end of the tree, mostly `move-child 1` on a lambda
whose only child is the body at index 0; seven were `construct-var n` after the cursor
had drifted outside the lambda binding `n`. The same mistake twice: the model assumed the
annotation was a child it could move into, landed somewhere unexpected, then asked for a
variable not in scope there.

| family | tasks | A edits | A invalid | A reached target | B reached target |
|---|---:|---:|---:|---:|---:|
| fill a hole | 10 | 14 | 0 | 7 | 10 |
| build a small function | 10 | 80 | 14 | 3 | 10 |
| fix a quarantine | 6 | 15 | 0 | 4 | 5 |
| extend a program | 6 | 23 | 1 | 6 | 5 |

Fourteen of the fifteen were in the build family, the only one starting from an empty
program that must construct a binder before using it. The fill and fix families, where the
cursor starts where the work is, produced zero invalid edits across 29 actions. That
build-family gap — reaching the target 3 times out of 10 against the baseline's 10 — is the
real finding, and it is a usability finding rather than a safety one. I made the benchmark
one-shot to hold the run to 64 calls, so the model emits eight to sixteen actions blind,
simulating a cursor it cannot see. Condition B has no cursor to simulate. The interactive
harness does not have this problem: driven one action per call with a fresh hole-context
each time, the same model built the factorial reference program in **16 actions with 0
refusals**. Everything that went wrong in the one-shot arm points at the one-shot design
rather than at the protocol — but I have no benchmark of the interactive loop at scale, and
guessing at what one would say would be fabricating it. Separating the two conditions needs
bigger programs or deeper scope nesting, a larger spend than this phase budgeted.

Scoring condition B needed a parser for the rendered syntax, used by nothing but this
benchmark: a measurement instrument, not a pipeline, and there to score the baseline
against a real parser rather than a strawman.

## What turned out to be wrong

**The benchmark measures the wrong thing, and it says so.** Every keystroke ratio here
comes from fixtures that type a *known program once, correctly, with no exploration, no
mistakes, no backtracking*. Real editing is all three. The five fixes I made after
dogfooding addressed frictions that cost keystrokes *in a session* — repairing a
quarantine, recovering from a wrong turn — and not one moved a single number, because a
fixture never takes a wrong turn. The saving had to be pinned as a test instead: one
asserting that repairing a quarantine from where the repairing keystroke leaves the cursor
is exactly one keystroke cheaper than walking out to the wrapper first. In the session,
one wrong turn cost about 16 keystrokes of garbage that had to be deleted and retyped, and
that cost appears nowhere in the results file. The guard I built to keep myself honest
measures a scenario I do not actually do.

**A proptest over 5,000 branch pairs can be almost entirely vacuous.** The merge property
is "any successful merge of two well-typed branches is well-typed", and the implication is
trivially satisfied by any pair that conflicts — so if the branch generator mostly produces
conflicting pairs, the test passes 5,000 times while checking nearly nothing. Hence a
second test whose only job is to show the first is not vacuous: it merges 1,000 random
pairs and asserts at least 200 merge without conflict. Running it now reports 496 clean, 1
repaired by quarantine, 503 conflicting. The sensibility proptest carries the same guard,
asserting every action variant applied successfully somewhere across more than 50,000
judgments, because "the action was refused" satisfies that property vacuously too. Not
paranoia: during the friction session I found a movement test whose `1 + ⦇true⦈` case
asserted *nothing at all*, because it walked empty holes and that program has none.

**Recursion cost about thirty keystrokes and I took the deal anyway.** Phase 6 offered
`letrec` or a fixpoint combinator, and I picked the combinator for a reason specific to
this language: `Ty::Hole` is consistent with everything and `matched_arrow(?) = (?, ?)`,
so at a `?` annotation `x x` synthesises `?` and typechecks. The whole untyped lambda
calculus is already sitting inside the phase-1 surface, reachable through the annotation
the editor writes by default. The call-by-value fixpoint combinator synthesises `? -> ?`
in the empty context with zero changes to the AST, the typing rules, the zipper, the
renderer or the action calculus, and factorial(12) evaluates to 479001600 today. `letrec`
would have cost a new expression variant, two typing rules, a zipper frame, a construction
action, a rendering case, one of the seven characters held in reserve, and — the part that
matters — re-establishing the 10,000-case sensibility proptest and the 1,000-pair
reachability proptest over a grammar one constructor larger. But `letrec` is one keystroke
and the combinator is roughly thirty, typed at every recursive definition: a straight
regression in the exact thing this project measures, which stays out of the results file
only because none of the five fixtures uses recursion. Worse, a function built this way has
type `?`, not `Num -> Num`, so the editor offers no expected type at its argument, throwing
away the bidirectional typing payoff exactly where you most want it. That is the sharpest
argument against the choice, and the one I expect to revisit.

**The calculus was never the problem; the feedback was.** I ran a two-hour session against
the real binary in a real terminal — about 730 keystrokes, 362 of them captured frame by
frame with the screen read after every single key, the terminal resized down to 46×12
partway through. Five programs, all from empty holes, with wrong turns taken on purpose.
The spec asked for fifteen friction points; I found twenty-seven. **Not one was the
calculus.** Not once did the editor refuse an edit it should have allowed, lose
well-typedness, or leave a state I could not type my way out of. All twenty-seven are
about feedback, navigation, or rendering.

The worst were instructive. The editor silently captured a reference during a binder
rename — `KEYS.md` promised that case would be warned about live, it was not, and the
capture only surfaced three keystrokes later as an error about the *annotation*, for a
problem caused by the *rename*. `Tab` walked empty holes but not quarantines, so the
editor's own answer to "is this finished?" ignored the exact construct that means *not
finished*, reporting "no empty hole in this program" with two quarantines on screen. And
the cursor markers did not distinguish a wrapper from its contents: `x3 ⦇»x1 -5«⦈` and
`x3 »⦇x1 -5⦈«` differ by two brackets in a 120-character line, and misreading that is what
cost me those sixteen keystrokes.

I fixed five. The uncomfortable part is the severity ranking: its top two entries were
persistence and layout, and neither belonged to the phase that surfaced them. `C-q` exited
instantly with no save key and no prompt, so the whole session went into the void. Both
exist now, but that is a structural failure of a phase-ordered plan — dependency ordering
schedules the things that make an editor usable behind the things that make it correct,
and the session meant to evaluate usability runs before them.

## Limitations, stated plainly

The language is tiny — no strings, lists, records, polymorphism, user-defined types or
modules. Four of the five reference programs are approximations forced by that surface and
the honest comparison is the fifth; everything claimed above about merge and about the
agent protocol is claimed about programs in that surface.

There is one user and the user is me. The keystroke ratios come from fixtures, not
sessions. The single real editing session was two hours and produced twenty-seven friction
points, five fixed and nineteen open. The agent benchmark is one model, 32 tasks, one call
each, at a scale where the baseline cannot fail; the one place the protocol wins outright,
16 actions and 0 refusals for factorial end to end, is a single program in the interactive
loop rather than a benchmark.

And the beginner projection, which renders the same AST in sentences with no operator
symbols, has a "done when" criterion reading *verified by showing it to an actual person*.
Nobody has looked at it. It is snapshotted against every example program in the test
suite, which tells me it is stable and nothing about whether it is legible. That is a
question a test cannot answer, and it still wants a person.

## Three paths

The spec's last phase is a decision point, written in at the start so that a project
without one could not quietly become a hobby that ends. Three options, and I have not
picked.

**Research artifact.** Polish the calculus, write it up properly against the Hazelnut line
of work, stop. The calculus is sound, the proptests are real and non-vacuous, and the
friction record is an honest account of what a projectional editor feels like to use. The
measurements would be the deliverable.

**Merge service.** Extract the merge engine as a standalone product with a text-language
frontend. It is 13 of 16 against `git merge-file`'s 2 of 16, and the three it refuses are
the three that should be refused. The engine is valuable to people who will never adopt
the language — but it needs a parser and a projection for a language they already use,
which is a large amount of exactly the work this project was built to avoid.

**Agent SDK.** Extract the edit protocol. The honest state of the evidence is that at toy
scale the guarantee is not yet worth anything measurable, and the one-shot failure mode is
real: models are worse at simulating an invisible cursor than at writing text. But zero
ill-typed states out of 132 actions is a property text editing cannot offer at any scale,
and 16 for 16 in the interactive loop is the protocol used as intended. The bet is that
the guarantee starts paying at a size this benchmark did not reach.

I am going to sit with the numbers before I choose.
