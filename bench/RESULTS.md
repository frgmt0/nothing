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
