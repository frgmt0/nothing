# Keystroke benchmark results

Each entry is dated. Entries are appended, never edited: the point of this
file is the trend line, and a trend line you are allowed to retouch is not
evidence of anything.

The denominator is always the permanent Neovim baseline fixed in
`bench/references.md` (84 / 114 / 65 / 151 / 146). It is never recomputed.

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
