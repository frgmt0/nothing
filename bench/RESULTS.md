# Keystroke benchmark results

Each entry is dated. Entries are appended, never edited: the point of this
file is the trend line, and a trend line you are allowed to retouch is not
evidence of anything.

The denominator is always the permanent Neovim baseline fixed in
`bench/references.md` (84 / 114 / 65 / 151 / 146, and 127 for the sixth
reference added on 2026-08-29). It is never recomputed.

---

## 2026-08-26 — Phase 3, first run (pre-keybinding)

Reproduce with:

```
cargo run -p nothing-bench -- table
```

Counts come from replaying `bench/fixtures/<name>.actions` through the real
action calculus (`nothing_action::script::replay_script`). Nothing here is
hand-counted; the fixtures are also asserted to replay to a well-typed
program matching a committed rendering (`bench/fixtures/<name>.expected`).

| # | Program | Neovim keystrokes | `nothing` actions | Ratio |
|---|---------|------------------:|------------------:|------:|
| 1 | factorial * | 84 | 16 | 0.19x |
| 2 | list_map * | 114 | 22 | 0.19x |
| 3 | record * | 65 | 30 | 0.46x |
| 4 | state_machine * | 151 | 24 | 0.16x |
| 5 | nested_conditional | 146 | 35 | 0.24x |

`*` — the fixture builds an *approximation* of the reference program, not
the reference program. See below, and `bench/references.md` §"Mapping the
five references onto the Phase-1 surface" for the exact substitutions.

### These numbers are pre-keybinding, and they do not mean what they look like

The spec predicted this first run would be terrible because it counts
verbose action names rather than keystrokes. It came out the other way, and
the reason is not that the editor is good — there is no editor yet. Three
things make this table optimistic, and all three have to be said out loud
before the 3× guard from Phase 0 is applied to anything.

**1. An action is not a keystroke.** The denominator is one keypress per
character in Neovim. The numerator is one *action* — `construct-if` builds
an entire `if ⦇⦈ then ⦇⦈ else ⦇⦈` skeleton, `set-ann Num -> Num` writes a
whole type. Phase 4 maps single keypresses onto actions, and only then is
the ratio measuring comparable units. Until `KEYS.md` exists, the honest
reading of this table is "how many tree operations does the program cost",
not "how many keys does the user press". `set-ann Num -> Num` will not be
one keystroke.

**2. Four of the five fixtures build smaller programs than the reference.**
Phase 1 has no recursion, no lists, no pattern matching, no records and no
sum types, and the spec forbids adding them early. So factorial's recursive
call is an empty hole, `map` is map-over-a-pair, the record is a positional
product, and the state machine's `match` is a chain of `if`s. Those
approximations are genuinely less program than the pseudocode they stand in
for, while the denominator still charges Neovim for typing the full
reference. Only #5 (nested_conditional) is a like-for-like comparison — and
notably it is *not* the best ratio, which is the number to trust.

**3. Nothing here is an interactive session.** The fixtures were recorded by
building each program once, correctly, with no exploration, no mistakes and
no backtracking. Real editing includes all three. The keystroke ratio that
decides whether the action grammar is wrong is the one measured in Phase 4
from actual use, not this one.

### What the composition says, which is more useful than the ratio

| Program | Total | Movement | Construction | Metadata (`set-*`) |
|---------|------:|---------:|-------------:|-------------------:|
| factorial | 16 | 6 | 8 | 2 |
| list_map | 22 | 7 | 11 | 4 |
| record | 30 | 14 | 9 | 7 |
| state_machine | 24 | 10 | 12 | 2 |
| nested_conditional | 35 | 16 | 17 | 2 |

Construction is 43% of all actions and movement is 39%. That is the real
finding of this run: **cursor movement costs about as much as building the
program**, and almost all of it is the `move-parent` / `move-next-sibling`
walk back up and across after finishing a subtree. `record` is the worst
case (14 of 30 actions are movement, including a run of four consecutive
`move-parent`s to climb out of a nested lambda) and it is no coincidence
that it also has the worst ratio.

Two candidate fixes for Phase 4's `KEYS.md` to consider, written down now so
the design is answering evidence rather than taste:

- an "ascend to the next unfilled hole" motion, which would collapse most of
  the `move-parent`+`move-next-sibling` pairs into one key;
- automatic advance to the next empty hole after a construction completes a
  subtree, making the common case zero movement keys.

The metadata column is the other visible cost: every binder needs
`set-binder-id` (an artifact of this harness — the REPL has to pin binder
identities explicitly because the fresh-id supply would otherwise choose
them) plus `set-ann` for its type. Phase 5 turns naming into a name-table
write and Phase 4 should fold annotation into the binder-entry path; both
should take this column down.

### Guard status

Phase 0's guard — "if after four weeks the keystroke ratio versus Neovim
exceeds 3×, the action grammar is wrong" — is **not yet in force**. It is
stated in keystrokes, and there are no keystrokes yet. The first number it
applies to is the Phase 4 re-run, after `KEYS.md` and the TUI exist. This
entry is the baseline that run will be compared against.

---

## 2026-08-27 — Phase 4, keystroke re-run

Reproduce with:

```
cargo run -p nothing-bench -- keytable
```

which reads `tui/tests/keys/<name>.keys` — one keystroke per line, the
`nothing-tui` format documented in `tui/src/keyscript.rs` — and divides the
countable-line count by the permanent Neovim baseline. Nothing here is
hand-counted: the same fixtures are driven through the real, pure key
handler (`nothing_tui::keys::handle_key`, no REPL, no hand-built `Exp`) by
`tui/tests/references.rs`, which asserts the resulting program is
structurally identical (up to hole *identity*) to what `bench/fixtures/
<name>.actions` builds through the Phase 3 action calculus, byte-identical
to the committed `.expected` rendering, and well-typed. That test's own
tripwire (`no_reference_program_exceeds_the_three_times_guard`) and
`nothing-bench`'s (`no_keystroke_ratio_exceeds_the_three_times_guard`) both
assert the guard below in code, so a regression fails `cargo test
--workspace`, not just this table.

This is the number the Phase 0 guard was actually stated in terms of. The
2026-08-26 entry above measured *actions*, which the entry itself said was
not yet a comparable unit; this entry measures *keystrokes* against the
same five committed `.keys` fixtures the TUI's own acceptance test uses.

| # | Program | Neovim keystrokes | `nothing` keystrokes | `nothing` actions | Ratio | Guard |
|---|---------|------------------:|----------------------:|-------------------:|------:|:-----:|
| 1 | factorial * | 84 | 16 | 19 | 0.19x | OK |
| 2 | list_map * | 114 | 29 | 33 | 0.25x | OK |
| 3 | record * | 65 | 33 | 37 | 0.51x | OK |
| 4 | state_machine * | 151 | 24 | 32 | 0.16x | OK |
| 5 | nested_conditional | 146 | 31 | 41 | 0.21x | OK |

`*` — same caveat as the 2026-08-26 entry: the fixture builds an
*approximation* of the reference program forced by the Phase-1 surface (no
recursion, lists, records, or `match`); only #5 is a like-for-like
comparison, and it is the one to weight most heavily. `nothing` actions is
`AppState::actions().len()` after replaying the `.keys` file — the primitive
action-log length the keyboard grammar expanded into, printed for
comparison with the pre-keybinding table above (note it is *not* the same
number as the pre-keybinding `bench/fixtures/*.actions` counts, e.g.
factorial 19 here vs. 16 there: the two routes mint and order actions
differently, in particular around binder-id/annotation assignment, even
though they build the same program).

### Guard status: PASS, by a wide margin

**Every ratio is comfortably under the 3× guard from Phase 0** — the worst
case (`record`, 0.51×) is barely a sixth of the limit, and the best
(`state_machine`, 0.16×) is a sixth of *that*. No stop, no grammar rework.
The keyboard grammar in `KEYS.md` (modeless, binder slots, commit-live
literal entry, operator climbing) is validated against real use of the
actual `nothing-tui` key handler, not a prediction: this table is what
`tui/tests/references.rs` measures on every `cargo test --workspace` run,
so it cannot silently drift stale the way a hand-computed number could.

### Compared with the pre-keybinding (action-count) table

| Program | 2026-08-26 ratio (actions) | 2026-08-27 ratio (keystrokes) | Change |
|---|---:|---:|---|
| factorial | 0.19x | 0.19x | unchanged |
| list_map | 0.19x | 0.25x | up (0.19x was actions=22; keystrokes=29 costs more per action here — SetAnn/SetBinderId collapse less than construction expands) |
| record | 0.46x | 0.51x | up slightly |
| state_machine | 0.16x | 0.16x | unchanged |
| nested_conditional | 0.24x | 0.21x | down |

The two tables are not directly comparable — one counts tree operations
(some of which, like `ConstructIf`, are many keystrokes' worth of one-shot
scaffolding in the Phase 3 REPL syntax but only one keypress at the
keyboard, e.g. `?`), the other counts keys — and the pre-keybinding entry
said so explicitly. What is worth noting: the keystroke ratios land in the
same rough band as the action-count ratios, which means the Phase 4
grammar's costliest moves (binder-name entry, annotation entry, operator
climbing, Tab-to-next-hole) did not introduce a keystroke tax anywhere near
what a naive one-key-per-primitive-action mapping would have cost. The
metadata column the 2026-08-26 entry flagged as the biggest remaining cost
(`set-binder-id` + `set-ann` per binder) is exactly the column KEYS.md's
binder slots collapsed: `\ x 0 : n .` is five keystrokes for a whole
`λx0:Num.` skeleton that cost three separate actions (`ConstructLam`,
`SetBinderId`, `SetAnn`) in the pre-keybinding fixtures, plus the movement
to reach the annotation.

### What is still open, honestly

This run is still not the number Phase 4's next checkbox asks for. It is
"type it once, correctly, with no exploration, no mistakes, no
backtracking" — the same caveat #3 from the 2026-08-26 entry, carried
forward unchanged, because building the `.keys` fixtures was still an
exercise in constructing a known program with the right keys, not an actual
editing session. The next checkbox ("use the editor to build something
real, for at least two hours, without fixing it") is the first time this
number will reflect real use, including backtracking, undo, and mistakes —
and it may come back worse than 0.51×. The guard is not "satisfied
forever"; it is "not breached by this measurement."

---

## 2026-08-27 — Phase 4, post-fix re-run

Reproduce with:

```
cargo run -p nothing-bench -- keytable
cargo test -p nothing-tui --test references -- --nocapture
```

The re-run required by Phase 4's last checkbox ("fix the top five friction
points… **Done when** all five are resolved and the benchmark has not
regressed"). Five points from `FRICTION.md` were fixed — #7 (a binder rename
that would capture references is declined instead of silently applied), #3
(the program pane is a viewport that follows the cursor), #12 (`Tab`/`S-Tab`
walk quarantines as well as empty holes), #13 (the focus is a highlighted
span, so a wrapper is legibly different from its contents), #10 (`Enter`
finishes the quarantine the cursor is *inside*) — and the same five committed
`.keys` fixtures were replayed through the same pure key handler afterwards.

| # | Program | Neovim keystrokes | `nothing` keystrokes | `nothing` actions | Ratio | vs. 2026-08-27 pre-fix | Guard |
|---|---------|------------------:|----------------------:|-------------------:|------:|:----------------------:|:-----:|
| 1 | factorial * | 84 | 16 | 19 | 0.19x | unchanged | OK |
| 2 | list_map * | 114 | 29 | 33 | 0.25x | unchanged | OK |
| 3 | record * | 65 | 33 | 37 | 0.51x | unchanged | OK |
| 4 | state_machine * | 151 | 24 | 32 | 0.16x | unchanged | OK |
| 5 | nested_conditional | 146 | 31 | 41 | 0.21x | unchanged | OK |

`*` — same approximation caveat as both entries above.

### No regression, and no fixture changed

Every keystroke count, every primitive-action count and every ratio is
identical to the pre-fix run earlier the same day. **No `.keys` fixture was
edited**, which is the stronger statement: the five fixes changed no keystroke
*sequence* that any reference program uses, so this is a like-for-like
comparison rather than a re-recorded one. Two of the fixes could in principle
have moved a number and did not:

- `Tab`/`S-Tab` now stop on non-empty holes too, so a fixture that pressed
  `Tab` while a quarantine was on screen would have landed somewhere new.
  None does: all five build their programs without ever leaving a quarantine
  standing (`state_machine.keys` says so in its own header comment, and types
  its comparisons operator-first for exactly that reason).
- `Enter` finishes an enclosing quarantine. No fixture presses `Enter` at all.

### The fixes that save keystrokes are not visible in this table, and that is the honest reading

The frictions these five fixes address cost keystrokes *in an editing
session*, not while typing a known program once, correctly, from the top —
which is still all these fixtures do (the caveat carried forward from
2026-08-26 §3 and 2026-08-27 §"What is still open"). The measurable saving is
pinned as a test instead of as a ratio:
`keys::tests::enter_finishes_the_quarantine_the_cursor_is_inside` asserts that
repairing a quarantine from where the repairing keystroke leaves the cursor is
**exactly one keystroke cheaper** than walking out to the wrapper first, on
the same program. The dogfooding session spent about 16 keystrokes on the
wrong turn of #13 and three navigation keystrokes per quarantine repair on
#10; neither cost appears in a fixture, because a fixture never takes a wrong
turn.

### Guard status: PASS, unchanged

Worst case is still `record` at **0.51×**, a sixth of the 3× guard from Phase
0; best is still `state_machine` at 0.16×. The guard is asserted in code by
`tui/tests/references.rs::no_reference_program_exceeds_the_three_times_guard`
and `nothing-bench`'s `no_keystroke_ratio_exceeds_the_three_times_guard`, both
of which pass, so a regression fails `cargo test --workspace` rather than
waiting for someone to re-read this file.

To be explicit, because the instruction that prompted this run said otherwise:
**the guard was not breached before these fixes and is not breached after
them.** Every ratio in the 2026-08-27 pre-fix table was already between 0.16×
and 0.51×; nothing in this project's recorded measurements has ever exceeded
1×, let alone 3×.

---

## 2026-08-28 — Phase B1, definition-era re-run

Reproduce with:

```
cargo run -p nothing-bench -- keytable
cargo run -p nothing-bench -- table
cargo test -p nothing-tui --test references -- --nocapture
```

The re-run required by Phase B1's fourth checkbox. Programs are no longer a
single expression: a program is now a *document* of named, annotated
top-level definitions, and a definition's body may call any definition in
the document by id — including itself. Two of the five reference fixtures
were rebuilt to say what they always meant, and three were left alone.

- **factorial** is now the real reference program. The 2026-08-26 and
  2026-08-27 entries both carried the caveat that "factorial's recursive
  call is an empty hole" because Phase 1 had no recursion. It is no longer a
  hole: `main : Num -> Num = λx0:Num. if x0 == 0 then 1 else x0 * main (x0 - 1)`,
  where `main` is the definition being written, resolved by id. No
  Z-combinator, no approximation.
- **record** is now two definitions, `main` and `mk`, instead of one
  expression smuggling the constructor through a `let`.
- **list_map**, **state_machine** and **nested_conditional** are unchanged
  programs; they gained the document header (`main : ? = …`) and nothing
  else, because a second definition would have been decoration.

| # | Program | Neovim keystrokes | `nothing` keystrokes | `nothing` actions | Ratio | vs. 2026-08-27 | Guard |
|---|---------|------------------:|----------------------:|-------------------:|------:|:--------------:|:-----:|
| 1 | factorial | 84 | 28 | 33 | 0.33x | 0.19x → 0.33x | OK |
| 2 | list_map * | 114 | 29 | 35 | 0.25x | unchanged | OK |
| 3 | record | 65 | 46 | 40 | 0.71x | 0.51x → 0.71x | OK |
| 4 | state_machine * | 151 | 24 | 33 | 0.16x | unchanged | OK |
| 5 | nested_conditional | 146 | 31 | 42 | 0.21x | unchanged | OK |

`*` — the approximation caveat now applies to **two** fixtures, not four.
`list_map` is still map-over-a-pair (no lists) and `state_machine` is still
a chain of `if`s (no `match`). `factorial`, `record` and
`nested_conditional` are like-for-like comparisons against the reference
pseudocode; `factorial` and `record` only became like-for-like in this run.

### Two ratios went up, and that is the entry's whole point

**These are worse numbers for a better editor, and the increase is the
honest price of the approximations the earlier entries flagged.**

`factorial` went 16 → 28 keystrokes. Twelve of those keystrokes buy the
recursive call `main (x0 - 1)` that the previous fixture left as `⦇⦈` while
still charging Neovim all 84 keystrokes for typing the whole function. The
old 0.19× was measuring a program with a hole in it against a program
without one. 0.33× is the first factorial ratio in this file that compares
two complete programs.

`record` went 33 → 46 keystrokes. The extra thirteen are a second
definition's name, its `Num -> Num -> Num * Num` annotation and the
navigation between them. The old fixture bound the constructor with `let`,
which is one keystroke of syntax and no annotation; the new one gives it a
name and a type at the top level, which is what "a two-field record
constructor plus accessor" actually is. Note the *action* count moved the
other way, 30 → 25 (see the table below): the definition is fewer tree
operations and more keystrokes, because typing a type is many keys and one
action.

The three unchanged fixtures are the control: their keystroke counts,
action counts and ratios are byte-identical to 2026-08-27, so nothing in
the definition work taxed the existing grammar. The new bindings (`C-n`,
`C-d`, `C-l`, `C-t`, `C-↑`, `C-↓`) cost zero keystrokes in a program that
does not use them.

### Action counts, for comparison with the pre-keybinding table

```
cargo run -p nothing-bench -- table
```

| # | Program | Neovim | actions | Ratio | 2026-08-26 actions |
|---|---------|-------:|--------:|------:|-------------------:|
| 1 | factorial | 84 | 23 | 0.27x | 16 |
| 2 | list_map * | 114 | 22 | 0.19x | 22 |
| 3 | record | 65 | 25 | 0.38x | 30 |
| 4 | state_machine * | 151 | 24 | 0.16x | 24 |
| 5 | nested_conditional | 146 | 35 | 0.24x | 35 |

Composition of the rebuilt fixtures (`set-def-ann` and `rename` of a
definition are counted as metadata, as `set-ann`/`set-binder-id` were):

| Program | Total | Movement | Construction | Metadata |
|---------|------:|---------:|-------------:|---------:|
| factorial | 23 | 7 | 13 | 3 |
| record | 25 | 7 | 8 | 9 (+1 `create-definition`) |

`record`'s metadata column is now 9 of 25 actions — the two definition
names and the two definition types. That is the definition era's standing
cost and it is visible rather than hidden inside a `let`.

### Guard status: PASS

Worst case is `record` at **0.71×**, still under a quarter of the 3× guard
from Phase 0; best is `state_machine` at 0.16×. Both tripwires
(`tui/tests/references.rs::no_reference_program_exceeds_the_three_times_guard`
and `nothing-bench`'s `no_keystroke_ratio_exceeds_the_three_times_guard`)
pass, so the guard is still asserted in code on every `cargo test
--workspace` rather than by re-reading this file.

The trend to watch is that the two ratios that moved both moved *up*, and
both moved up because a fixture stopped approximating. If a later phase
removes the remaining two approximations (lists for `list_map`, `match` for
`state_machine`) those ratios should be expected to rise too. A ratio that
only ever falls in this file would be evidence that the fixtures are
getting easier, not that the editor is getting better.

### What is still open, unchanged

Still not an editing session: these fixtures type a known program once,
correctly, with no exploration, no mistakes and no backtracking — the
caveat carried forward from 2026-08-26 §3 and repeated in both 2026-08-27
entries. Nothing in this run changes that.

---

## 2026-08-29 — Phase B2, strings

Reproduce with:

```
cargo run -p nothing-bench -- keytable
cargo run -p nothing-bench -- table
cargo test -p nothing-tui --test references -- --nocapture
```

The re-run required by Phase B2. Strings added a sixth reference program
(`bench/references.md` §6, a greeting formatter) because the first five
contain no text at all and so could not measure the feature. Its Neovim
denominator, 127, was hand-counted by the same method as the other five and
is fixed forever from this entry onward.

The five existing fixtures were **not touched**. Their programs, keystroke
counts and action counts are byte-identical to 2026-08-28.

| # | Program | Neovim keystrokes | `nothing` keystrokes | `nothing` actions | Ratio | vs. 2026-08-28 | Guard |
|---|---------|------------------:|----------------------:|-------------------:|------:|:--------------:|:-----:|
| 1 | factorial | 84 | 28 | 33 | 0.33x | unchanged | OK |
| 2 | list_map * | 114 | 29 | 35 | 0.25x | unchanged | OK |
| 3 | record | 65 | 46 | 40 | 0.71x | unchanged | OK |
| 4 | state_machine * | 151 | 24 | 33 | 0.16x | unchanged | OK |
| 5 | nested_conditional | 146 | 31 | 42 | 0.21x | unchanged | OK |
| 6 | greeting * | 127 | 52 | 56 | 0.41x | new | OK |

`*` — `greeting` joins `list_map` and `state_machine` in the approximate
column, for one reason only: `nothing` has no multi-argument functions, so
`greet(name, formal)` is two nested lambdas. Both string literals, both
joins, the conditional and both parameter references are exact.

### 52 keystrokes, 33 of which are the text itself

The greeting fixture is the first reference whose cost is dominated by
*content* rather than structure:

| part of the program | keystrokes |
|---------------------|-----------:|
| the two lambdas (`\x0:s.` and `\x1:b.`) | 12 |
| the conditional and its condition (`?x1`) | 3 |
| the two `Tab`s to the branch holes | 2 |
| the four `&` joins and the two `x0` references | 8 |
| **the characters inside the four string literals** | **19** |
| the eight `"` that open and close them | 8 |

Nineteen of the 52 are characters that would have to be typed in any
editor, and eight more are the quotes that would also have to be typed. A
projectional editor has no leverage over the contents of a string, and this
is the entry that says so out loud: the ratio here (0.41×) is worse than
four of the five older references not because the grammar got worse but
because 27 of the 52 keystrokes are text, where the structural advantage is
exactly zero. The right way to read 0.41× is that the *structure* cost 25
keystrokes against a 127-keystroke Neovim baseline.

The action count (56) is higher than the keystroke count (52) for the first
time in this file, and for the same reason: every keystroke inside a string
run re-issues `ConstructStr` with the whole run so far (see `KEYS.md`, the
commit-live invariant — there is no uncommitted buffer anywhere in this
editor, strings included), so a 14-character literal is 14 actions. That is
the price of "every keystroke is a real edit to the real program", and it
is paid in the log, not at the keyboard.

### Action counts, for comparison with the earlier tables

```
cargo run -p nothing-bench -- table
```

These come from the hand-written `bench/fixtures/<name>.actions` scripts,
not from the keyboard, which is why `greeting` is 27 here and 56 above: the
script writes each string literal with a single `construct-str`, where the
keyboard re-issues one per character typed.

| # | Program | Neovim | actions | Ratio | 2026-08-28 actions |
|---|---------|-------:|--------:|------:|-------------------:|
| 1 | factorial | 84 | 23 | 0.27x | 23 |
| 2 | list_map * | 114 | 22 | 0.19x | 22 |
| 3 | record | 65 | 25 | 0.38x | 25 |
| 4 | state_machine * | 151 | 24 | 0.16x | 24 |
| 5 | nested_conditional | 146 | 35 | 0.24x | 35 |
| 6 | greeting * | 127 | 27 | 0.21x | new |

### Guard status: PASS

Worst case is still `record` at **0.71×**; `greeting` at 0.41× is the
second worst and is under a seventh of the 3× guard. Both tripwires
(`tui/tests/references.rs::no_reference_program_exceeds_the_three_times_guard`
and `nothing-bench`'s `no_keystroke_ratio_exceeds_the_three_times_guard`)
now cover six programs and pass.

---

## 2026-08-29 — Phase B2, lists (`List τ`, `nil`, `::`, `fold`)

Reproduce with:

```
cargo run -p nothing-bench -- keytable
cargo run -p nothing-bench -- table
```

The re-run required by Phase B2's list checkbox. One fixture changed:
**reference 2, `list_map`, is now a real list map**. Every other fixture
and every other keystroke is byte-identical to 2026-08-29's strings entry,
which is what makes this a controlled measurement of one feature.

### Reference 2 was upgraded, not re-measured

Since 2026-08-26 the `list_map` fixture has been map over a *pair*
(`λf:Num -> Num. λxs:Num * Num. (f (fst xs), f (snd xs))`), because Phase 1
had no lists, and `bench/references.md` §2 said so in a paragraph headed
*what is lost: arbitrary length, and with it the `match`/recursion that
makes the reference 114 keystrokes*. Lists exist now, so that paragraph
was cashed in. The fixture is

```
λx0:Num -> Num. λx1:List Num. fold x1 nil (λx2:Num. λx3:List Num. x0 x2 :: x3)
```

**The denominator did not move.** 114 was hand-counted once on 2026-08-26
by the method in `bench/references.md` — `i`, then every character of the
reference pseudocode including newlines, then `Esc`: 1 + 112 + 1 = 114 — and
that method was applied to the reference text, which has not changed. Only
the numerator moved, and it moved the wrong way on purpose:

| reference 2 | keystrokes | actions | ratio |
|---|---:|---:|---:|
| old fixture (map over a pair, 2026-08-28) | 29 | 35 | 0.25× |
| new fixture (map over a list, this entry) | 44 | 37 | 0.39× |

Fifteen more keystrokes and a ratio half again as large, for a program that
now does what reference 2 actually describes. The old 0.25× was charging
Neovim 114 keystrokes for a `match`-and-recurse function while charging
`nothing` for a two-element tuple; this entry stops doing that. Two
approximations remain and are marked `*`: no polymorphism (the map is
`List Num -> List Num`), and the eliminator is `fold` rather than `match`.

### Keystrokes

| # | Program | Neovim keystrokes | `nothing` keystrokes | `nothing` actions | Ratio | vs. 2026-08-29 (strings) | Guard |
|---|---------|------------------:|----------------------:|-------------------:|------:|:------------------------:|:-----:|
| 1 | factorial | 84 | 28 | 33 | 0.33x | unchanged | OK |
| 2 | list_map * | 114 | 44 | 53 | 0.39x | 0.25x → 0.39x | OK |
| 3 | record | 65 | 46 | 40 | 0.71x | unchanged | OK |
| 4 | state_machine * | 151 | 24 | 33 | 0.16x | unchanged | OK |
| 5 | nested_conditional | 146 | 31 | 42 | 0.21x | unchanged | OK |
| 6 | greeting * | 127 | 52 | 56 | 0.41x | unchanged | OK |

### Where the 44 keystrokes go

| part of the program | keystrokes |
|---------------------|-----------:|
| the four lambdas and their annotations | 25 |
| `/` and the list argument (`x1`) | 3 |
| `Tab` `n` — the seed, where `nil` is a candidate and not a key | 3 |
| `:` — the cons cell | 1 |
| `space` and the two variable references in the head (`x0`, `x2`) | 7 |
| `Tab` and the tail reference (`x3`) | 5 |

Twenty-five of the 44 are lambdas and their type annotations, which lists
did not make more expensive; the list feature itself costs seven keystrokes
in this program (`/`, `:`, `n`, and the four `[`/`n` characters of the two
`List Num` annotations). The three new bindings — `:` for cons, `/` for
fold, `[` for `List` inside an annotation slot — each cost exactly one
keystroke wherever they appear, and `nil` costs one because it is a
completion candidate rather than a key (`KEYS.md` item 18).

### Action counts, for comparison with the earlier tables

```
cargo run -p nothing-bench -- table
```

| # | Program | Neovim | actions | Ratio | 2026-08-29 actions |
|---|---------|-------:|--------:|------:|-------------------:|
| 1 | factorial | 84 | 23 | 0.27x | 23 |
| 2 | list_map * | 114 | 37 | 0.32x | 22 |
| 3 | record | 65 | 25 | 0.38x | 25 |
| 4 | state_machine * | 151 | 24 | 0.16x | 24 |
| 5 | nested_conditional | 146 | 35 | 0.24x | 35 |
| 6 | greeting * | 127 | 27 | 0.21x | 27 |

### Guard status: PASS

Worst case is still `record` at **0.71×**. `list_map` moved from the third
best ratio to the third worst and is still under a seventh of the 3× guard.
Both tripwires
(`tui/tests/references.rs::no_reference_program_exceeds_the_three_times_guard`
and `nothing-bench`'s `no_keystroke_ratio_exceeds_the_three_times_guard`)
cover six programs and pass.

---

## 2026-08-29 — Phase B2, records (`{x = e}`, `e.x`)

Reproduce with:

```
cargo run -p nothing-bench -- keytable
cargo run -p nothing-bench -- table
cargo test -p nothing-tui --test references -- --nocapture
```

The re-run required by Phase B2's record checkbox. One fixture changed:
**reference 3, `record`, is a record now** — a two-field record built in one
definition and projected by name in another. Every other fixture and every
other keystroke is byte-identical to the lists entry above, which is what
makes this a controlled measurement of one feature.

### Reference 3 was upgraded, not re-measured

Since 2026-08-26 the `record` fixture has been a positional **pair**, and
`bench/references.md` §3 said so in a paragraph headed *what is lost: the
field names — nothing in the program records that component 0 is called
`x`. This is the substitution that gives up the most.* Records exist now, so
that paragraph was cashed in. The fixture is

```
mk   : Num -> Num -> ? = λx1:Num. λx2:Num. {x = x1, y = x2}
main : ? -> Num        = λp:?. p.x
```

and the `x` in `main` is the same `Id` as the `x` in `mk`, in a different
definition. **The denominator did not move:** 65 was hand-counted once on
2026-08-26 from the reference text by the method in `bench/references.md`,
and the reference text has not changed. Only the numerator moved, and it
moved up:

| reference 3 | keystrokes | actions (keyboard) | actions (script) | ratio |
|---|---:|---:|---:|---:|
| old fixture (a positional pair, 2026-08-28) | 46 | 40 | 25 | 0.71× |
| new fixture (a real record, this entry) | 50 | 43 | 27 | 0.77× |

Four more keystrokes, three more logged actions, and a ratio six points
worse for a program that now says what reference 3 describes. The parts:

| part of the program | old | new | Δ |
|---|---:|---:|---:|
| definition names | 4 | 10 | +6 |
| definition annotations | 14 | 10 | −4 |
| lambdas and their annotations | 20 | 15 | −5 |
| the bodies | 8 | 13 | +5 |
| navigation (`up` out of the record) | 0 | 2 | +2 |

Two of those five point downward, and neither is a win. The annotations and
the lambda parameters got *shorter* because a record type is a list of field
identities and identities are not spelled (`DECISIONS.md`, 2026-08-29): the
pair fixture wrote `Num * Num` twice, this one writes `?` twice. **The new
program is cheaper to type in exactly the places where it says less about
itself**, and it would be dishonest to bank that as a saving. The three
upward parts are the feature itself — two field names, a projection that
names its field, a constructor that now needs a name of its own because it
is the document's first definition, and two keystrokes of cursor movement
out of the record so that `C-n` means "new definition" again rather than
"new field" (`KEYS.md` item 19(c)).

### Keystrokes

| # | Program | Neovim keystrokes | `nothing` keystrokes | `nothing` actions | Ratio | vs. 2026-08-29 (lists) | Guard |
|---|---------|------------------:|----------------------:|-------------------:|------:|:----------------------:|:-----:|
| 1 | factorial | 84 | 28 | 33 | 0.33x | unchanged | OK |
| 2 | list_map * | 114 | 44 | 53 | 0.39x | unchanged | OK |
| 3 | record * | 65 | 50 | 43 | 0.77x | 0.71x → 0.77x | OK |
| 4 | state_machine * | 151 | 24 | 33 | 0.16x | unchanged | OK |
| 5 | nested_conditional | 146 | 31 | 42 | 0.21x | unchanged | OK |
| 6 | greeting * | 127 | 52 | 56 | 0.41x | unchanged | OK |

`record` keeps its `*`: the reference declares a *nominal* type
(`type Point = …`) and `nothing` has no such declaration, so the two
annotated positions in the reference are holes here.

### Action counts, for comparison with the earlier tables

```
cargo run -p nothing-bench -- table
```

| # | Program | Neovim | actions | Ratio | 2026-08-29 (lists) actions |
|---|---------|-------:|--------:|------:|---------------------------:|
| 1 | factorial | 84 | 23 | 0.27x | 23 |
| 2 | list_map * | 114 | 37 | 0.32x | 37 |
| 3 | record * | 65 | 27 | 0.42x | 25 |
| 4 | state_machine * | 151 | 24 | 0.16x | 24 |
| 5 | nested_conditional | 146 | 35 | 0.24x | 35 |
| 6 | greeting * | 127 | 27 | 0.21x | 27 |

Two more script actions than the pair fixture, and they are not the record
machinery. The constructor went 17 → 18: its body is six actions
(`construct-record`, `rename-field`, `construct-var`, `add-field`,
`rename-field`, `construct-var`) where the pair's was four, and it saves one
because it is the document's first definition and no longer needs
`create-definition`. The accessor went 8 → 9: it *gains*
`create-definition` and `rename-def` for the same reason, and loses the
`set-ann Num * Num` it can no longer write. The projection itself,
`construct-field x`, costs exactly what `construct-proj l` cost.

### Guard status: PASS

Worst case is still `record`, now at **0.77×** — a quarter of the 3× guard
from Phase 0; best is `state_machine` at 0.16x. Both tripwires
(`tui/tests/references.rs::no_reference_program_exceeds_the_three_times_guard`
and `nothing-bench`'s `no_keystroke_ratio_exceeds_the_three_times_guard`)
cover six programs and pass.

Three of the six ratios have now risen across Phase B2 — `list_map` when
lists arrived, `record` when records did, and `factorial` back in the
definition era — every one of them because a fixture stopped approximating.
That is still the only kind of regression this file welcomes.

---

## 2026-08-29 — Phase B2, variants (`` `C e ``, `match e { … }`)

Reproduce with:

```
cargo run -p nothing-bench -- keytable
cargo run -p nothing-bench -- table
cargo test -p nothing-tui --test references -- --nocapture
```

The re-run required by Phase B2's variant checkbox, and the fourth and last
of the phase. One fixture changed: **reference 4, `state_machine`, is a real
variant and a real match now**. Every other fixture and every other keystroke
is byte-identical to the records entry above.

### Reference 4 was upgraded, and it is the one that most needed it

Since Phase 0 the `state_machine` fixture has been

```
λx0:Num. if x0 == 0 then 1 else if x0 == 1 then 2 else 0
```

— `Idle`/`Running`/`Stopped` as the codes 0/1/2, and the reference's `match`
as a chain of equality tests with a catch-all `else`. `bench/references.md`
§4 said what that cost: *exhaustiveness … also the distinction between a
state and any other number — `transition(7)` is well-typed here and was not
in the reference.* Variants exist now, so it is

```
main : ? = λs:?. match s { Idle x0 -> `Running {}
                         | Running x1 -> `Stopped {}
                         | Stopped x2 -> `Idle {} }
```

Half of that cost is paid and half is not. Exhaustiveness is paid in full —
a match with a missing arm is not a state this editor can hold. The
number/state distinction is not: the parameter is still annotated `?`,
because a variant type cannot be spelled in an annotation, so
`transition 7` still typechecks. What changed is what it does — it gets
stuck instead of falling through an `else` and answering `Idle`
(`eval/tests/references.rs::reference_four_state_machine_transitions`).

There is a sharper thing to say about this row than about references 2 and 3,
and it should be said first. **The 151 in the denominator was always the
count for the `match` version** — it was hand-counted on 2026-08-26 from the
reference text, which reads `match s with | Idle -> Running | …`. From Phase 0
until today the numerator was measuring a *different program* from the one the
denominator measured, and the ratio was flattering by exactly that difference.
Fixing it is not a regression in the editor; it is the removal of a
measurement error that had been sitting in row 4 for the whole project.

The denominator did not move. The numerator moved up:

| reference 4 | keystrokes | actions (keyboard) | actions (script) | ratio |
|---|---:|---:|---:|---:|
| old fixture (an `if`-chain on numeric codes, Phase 0 → 2026-08-29) | 24 | 33 | 24 | 0.16× |
| new fixture (a real variant and match, this entry) | 41 | 53 | 36 | 0.27× |

Seventeen more keystrokes, and the parts:

| part of the program | old | new | Δ |
|---|---:|---:|---:|
| the lambda and its parameter (`\x0:n.` vs `\s.`) | 6 | 3 | −3 |
| naming the scrutinee (`x0` in each test vs `s` once) | 4 | 1 | −3 |
| the case analysis (two `?` and two `=` vs `\|` and three `C-n`) | 4 | 4 | 0 |
| the state codes (`0`, `1`) vs the case *names* | 2 | 18 | +16 |
| the three results (`1`, `2`, `0` vs three injections) | 3 | 12 | +9 |
| navigation (`tab`) | 5 | 3 | −2 |

Three of the six point down and none of them is the feature paying for
itself: the case analysis costs exactly what it cost before (four keystrokes
either way — `\|` and three `C-n`s against two `?`s and two `=`s), the
parameter annotation is gone because a variant type is never spelled and `?`
is the default (`DECISIONS.md`, 2026-08-29), the scrutinee is named once
instead of twice because a `match` looks at one expression where a chain of
tests re-reads it, and two `tab`s went away with the branches they walked.

The +16 is the entry. `Idle`, `Running` and `Stopped` are eighteen characters
that the numeric encoding did not write down *anywhere* — that was the whole
of what "encode the states as 0, 1 and 2" was saving, and it was saving it by
not saying what the program meant. The +6 on the results is the same
transaction in miniature: `1` became `` `Running {} ``, four keystrokes
(`` ` ``, `R`, `{`, `C-d`) instead of one, and one of those four is a single
letter because the constructor completion ranks the expected variant's cases
first (`KEYS.md` item 20(d)).

### Keystrokes

| # | Program | Neovim keystrokes | `nothing` keystrokes | `nothing` actions | Ratio | vs. 2026-08-29 (records) | Guard |
|---|---------|------------------:|----------------------:|-------------------:|------:|:------------------------:|:-----:|
| 1 | factorial * | 84 | 28 | 33 | 0.33x | unchanged | OK |
| 2 | list_map * | 114 | 44 | 53 | 0.39x | unchanged | OK |
| 3 | record * | 65 | 50 | 43 | 0.77x | unchanged | OK |
| 4 | state_machine * | 151 | 41 | 53 | 0.27x | 0.16x → 0.27x | OK |
| 5 | nested_conditional | 146 | 31 | 42 | 0.21x | unchanged | OK |
| 6 | greeting * | 127 | 52 | 56 | 0.41x | unchanged | OK |

`state_machine` keeps its `*`, for two reasons now instead of one. The
reference declares a *nominal* type (`type State = Idle | Running | Stopped`)
and `nothing` has no such declaration, so the parameter is `?`; and a nullary
constructor carries `{}` rather than nothing at all, which is two characters
the reference does not write.

### Action counts, for comparison with the earlier tables

```
cargo run -p nothing-bench -- table
```

| # | Program | Neovim | actions | Ratio | 2026-08-29 (records) actions |
|---|---------|-------:|--------:|------:|-----------------------------:|
| 1 | factorial * | 84 | 23 | 0.27x | 23 |
| 2 | list_map * | 114 | 37 | 0.32x | 37 |
| 3 | record * | 65 | 27 | 0.42x | 27 |
| 4 | state_machine * | 151 | 36 | 0.24x | 24 |
| 5 | nested_conditional | 146 | 35 | 0.24x | 35 |
| 6 | greeting * | 127 | 27 | 0.21x | 27 |

Twelve more script actions. Three of them are `add-arm`, three are
`set-constructor`, six are the `construct-record`/`remove-field` pairs that
write the three `{}` payloads — and the script pays a cursor-movement tax the
keyboard does not, because `set-constructor` at the action level wants the
cursor on the injection while `` ` `` at the keyboard leaves it there
already. The keyboard fixture is 41 keystrokes to the script's 36 actions and
53 logged actions; the gap between 36 and 53 is the same gap every fixture
has had since the definition era, and it is navigation.

### Guard status: PASS

Worst case is still `record` at **0.77×**, a quarter of the 3× guard; best is
now `nested_conditional` at 0.21x, `state_machine` having given up the title
it held on a technicality. Both tripwires
(`tui/tests/references.rs::no_reference_program_exceeds_the_three_times_guard`
and `nothing-bench`'s `no_keystroke_ratio_exceeds_the_three_times_guard`)
cover six programs and pass.

Four of the six ratios have now risen across Phase B2 — `factorial` in the
definition era, `list_map` when lists arrived, `record` when records did, and
`state_machine` today — every one of them because a fixture stopped
approximating. Row 4 is the last one that was approximating in a way a
feature could fix, which is the closing note this phase gets: the benchmark
now measures the programs it says it measures.

### The merge benchmark, same day

```
cargo run -p nothing-merge --bin merge-bench
```

21 scenarios (19 before this feature). `git merge-file` clean 2, clean and
correct 2; structural clean 18, well-typed 21/21. The two new ones are
**two branches edit two different arms of the same match** (moving; git
conflicts, structural clean — the arms are disjoint subtrees with the same
parent) and **one branch renames a constructor while the other edits an arm**
(renaming; git conflicts, structural clean — a constructor is an identity, so
the rename has no structural footprint at all). Full table in
`bench/MERGE.md`, regenerated by the harness on the same day.
