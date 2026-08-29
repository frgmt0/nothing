# Invalid-edit rate: the action protocol versus a text baseline

**There are two runs in this file.** The first, dated 2026-08-28, is immediately
below: 32 small Phase-1 programs, both conditions one-shot. The second, dated
2026-08-29 and starting at [Second run: post-B2 scale](#second-run-post-b2-scale-2026-08-29),
is 32 larger post-B2 programs with strings, lists, records and `match`, with
condition A driven as an interactive one-action-per-call loop. The two tables
are **not** comparable column-for-column; the methodology delta is written out
in the second section, and neither table has been edited to agree with the
other. **The text baseline won both times.**

## First run: Phase-1 scale (2026-08-28)

Measured 2026-08-28. Model: **`claude-haiku-4-5-20251001`**, driven headless
through the `claude` CLI (v2.1.251) with the prompt piped on stdin:

```
claude -p --model claude-haiku-4-5-20251001
```

Every number in this section came out of one executed run of 64 real model calls —
32 tasks, two conditions, one call each. Nothing is estimated and nothing is
hand-written. Reproduce it with:

```
NOTHING_CLAUDE_BIN=$(command -v claude) cargo run -p nothing-agentapi --bin agentbench
```

The full transcript, including the exact prompt sent and the exact reply
received for all 64 calls, is `bench/agent-transcripts/invalid-edit-rate.jsonl`.
The two model-driven construction runs from the same phase are
`factorial.jsonl` and `mixed-authorship.jsonl` in the same directory.

## The question

The claim under test is the one at the top of `spec.md`: that handing a model an
**action calculus** instead of program text removes a class of failure, because
an action either applies and leaves a well-typed program or is refused, and
there is no third outcome. The measurable consequence is the **invalid-edit
rate** — the share of a model's emitted edits that the editor cannot use.

## The two conditions

Both conditions get the same 32 tasks, the same model, the same task
description, and the same starting program. Neither is shown the target
render — the goals are written descriptively, so the text arm is not reduced to
transcription. Both get exactly one call per task; the model answers with its
whole edit in that one reply.

**Condition A — action protocol.** The prompt carries the program rendered with
the cursor in it, the hole-context query (expected type at the cursor, the
in-scope bindings with their types and display names, and the constructions that
are well typed there), and the action grammar. The model answers with a sequence
of actions, one per line. **Each emitted action counts as one edit.** An edit is
invalid if it fails to parse as an action (`parse_error`) or parses but does not
apply at the cursor (`did_not_apply`). Applied actions cannot produce an
ill-typed program — that is what is being bought — so the thing actually being
measured on this arm is the **reject rate**.

**Condition B — text baseline.** The prompt carries the program rendered as
text plus a legend for the surface syntax. The model answers with the complete
edited program on one line. **The whole program counts as one edit.** An edit is
invalid if it fails to parse or if `is_well_typed` rejects it.

Scoring condition B needs a parser for the rendered syntax. One lives in
`agentapi/src/measure/text_parse.rs` and is used by nothing but this benchmark
and its own tests. This does not violate the no-text-intermediate commitment:
it is a **measurement instrument, not a pipeline**. No action, no protocol
method and no projection in the editor ever reads program text; deleting that
file would leave the editor bit-for-bit identical and only make this table
unmeasurable. It is here to score the baseline fairly, which requires giving the
baseline a real parser rather than a strawman.

## Headline numbers

| | edits | invalid | **invalid-edit rate** | reached target | failed calls | retries |
| --- | --- | --- | --- | --- | --- | --- |
| **A — action protocol** | 132 | 15 | **11.4 %** | 20 / 32 | 0 | 0 |
| **B — text baseline** | 32 | 0 | **0.0 %** | 30 / 32 | 0 | 0 |

The honest headline is that **the text baseline did not produce a single invalid
edit**, and the action protocol produced fifteen. Read the next section before
using either number.

## What those numbers do and do not say

**The denominators are not the same unit.** Condition A counts 132 edits
because an action is a small edit and a task takes several; condition B counts
32 because a whole program is one edit. An 11.4 % per-action reject rate and a
0 % per-program reject rate are not directly comparable ratios. Put on the same
denominator — per task, did anything the model emitted get rejected? — the
result is:

| | tasks with at least one invalid edit |
| --- | --- |
| A — action protocol | 6 / 32 (18.8 %) |
| B — text baseline | 0 / 32 (0.0 %) |

**The tasks are small enough that the baseline does not break.** These are
one-to-six-node programs in a syntax with five operators and no library. Haiku
writes them as text without syntax errors and without type errors, every time.
This benchmark therefore does *not* show the action protocol reducing invalid
edits at this scale; at this scale there is nothing to reduce. That is a
genuine result and it is recorded as one.

**What the protocol did buy, measurably.** Across condition A the harness
checked well-typedness after every recorded step:

* 132 actions emitted, 117 applied, 15 refused;
* **0 steps out of 132 left an ill-typed program** — the 117 applied actions
  each landed on a well-typed program by construction, and the 15 refusals
  changed nothing at all;
* every refusal cost exactly one action and left the program where it was.

Condition B has no such guarantee, only a lucky one: its failure mode is
all-or-nothing. An invalid reply there discards the entire program, because the
unit of edit is the entire program. On these tasks that never happened; on a
program of any size it is the failure that matters.

**Both arms are scored leniently in the same way.** A quarantine `⦇e⦈` is a
well-typed program — that is the whole point of the non-empty hole — so a text
reply that leaves one standing is *valid* even though it is wrong. Two of
condition B's replies did exactly that (`fix_bool_argument` answered
`λf:Num -> Num. f ⦇3⦈`, `extend_flip_comparison` answered
`λn:Num. if 1 < ⦇n⦈ then 1 else n`): the model copied the quarantine brackets
out of the program it was shown rather than resolving them. Those count as
valid edits that missed the target, which is why B's "reached target" is 30 and
not 32.

## Where condition A's 15 invalid edits came from

Every one of them is a navigation error, and they cluster into two spellings of
the same mistake:

| outcome | count | what it was |
| --- | --- | --- |
| `did_not_apply` | 8 | a movement off the end of the tree — mostly `move-child 1` on a λ (whose only child is the body, index 0) and `move-next-sibling` with no next sibling |
| `parse_error` | 7 | `construct-var n` where the cursor had drifted outside the λ that binds `n`, so no binder of that name is in scope |

They are the same error twice: the model assumed the λ's annotation was a
child it could move into, landed somewhere unexpected, and then asked for a
variable that is not in scope there. All 15 occurred in 6 of the 32 tasks, and
14 of those 15 were in the `build a small function` family, which is the only
family that starts from an empty program and has to construct a binder before
using it. The `fill a hole` and `fix a quarantine` families — where the cursor
starts where the work is — produced **zero** invalid edits across 29 actions.

| family | tasks | A edits | A invalid | A reached target | B reached target |
| --- | --- | --- | --- | --- | --- |
| fill a hole | 10 | 14 | 0 | 7 | 10 |
| build a small function | 10 | 80 | 14 | 3 | 10 |
| fix a quarantine | 6 | 15 | 0 | 4 | 5 |
| extend a program | 6 | 23 | 1 | 6 | 5 |

The gap in "reached target" on the build family (3 versus 10) is the real
finding of this run, and it is a *usability* finding rather than a safety one:
emitting a whole action sequence blind, in one shot, with no chance to look at
the program between actions, is hard. The model has to simulate the cursor in
its head for eight to sixteen steps. Condition B has no cursor to simulate. The
step-at-a-time harness does not have this problem — driven one action per call
with a fresh hole-context each time, the same model built the factorial
reference program in 16 actions with 0 refusals (`factorial.jsonl`). The
one-shot design here is a deliberate concession to keeping the run to 64 calls;
it costs condition A accuracy and it is the reason its target rate is 20 and
not higher.

## The tasks

32 tasks in four families, derived from the example programs in `core/` and the
reference programs in `bench/references.md`. Definitions are in
`agentapi/src/measure/tasks.rs`, where tests assert that every setup replays to
a well-typed program, that every `fix` task really starts on a `NonEmptyHole`,
that every `fill` task really starts on an `EmptyHole`, that every target parses
and typechecks, and that no setup already equals its target.

"A inv" is the number of that task's emitted actions that were rejected. "hit"
is whether the final program rendered exactly as the target — a secondary
score, not part of the invalid-edit rate.

| task | family | start | target | A edits | A inv | A hit | B inv | B hit |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `fill_double` | fill | `λn:Num. n * ⦇⦈` | `λn:Num. n * 2` | 1 | 0 | yes | 0 | yes |
| `fill_increment` | fill | `λn:Num. n + ⦇⦈` | `λn:Num. n + 1` | 1 | 0 | yes | 0 | yes |
| `fill_condition` | fill | `λn:Num. if ⦇⦈ then 1 else 0` | `λn:Num. if n < 0 then 1 else 0` | 3 | 0 | no | 0 | yes |
| `fill_then_branch` | fill | `λn:Num. if ⦇n⦈ == 0 then ⦇⦈ else ⦇⦈` | `λn:Num. if n == 0 then 1 else ⦇⦈` | 1 | 0 | no | 0 | yes |
| `fill_pair_second` | fill | `(1, ⦇⦈)` | `(1, true)` | 1 | 0 | yes | 0 | yes |
| `fill_constant_false` | fill | `λb:Bool. ⦇⦈` | `λb:Bool. false` | 1 | 0 | yes | 0 | yes |
| `fill_projection_operand` | fill | `λp:Num * Num. fst ⦇⦈` | `λp:Num * Num. fst p` | 1 | 0 | yes | 0 | yes |
| `fill_application_argument` | fill | `λf:Num -> Num. f ⦇⦈` | `λf:Num -> Num. f 10` | 1 | 0 | yes | 0 | yes |
| `fill_let_body` | fill | `let x = 5 in ⦇⦈` | `let x = 5 in x * x` | 3 | 0 | no | 0 | yes |
| `fill_comparison_rhs` | fill | `λn:Num. n < ⦇⦈` | `λn:Num. n < 100` | 1 | 0 | yes | 0 | yes |
| `build_constant` | build | `⦇⦈` | `42` | 1 | 0 | yes | 0 | yes |
| `build_sum` | build | `⦇⦈` | `1 + 2` | 3 | 0 | no | 0 | yes |
| `build_identity` | build | `⦇⦈` | `λn:Num. n` | 6 | 0 | yes | 0 | yes |
| `build_double` | build | `⦇⦈` | `λn:Num. n * 2` | 9 | 3 | no | 0 | yes |
| `build_is_zero` | build | `⦇⦈` | `λn:Num. n == 0` | 8 | 2 | no | 0 | yes |
| `build_clamp_to_one` | build | `⦇⦈` | `λn:Num. if n < 1 then 1 else n` | 11 | 0 | no | 0 | yes |
| `build_let_square` | build | `⦇⦈` | `let x = 7 in x * x` | 9 | 0 | yes | 0 | yes |
| `build_nested_conditional` | build | `⦇⦈` | `λx:Num. if 0 < x then (if 10 < x then 2 else 1) else 0` | 16 | 3 | no | 0 | yes |
| `build_pair_of_argument` | build | `⦇⦈` | `λn:Num. (n, n)` | 9 | 4 | no | 0 | yes |
| `build_apply_twice` | build | `⦇⦈` | `λf:Num -> Num. f 3` | 8 | 2 | no | 0 | yes |
| `fix_bool_in_addition` | fix | `1 + ⦇true⦈` | `1 + 2` | 1 | 0 | yes | 0 | yes |
| `fix_num_in_condition` | fix | `if ⦇1⦈ then ⦇⦈ else ⦇⦈` | `if true then 2 else 3` | 3 | 0 | no | 0 | yes |
| `fix_bool_operand_of_plus` | fix | `λb:Bool. ⦇b⦈ + ⦇⦈` | `λb:Bool. 5 + 6` | 4 | 0 | yes | 0 | yes |
| `fix_bool_argument` | fix | `λf:Num -> Num. f ⦇true⦈` | `λf:Num -> Num. f 3` | 1 | 0 | yes | 0 | no |
| `fix_branch_mismatch` | fix | `if true then 1 else ⦇false⦈` | `if true then 1 else 0` | 1 | 0 | yes | 0 | yes |
| `fix_projection_of_a_number` | fix | `λn:Num. fst ⦇n⦈` | `λn:Num. fst (n, 0)` | 5 | 0 | no | 0 | yes |
| `extend_mul_into_sum` | extend | `1 + 2` | `1 + 2 * 3` | 2 | 0 | yes | 0 | yes |
| `extend_wrap_in_lambda` | extend | `5` | `λk:Num. 5` | 4 | 1 | yes | 0 | yes |
| `extend_bind_with_let` | extend | `7 * 7` | `let y = 7 * 7 in y + 1` | 7 | 0 | yes | 0 | yes |
| `extend_apply_function` | extend | `λf:Num -> Num. f` | `λf:Num -> Num. f 2` | 2 | 0 | yes | 0 | yes |
| `extend_pair_the_result` | extend | `λn:Num. n` | `λn:Num. (n, n)` | 2 | 0 | yes | 0 | yes |
| `extend_flip_comparison` | extend | `λn:Num. if ⦇n⦈ < 1 then 1 else n` | `λn:Num. if 1 < n then 1 else n` | 6 | 0 | yes | 0 | no |

## Reading this honestly

The result this run supports is narrow: **on programs this small, a competent
model does not need the protocol to stay syntactically and type-correct in
text.** The protocol's guarantee showed up as promised — 132 emitted actions,
0 of them able to leave the program ill-typed — but the baseline never needed
the guarantee, so the two rates do not separate in the protocol's favour here.

What the run does surface is a cost: one-shot action sequences are harder for
the model to get right end-to-end than one-shot text, because the model must
track a cursor it cannot see. Everything that went wrong in condition A was
cursor tracking, and none of it was type errors. That points at the fix — give
the model the state between actions, which is exactly what the `drive` harness
does and where the same model went 16-for-16 — rather than at the protocol.

A run that would separate the two conditions needs bigger programs, deeper
scope nesting, or a model under more pressure. That is a larger spend than this
phase budgeted, and guessing at its outcome here would be fabricating it.

## Run metadata

| | |
| --- | --- |
| date | 2026-08-28 |
| model | `claude-haiku-4-5-20251001` |
| CLI | `claude` v2.1.251, headless (`-p`), prompt on stdin |
| tasks | 32 |
| model calls | 64 (32 per condition, one per task) |
| failed calls | 0 |
| retries used | 0 (the harness retries once on a transient failure; it was never needed) |
| transcript | `bench/agent-transcripts/invalid-edit-rate.jsonl` |
| harness | `agentapi/src/bin/agentbench.rs`, tasks in `agentapi/src/measure/tasks.rs` |


## Second run: post-B2 scale (2026-08-29)

Measured 2026-08-29 on 32 **new** larger programs — strings, lists, records,
variants, `match`, `fold` — with condition A driven as an interactive
one-action-per-call loop rather than one shot. Same model,
**`claude-haiku-4-5-20251001`**, same headless CLI. Every number below came out
of one executed run of 385 real model calls; the full transcript, with the
exact prompt sent and the exact reply received for each of them, is
`bench/agent-transcripts/post-b2-invalid-edit-rate.jsonl`.

**The text baseline won again.** It emitted 0 invalid edits out of 32
(0.0 %) and the action protocol emitted 9 out of 315
(2.9 %); the baseline also hit the exact target on more tasks
(30 / 32 one-shot and 31 / 32 interactive, against
23 / 32 for the protocol). Scaling the programs up did not flip the
result. What did change is the size of the gap and the shape of the failures,
and those are worth more than the verdict — see the two sections below.

### What is different from the first run

Five things changed, and each of them changes what the second table means:

1. **Bigger programs.** The first run's tasks were one-to-six-node arithmetic
   and boolean terms. These are post-B2 programs: string concatenation, `List`
   values built with `::`/`nil`, `fold`, record literals and projection,
   variants and `match`, and `Cmd`/`bind`/`print`. The targets average 90
   rendered characters against 17 for the first set — a little over five times
   the size, and a test in `tasks.rs` holds the ratio above three so the two
   sets cannot drift back together. Both sets live side by side in
   `agentapi/src/measure/tasks.rs`; the first set is untouched, so the
   2026-08-28 table stays reproducible with `--tasks original` (still the
   default).
2. **Condition A is now an interactive loop.** One model call per action. Each
   prompt carries the program rendered with the cursor marked, the hole context
   the editor offers at that cursor, the full action grammar, the goal, and the
   last few actions with their outcomes. The loop stops when the program
   matches the target, when the model replies `done`, or at a step cap of
   30. This is the direct answer to the first run's finding that
   *every* one of condition A's invalid edits was the model tracking a cursor
   it could not see.
3. **The baseline legend was rewritten.** Condition B's syntax legend now
   covers strings, lists, records, variants, `match`, `fold` and `Cmd`,
   because otherwise the baseline would be asked to write programs in a syntax
   it was not shown. Every example in that legend is proven to round-trip
   through `core/src/render.rs` by a test
   (`every_syntax_example_round_trips_through_the_renderer`), so the baseline
   is being shown the real surface and not an approximation of it.
4. **A third arm, B2, was added** — the text baseline given the same
   interactive treatment as A. It is an addition, never a replacement; see
   *Why the baseline got a second arm* below.
5. **The harness runs tasks in parallel** across 10 worker threads (std
   only), with results collected by index so the reported order is
   deterministic regardless of completion order, and the transcript flushed
   after every completed task.

Because of (1) and (2), **the second table is not comparable column-for-column
with the first.** Neither table has been edited to agree with the other.

### The three conditions

**A — action protocol, interactive.** One action per model call, against a live
editor session. Every emitted action counts as one edit. An edit is invalid if
the harness cannot parse it into a step (`parse_error`) or the editor refuses
it (`did_not_apply`). A `done` reply ends the task and is not counted as an
edit. Well-typedness is recorded after every step.

**B — text baseline, one shot.** Unchanged in kind from the first run: the
program as text plus the (now larger) syntax legend, one reply containing the
whole edited program. The whole program is one edit; it is invalid if it fails
to parse or fails `is_well_typed`.

**B2 — text baseline, interactive.** Same prompt as B, but the model gets up to
5 turns, each showing the current program and whether the previous reply
was accepted or rejected and why. Each reply is still one whole-program edit.

### Headline numbers

| | edits | invalid | **invalid-edit rate** | reached target | failed calls | retries |
| --- | --- | --- | --- | --- | --- | --- |
| **A — action protocol (interactive)** | 315 | 9 | **2.9 %** | 23 / 32 | 0 | 0 |
| **B — text baseline (one shot)** | 32 | 0 | **0.0 %** | 30 / 32 | 0 | 0 |
| **B2 — text baseline (interactive)** | 32 | 0 | **0.0 %** | 31 / 32 | 0 | 0 |

| | model calls | tasks with an invalid edit | `parse_error` | `did_not_apply` | type error | steps leaving an ill-typed program |
| --- | --- | --- | --- | --- | --- | --- |
| A | 320 | 7 / 32 (21.9 %) | 6 | 3 | 0 | 0 |
| B | 32 | 0 / 32 (0.0 %) | 0 | 0 | 0 | 0 |
| B2 | 33 | 0 / 32 (0.0 %) | 0 | 0 | 0 | 0 |

The denominators still are not the same unit — an action is a small edit and a
whole program is a large one — which is why the second table is here. On the
per-task denominator the protocol went wrong somewhere on
7 of 32 tasks and both text arms on none.

The one number that moved in the protocol's favour is its own: **2.9 %, down
from 11.4 % in the first run, on larger programs.** Showing the model the
cursor between actions removed roughly three quarters of the invalid edits,
which is what the first run predicted would happen. It did not remove enough of
them to reach the baseline's zero.

### By family

| family | tasks | A calls | A edits | A invalid | A reached | B reached | B2 reached |
| --- | --- | --- | --- | --- | --- | --- | --- |
| fill a hole | 8 | 27 | 26 | 0 | 7 / 8 | 8 / 8 | 8 / 8 |
| extend a program | 8 | 83 | 82 | 2 | 6 / 8 | 8 / 8 | 8 / 8 |
| fix a quarantine | 8 | 14 | 14 | 0 | 8 / 8 | 8 / 8 | 8 / 8 |
| build a small function | 8 | 196 | 193 | 7 | 2 / 8 | 6 / 8 | 7 / 8 |

The concentration from the first run survived: 7 of the protocol's 9 invalid
edits, and 196 of its 320 model calls, are in *build a small function* — the
family that starts from an empty hole and has to construct everything. `fix a
quarantine` is the mirror image: 14 calls for 8 tasks, no invalid edits, 8 / 8
on target. Repairing something the editor already built is where the action
protocol is at its strongest, and building from nothing is where it is weakest.

### Where condition A's 9 invalid edits came from

Every one of them. The cursor position each was emitted at is in the
transcript, in the `render_with_cursor` field of the preceding step.

| task | outcome | action | why the editor refused it |
| --- | --- | --- | --- |
| `extend_name_with_last` | `parse_error` | `Looking at the task and current state:` | unknown action `Looking` |
| `extend_join_into_let` | `parse_error` | `Looking at the current state, I'm on ...` | unknown action `Looking` |
| `build_exclaim` | `parse_error` | `construct-var s` | no binder named `s` is in scope |
| `build_exclaim` | `parse_error` | `construct-var s` | no binder named `s` is in scope |
| `build_name_list` | `did_not_apply` | `finish` | the editor had no such move at that cursor |
| `build_labelled_count` | `did_not_apply` | `set-ann Str` | the editor had no such move at that cursor |
| `build_choose_greeting` | `parse_error` | `construct-var formal` | no binder named `formal` is in scope |
| `build_status_word` | `did_not_apply` | `move-next-sibling` | the editor had no such move at that cursor |
| `build_status_word` | `parse_error` | `set-constructor Bad` | no constructor named `Bad` in this document |

Only one of these — the `move-next-sibling` — is a movement off the end of the
tree, the error that dominated the first run. The cursor is now printed in
every prompt, and the model reads it. What replaced cursor drift is **name bookkeeping**: four of
the 9 name a binder or constructor that does not exist yet
(`construct-var s` twice and `construct-var formal` before the λ that binds
them had been created; `set-constructor Bad` when the editor had minted the arm
as `C0`). Two more are not edits at all — the model wrote a paragraph of
reasoning where an action was expected, and the action parser refused the first
word. The remaining two are a `finish` and a `set-ann` at positions that admit
neither.

That is a better class of error than the first run's. It is also a smaller one,
and it is still nine more than the baseline produced.

### How the two conditions fail differently

The interesting result of this run is not the rates, which the baseline wins
again, but that **the two conditions fail in ways that are not the same kind of
thing.**

Condition A never produced a program that was wrong in the sense of being
broken. 320 steps were recorded and **0 of them left an ill-typed
program**; every refusal cost exactly one action and left the program exactly
where it was. All 9 of A's misses are well-typed programs, and the transcript is
unusually legible about what went wrong in each. Four of them are the *right
term in the wrong place*, two differ only in associativity, and three are
names the editor minted that the model could not change:

* `build_name_list` built `λfirst:Str. first :: "bob" :: "cy" :: nil`
  correctly and then left it as the head of a list —
  `(λfirst:Str. first :: "bob" :: "cy" :: nil) :: nil`. The term is right; it
  is one constructor too deep.
* `build_choose_greeting` did the same thing three times over, ending at
  `if ⦇if ⦇if ⦇λformal:Bool. ...⦈ ...⦈ ...⦈` — it kept constructing at the
  cursor instead of moving to the hole first, and the quarantine brackets
  faithfully recorded each mistake instead of hiding it.
* `build_exclaim` and `fill_greeting_formal_branch` differ from their targets
  only in associativity: `"hello, " ++ (s ++ "!")` where the target renders
  `"hello, " ++ s ++ "!"`.
* `extend_match_with_stopped_arm` hit the step cap having added twelve
  `add-arm` arms named `C0`…`C11`, two of them correctly returning
  `"stopped"`. It could not name an arm, so it kept making new ones.
  `build_labelled_count` is the same problem in one node: it built
  `{f0 = tag ++ "!", count = n + 1}` — structurally the target, with the first
  field left at the name the editor minted instead of `label`.

Condition B fails in exactly one way instead, and it is the opposite failure:
its 2 misses are programs that parse, typecheck, and are *structurally
correct*, differing from the target only in names the goal had specified.
`build_total_of_list` answered `λns:List Num. fold ns 0 (λh:Num. λa:Num. a + h)`
where the target binds `acc`, not `a`. `build_status_word` answered
`λs:?. match s { Ok x -> "fine" | Bad y -> "broken" }` where the target keeps
the editor's own payload binders `x0` and `x1`. B2 fixed the first of those on
its second look at the program and still missed the second, ending with both
payload binders shadowed to the same name `x`.

So: **the protocol's failures are mostly misplacements, and the baseline's are
only ever misnamings.** Naming is the one failure both arms share — and it is
the only failure the baseline has. The protocol cannot produce an ill-typed
program and did not; the
baseline can and — on programs this size, with this model — did not either. The
protocol's guarantee is structural and holds by construction across all 320
steps. The baseline's clean sheet is empirical, and it is a real result, but it
is a claim about 32 whole-program rewrites of programs that still fit on one
line. Nothing in this run tells you what happens to it at ten times the size,
and nothing in this file will pretend otherwise.

One asymmetry is worth naming because it cuts against the protocol: A spent
320 model calls to B's 32 and B2's 33, and reached the target less often.
Per unit of model attention the text baseline is far ahead here.

### Why the baseline got a second arm rather than a rewrite

Condition B was left exactly as it was — one shot, whole program, same shape as
the 2026-08-28 run — and B2 was added beside it.

The fairness question is real: condition A was handed a loop this run, and a
loop is a large advantage. If the baseline does not get one, the comparison is
rigged. But *replacing* B with an interactive form would have been a different
kind of rigging — it deletes the only column directly comparable to the
2026-08-28 table, and it is a change to the measuring instrument made after
seeing which way the numbers went. Post-hoc changes to a baseline are suspect
whichever direction they push the result. Adding B2 costs one extra arm and
settles the question without touching the old one: if interactivity is what
helped A, the baseline gets it too, and the reader can see both. It helped —
B2 reached 31 / 32 against B's 30 / 32, and B2 is the arm the protocol has to
beat.

B2's turn cap is 5 where A's step cap is 30, because the units differ. One
B2 turn rewrites the whole program; one A step moves the cursor or builds one
node. Equal call counts would have handicapped A; equal *chances to get the
program right* is the comparison B2 is built for. In practice B2 used 33 calls
for 32 tasks — it almost never wanted a second turn.

### Per task

| task | family | A calls | A edits | A inv | A hit | B inv | B hit | B2 edits | B2 inv | B2 hit | target |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `fill_row_score_step` | fill | 5 | 5 | 0 | yes | 0 | yes | 1 | 0 | yes | `let mk = (λn:Str. λs:Num. {name = n, score = s}) in let a = mk "ada" 90 in let b = mk "bob" 70 in let rows = a :: b :: nil in fold rows 0 (λr:?. λacc:Num. acc + r.score)` |
| `extend_row_score_double` | extend | 2 | 2 | 0 | yes | 0 | yes | 1 | 0 | yes | `let mk = (λn:Str. λs:Num. {name = n, score = s}) in let a = mk "ada" 90 in let b = mk "bob" 70 in let rows = a :: b :: nil in fold rows 0 (λr:?. λacc:Num. acc + r.score * 2)` |
| `fill_row_join_seed` | fill | 1 | 1 | 0 | yes | 0 | yes | 1 | 0 | yes | `let mk = (λn:Str. λs:Num. {name = n, score = s}) in let a = mk "ada" 90 in let b = mk "bob" 70 in let rows = a :: b :: nil in fold rows "" (λr:?. λacc:Str. acc ++ r.name)` |
| `extend_row_name_comma` | extend | 2 | 2 | 0 | yes | 0 | yes | 1 | 0 | yes | `let mk = (λn:Str. λs:Num. {name = n, score = s}) in let a = mk "ada" 90 in let b = mk "bob" 70 in let rows = a :: b :: nil in fold rows "" (λr:?. λacc:Str. acc ++ r.name ++ ",")` |
| `extend_best_projection` | extend | 1 | 1 | 0 | yes | 0 | yes | 1 | 0 | yes | `let mk = (λn:Str. λs:Num. {name = n, score = s}) in let best = mk "ada" 90 in best.score` |
| `fill_greeting_formal_branch` | fill | 10 | 9 | 0 | no | 0 | yes | 1 | 0 | yes | `λwho:Str. λformal:Bool. if formal then "Dear " ++ who ++ "," else "hey " ++ who ++ "!"` |
| `fix_number_in_greeting` | fix | 1 | 1 | 0 | yes | 0 | yes | 1 | 0 | yes | `λwho:Str. λloud:Bool. if loud then "HELLO, " ++ who ++ "!" else "hello, " ++ who ++ "."` |
| `extend_name_with_last` | extend | 13 | 13 | 1 | yes | 0 | yes | 1 | 0 | yes | `λfirst:Str. λlast:Str. λshout:Bool. if shout then "NAME: " ++ first ++ " " ++ last else "name: " ++ first ++ " " ++ last` |
| `fill_command_second_test` | fill | 4 | 4 | 0 | yes | 0 | yes | 1 | 0 | yes | `λcmd:Str. λarg:Str. if cmd == "add" then arg ++ " added" else if cmd == "del" then arg ++ " deleted" else "unknown " ++ cmd` |
| `fill_settings_retries` | fill | 1 | 1 | 0 | yes | 0 | yes | 1 | 0 | yes | `let settings = {host = "localhost", port = 8080, retries = 3} in if 0 < settings.retries then settings.host else "no host"` |
| `fix_offline_branch` | fix | 1 | 1 | 0 | yes | 0 | yes | 1 | 0 | yes | `let settings = {host = "localhost", port = 8080, verbose = false} in if settings.verbose then settings.host else "offline"` |
| `extend_settings_with_retries` | extend | 7 | 7 | 0 | yes | 0 | yes | 1 | 0 | yes | `let settings = {host = "localhost", port = 8080, retries = 3} in settings.host` |
| `fix_score_else_branch` | fix | 7 | 7 | 0 | yes | 0 | yes | 1 | 0 | yes | `let row = {name = "ada", score = 90} in if 0 < row.score then row.name ++ " passed" else row.name ++ " failed"` |
| `fill_match_stopped_arm` | fill | 1 | 1 | 0 | yes | 0 | yes | 1 | 0 | yes | `λs:?. match s { Idle x0 -> "waiting" \| Running x1 -> "in flight" \| Stopped x2 -> "done" }` |
| `fix_match_busy_arm` | fix | 1 | 1 | 0 | yes | 0 | yes | 1 | 0 | yes | `λs:?. match s { Idle x0 -> "idle" \| Busy x1 -> "busy" \| Done x2 -> "done" }` |
| `extend_match_with_stopped_arm` | extend | 30 | 30 | 0 | no | 0 | yes | 1 | 0 | yes | `λs:?. match s { Idle x0 -> "idle" \| Running x1 -> "running" \| Stopped x2 -> "stopped" }` |
| `fill_filter_threshold` | fill | 4 | 4 | 0 | yes | 0 | yes | 1 | 0 | yes | `λxs:List Num. fold xs nil (λh:Num. λacc:List Num. if 10 < h then h :: acc else acc)` |
| `fix_filter_else_branch` | fix | 1 | 1 | 0 | yes | 0 | yes | 1 | 0 | yes | `λxs:List Num. fold xs nil (λh:Num. λacc:List Num. if 5 < h then h :: acc else acc)` |
| `fill_join_separator` | fill | 1 | 1 | 0 | yes | 0 | yes | 1 | 0 | yes | `let names = "ada" :: "bob" :: "cy" :: nil in fold names "" (λn:Str. λacc:Str. acc ++ n ++ " ")` |
| `fix_number_in_name_list` | fix | 1 | 1 | 0 | yes | 0 | yes | 1 | 0 | yes | `let names = "ada" :: "bob" :: "cy" :: nil in fold names "" (λn:Str. λacc:Str. acc ++ n)` |
| `fix_list_tail` | fix | 1 | 1 | 0 | yes | 0 | yes | 1 | 0 | yes | `let names = "ada" :: "bob" :: nil in fold names "" (λn:Str. λacc:Str. acc ++ n)` |
| `fix_string_seed_in_total` | fix | 1 | 1 | 0 | yes | 0 | yes | 1 | 0 | yes | `λxs:List Num. fold xs 0 (λh:Num. λacc:Num. acc + h)` |
| `extend_join_into_let` | extend | 26 | 25 | 1 | no | 0 | yes | 1 | 0 | yes | `let names = "ada" :: "bob" :: nil in let joined = fold names "" (λn:Str. λacc:Str. acc ++ n) in joined ++ "!"` |
| `extend_item_count` | extend | 2 | 2 | 0 | yes | 0 | yes | 1 | 0 | yes | `λtag:Str. λn:Num. {label = "item: " ++ tag, count = n + 1, ok = 0 < n}` |
| `build_exclaim` | build | 22 | 21 | 2 | no | 0 | yes | 1 | 0 | yes | `λs:Str. "hello, " ++ s ++ "!"` |
| `build_total_of_list` | build | 30 | 30 | 0 | no | 0 | no | 1 | 0 | yes | `λns:List Num. fold ns 0 (λh:Num. λacc:Num. acc + h)` |
| `build_point_record` | build | 25 | 25 | 0 | yes | 0 | yes | 1 | 0 | yes | `λa:Num. λb:Num. {x = a, y = b}` |
| `build_name_list` | build | 22 | 21 | 1 | no | 0 | yes | 1 | 0 | yes | `λfirst:Str. first :: "bob" :: "cy" :: nil` |
| `build_labelled_count` | build | 30 | 30 | 1 | no | 0 | yes | 1 | 0 | yes | `λtag:Str. λn:Num. {label = tag ++ "!", count = n + 1}` |
| `build_choose_greeting` | build | 30 | 30 | 1 | no | 0 | yes | 1 | 0 | yes | `λformal:Bool. if formal then "good evening" else "hi"` |
| `build_city_lookup` | build | 15 | 15 | 0 | yes | 0 | yes | 1 | 0 | yes | `let row = {city = "oslo", temp = 4} in row.city` |
| `build_status_word` | build | 22 | 21 | 2 | no | 0 | no | 1 | 0 | no | `λs:?. match s { Ok x0 -> "fine" \| Bad x1 -> "broken" }` |

### Reading this honestly

The claim this run supports is narrower than "the protocol wins", because the
protocol did not win. It is this: **at post-B2 scale a competent model still
does not need the protocol to stay well-typed in text, and showing the model
the cursor cuts the protocol's invalid-edit rate by about four-fifths without
closing the gap.** 11.4 % to 2.9 % on harder programs is the run's real
finding, and 0 % is still the number to beat.

Two things this run does *not* show. It does not show the baseline degrading
with scale — these programs are larger than the first set but still small
enough to write in one line, and the baseline wrote all 32 of them without a
single parse or type error. And it does not show the protocol's guarantee
paying for itself in tasks completed: it paid in 0 ill-typed intermediate
states out of 320, which is what it promises, while costing calls and target
hits.

The place the numbers do separate is what happens *when* a model is wrong. Nine
times the protocol refused an edit, charged one action, and changed nothing.
The baseline's wrong answers were accepted whole, because the unit of edit is
the whole program and a wrong-but-well-typed program is indistinguishable from
a right one at the point of acceptance. On these tasks that cost two targets.
On a program where the model rewrites four hundred nodes to change one, it is
a different bet — but that program has not been benchmarked here, and guessing
at its number would be fabricating it.

### Run metadata

| | |
| --- | --- |
| date | 2026-08-29 |
| model | `claude-haiku-4-5-20251001` |
| CLI | `claude` v2.1.251, headless (`-p`), prompt on stdin |
| task set | `--tasks post-b2` (32 tasks, 8 per family) |
| conditions | A=action protocol (interactive), B=text baseline (one shot), B2=text baseline (interactive) |
| A step cap | 30 actions per task |
| B2 turn cap | 5 whole-program rewrites per task |
| worker threads | 10 (std `thread::scope`; results ordered by index, not completion) |
| model calls | 385 (320 A + 32 B + 33 B2) |
| failed calls | 0 |
| retries used | 0 |
| wall time | 33 min 43 s (2023.1 s) |
| transcript | `bench/agent-transcripts/post-b2-invalid-edit-rate.jsonl` (98 records: 1 run header, 96 task records, 1 summary; every record carries the full prompt and reply for every call) |
| harness | `agentapi/src/bin/agentbench.rs`, tasks in `agentapi/src/measure/tasks.rs`, prompt legends in `agentapi/src/measure/legend.rs` |
| reproduce | `cargo run --release -p nothing-agentapi --bin agentbench -- --tasks post-b2 --workers 10` |
