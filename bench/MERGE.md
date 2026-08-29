# Structural merge versus `git merge-file`

Measured 2026-08-29.

Every number below was produced by running the harness, not by hand. Reproduce it with:

```
cargo run -p nothing-merge --bin merge-bench
```

The harness builds each scenario as three program versions — a common ancestor and two branches — and then merges them twice.

* **Line-based**: each version is rendered to a text file through the multi-line projection in `merge/src/text.rs`, and the three files are handed to the real `git merge-file -p --diff3 ours base theirs`. Its exit status is the conflict count.
* **Structural**: the same three versions, as `(Exp, NameTable)` pairs, go through `nothing_merge::merge`, which diffs each branch against the ancestor into typed operations and replays the non-overlapping ones.

A line-based merge can also succeed for the wrong reason, so the harness compares `git merge-file`'s output against the structural result with whitespace normalised. A run that git reports as clean but whose text disagrees is recorded as **clean but wrong** — that is the failure mode that costs the most, because nothing reports it.

## The operation vocabulary

A diff is a list of these, never a list of lines. Each one carries a path into the tree (or, for `Rename`, a binder identity) and enough payload to be replayed on any tree that still has that shape.

| operation | means | footprint |
| --- | --- | --- |
| `Rename` | a binder's display name changed | that binder's name, nothing structural |
| `Fill` | an empty hole was filled | the node at the path |
| `DeleteToHole` | a subterm was deleted, leaving a gap | the node at the path |
| `Insert` | a subterm was wrapped in a new parent | the node at the path |
| `Delete` | a wrapper was removed and one child promoted | the node at the path |
| `Move` | a subtree with an unchanged content hash appears at a new path | both endpoints |
| `MoveBinding` | a `let` binding changed position in its chain | the chain's ordering only |
| `ReorderFields` | a record's fields were permuted | that record's field order only |
| `Replace` | a subterm became a structurally different one | the node at the path |
| `SetAnn` | a lambda's parameter annotation changed | that node's shape, not its body |
| `Rebind` | binder identities changed, structure did not | the node at the path |

Two operations conflict when their footprints overlap. Two node footprints overlap when one path is a prefix of the other — siblings never collide, which is why two branches editing different fields of the same pair merge with no conflict at all. A shape footprint covers a node but not its children, so retyping a parameter does not fight an edit in the body. A name footprint is a binder identity and touches no part of the tree. An ordering footprint covers a `let` chain's spine but not the expressions bound in it, so reordering bindings does not fight an edit inside one of them. A record's field order is the same kind of footprint over a different list: it covers the order of the fields and neither their values nor their names, which is why one branch can rename a field while the other moves it.

`Move` gets a further rule: an edit made inside a subtree that the other branch moved is *rebased* onto the subtree's new path instead of being called a conflict. The two operations do commute; they just need the path rewritten first.

A merge can still land somewhere ill-typed even when every accepted operation was non-overlapping — one branch retypes a parameter while the other adds a call site is the easy example. Rather than emit a broken tree or refuse a merge nobody asked to refuse, the merge repairs it the way the language repairs everything else: the offending subterm is wrapped in a non-empty hole, which keeps it visible and keeps the whole program well-typed. Unbound variables are the one case that cannot be quarantined — a non-empty hole around an unbound variable does not synthesise either — so those become empty holes. Both kinds of repair are reported, never silent.

## Totals

| | scenarios | clean | clean and correct | conflicts |
| --- | ---: | ---: | ---: | ---: |
| `git merge-file` on rendered text | 21 | 2 | 2 | 19 |
| structural merge on typed operations | 21 | 18 | 18 | 3 |

Every structural merge result is well-typed: 21/21.

## By scenario class

| class | scenarios | git clean and correct | git clean but wrong | git conflicts | structural clean |
| --- | ---: | ---: | ---: | ---: | ---: |
| reordering | 4 | 0 | 0 | 4 | 4 |
| renaming | 6 | 0 | 0 | 6 | 6 |
| reformatting | 4 | 0 | 0 | 4 | 4 |
| moving | 3 | 1 | 0 | 2 | 3 |
| control | 4 | 1 | 0 | 3 | 1 |

## Every scenario

| class | scenario | ops (ours / theirs) | `git merge-file` | structural |
| --- | --- | --- | --- | --- |
| reordering | swap adjacent bindings vs edit inside one of them | 1 / 1 | conflict | clean |
| reordering | move a binding past another vs edit the one it passes | 1 / 1 | conflict | clean |
| reordering | reverse the whole chain vs edit the last binding | 2 / 1 | conflict | clean |
| renaming | rename a parameter vs edit the line that uses it | 1 / 1 | conflict | clean |
| renaming | rename a function vs reorder the chain | 1 / 1 | conflict | clean |
| renaming | two branches rename two different functions | 1 / 1 | conflict | clean |
| control | two branches rename the same function differently | 1 / 1 | conflict | 1 conflict(s) |
| reformatting | reindent the whole file vs edit one literal | 0 / 1 | conflict | clean |
| reformatting | reindent the whole file vs rename a function | 0 / 1 | conflict | clean |
| reformatting | blank-line style vs reorder the chain | 0 / 1 | conflict | clean |
| reformatting | reindent both branches, one of which also edits | 0 / 1 | conflict | clean |
| moving | move a function into a hole vs edit inside the moved function | 1 / 1 | conflict | clean |
| moving | move a function into a hole vs edit a binding above it | 1 / 1 | clean | clean |
| control | both branches move the same function to different places | 1 / 1 | conflict | 1 conflict(s) |
| control | both branches change the same literal to different values | 1 / 1 | conflict | 1 conflict(s) |
| renaming | rename the greeted parameter vs reword the greeting itself | 1 / 2 | conflict | clean |
| reordering | one branch appends to a list, the other inserts into its middle | 1 / 1 | conflict | clean |
| renaming | two branches rename and reorder the fields of the same record | 1 / 2 | conflict | clean |
| moving | two branches edit two different arms of the same match | 1 / 1 | conflict | clean |
| renaming | one branch renames a constructor while the other edits an arm | 1 / 1 | conflict | clean |
| control | both branches make the identical edit | 1 / 1 | clean | clean |

## What the line-based merge is given

The projection is real multi-line code, not one long line — otherwise the comparison would be rigged, because every edit would touch the only line there is. This is the ancestor of the first scenario, exactly as it is written to the file handed to `git merge-file`:

```
let square =
  λa:Num.
    a * a
in
let bump =
  λb:Num.
    b + 1
in
let drop2 =
  λc:Num.
    c - 2
in
square 3 + bump 4
```

Inspect any scenario's three inputs, its typed operations and its structural result with:

```
cargo run -p nothing-merge --bin merge-bench -- --dump 0
```

## What a conflict says

A conflict is two operations whose footprints overlap and which therefore cannot commute. The report is written in terms of the program, not of lines. This is the verbatim output for *two branches rename the same function differently*:

```
conflict (competing renames of one binder) at the name of `square`
  why:    one branch renames `square` to `sq`; the other renames `square` to `pow2`. Those two edits touch the same nodes and do not commute, so neither can be replayed on top of the other.
  base:   square
  ours:   sq
  theirs: pow2
```

## What each scenario does

* **swap adjacent bindings vs edit inside one of them** — one branch reorders `square` and `bump`; the other changes the body of `square`
* **move a binding past another vs edit the one it passes** — one branch moves `drop2` up one slot; the other changes the body of `bump`
* **reverse the whole chain vs edit the last binding** — one branch reverses all three bindings; the other changes the body of `drop2`
* **rename a parameter vs edit the line that uses it** — one branch renames the parameter `a`; the other changes the expression `a * a`
* **rename a function vs reorder the chain** — one branch renames `square` to `sq`; the other moves `drop2` to the front
* **two branches rename two different functions** — both renames land on the call line `square 3 + bump 4`
* **two branches rename the same function differently** — a genuine conflict: both branches claim the display name of `square`
* **reindent the whole file vs edit one literal** — one branch reprints at four-space indent; the other changes `b + 1` to `b + 5`
* **reindent the whole file vs rename a function** — one branch reprints at four-space indent; the other renames `bump`
* **blank-line style vs reorder the chain** — one branch adds a blank line after every binding; the other reorders bindings
* **reindent both branches, one of which also edits** — both branches reprint; only one changes a literal
* **move a function into a hole vs edit inside the moved function** — one branch moves the lambda to the other pair component; the other edits its body
* **move a function into a hole vs edit a binding above it** — one branch moves the lambda across the pair; the other edits `bump`
* **both branches move the same function to different places** — a genuine conflict: the same subtree is claimed by two destinations
* **both branches change the same literal to different values** — a genuine conflict: line-based and structural merge should both refuse
* **rename the greeted parameter vs reword the greeting itself** — one branch renames `x` to `who`; the other rewrites the string literals around it
* **one branch appends to a list, the other inserts into its middle** — a cons chain is a spine like a let chain, but its cells have no ids: `1 :: 2 :: 3 :: nil` gains a 4 at the end on one side and a 9 after the 1 on the other
* **two branches rename and reorder the fields of the same record** — one branch renames the field `width`; the other renames `depth` and moves it to the front of the same record. A field is an identity, so a rename is a name-table write with no structural footprint and the reorder is an ordering footprint over the field list — the three edits touch one line of text and three disjoint regions of the tree
* **two branches edit two different arms of the same match** — the two arms of one match are disjoint subtrees with the same parent: one branch rewrites the `Open` arm and the other rewrites the `Shut` arm, which `git merge-file` sees as two edits inside one printed line
* **one branch renames a constructor while the other edits an arm** — a constructor is an identity, so renaming `Open` is a name-table write with no structural footprint at all, and the arm edit is a region the rename cannot overlap
* **both branches make the identical edit** — convergent edits: both merges should be clean

## Reading the table

The control class exists to keep the comparison honest. Two branches that rename the same binder differently, that move the same subtree to two different destinations, or that set the same literal to two different values are *real* disagreements; a merge engine that reports them as clean is broken, not clever. Structural merge conflicts on those, and it says which node and which two alternatives are in play.

The other four classes are the cases the line-based algorithm cannot see. Reordering, renaming and reformatting are all edits that leave the program's meaning untouched but rewrite the lines a text merge is reasoning about; moving is the case where the same subtree exists on both sides and only its address changed. In each of those, one branch changes the text everywhere and the other changes the program somewhere, and the line merge has no way to tell that those are different kinds of change.

Two structural facts do the work. Names live in a `NameTable` keyed by binder identity, so a rename is one `Rename` operation whose footprint is a name, not a region of the tree — it cannot collide with a structural edit. And subtree identity is the Phase 7 content hash, so a subtree that turns up at a new path with an unchanged hash is a `Move`, and an edit made inside that subtree on the other branch is *rebased* onto the new path rather than being declared a conflict.
