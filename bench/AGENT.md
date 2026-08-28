# Invalid-edit rate: the action protocol versus a text baseline

Measured 2026-08-28. Model: **`claude-haiku-4-5-20251001`**, driven headless
through the `claude` CLI (v2.1.251) with the prompt piped on stdin:

```
claude -p --model claude-haiku-4-5-20251001
```

Every number in this file came out of one executed run of 64 real model calls —
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
