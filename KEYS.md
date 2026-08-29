# KEYS.md — the keyboard grammar

*Phase 4, written before any TUI code exists. Every binding maps onto the
Phase 2 `Action` enum in `action/src/act.rs`; nothing here requires a new
action. Everything here is a design commitment to be falsified by the
Phase 4 benchmark re-run and the two-hour friction session.*

---

## The model

**There are no modes.** A text editor needs modes to recover verbs because
its only context is a caret in a string. Here the cursor is on a node, and
the node already says what it is. The meaning of a printable character is a
function of **what the cursor is on**, never of a flag the user has to
remember setting.

Three facts of the Phase 2 calculus make this survivable:

1. **Constructions auto-wrap.** `ConstructBinOp` on a focused `e` gives
   `e op ⦇⦈`, with the focus in the principal position (child 0). An
   operator never needs a selection or a motion in front of it.
2. **Constructions auto-quarantine.** Type-inconsistent entry lands in a
   non-empty hole instead of being refused, so no key is ever disabled
   because it "wouldn't typecheck here".
3. **Every action either applies or leaves the program untouched.** A key
   can be optimistic; there is no damaged state to recover from.

Consequence: **letters and digits are spent entirely on literals and
names** — the 80% path — and verbs live on punctuation and arrows. `d`
must be free to start `double`, so no letter can be a verb, so there is no
Vim-style normal mode. What survives from Vim/Kakoune is the structural
object-verb idea with the object free: the cursor *is* the selection, so
every verb is one punctuation key.

### Slots, not modes

The one piece of positional state beyond the zipper is the **slot**: which
part of the focused node the cursor addresses. Binders have parts the
zipper has no child index for, and `SetAnn`/`Rename` are unreachable
without them.

| node | slot 0 | slot 1 | slot 2 |
|---|---|---|---|
| the **definition** | definition **name** | definition **annotation** | body |
| `Lam` | binder **name** | **annotation** | body |
| `Let` | binder **name** | bound expression | body |
| everything else | child 0 | child 1 | child 2 |

Since Phase B1 a program is a *document* of named top-level definitions, and
the definition itself is the outermost row of that table: `↑` off the root of
a body reaches the definition's name, `↓` from there its annotation, `↓`
again the body. The alphabets are the ones already defined — the definition
name slot is a binder-name slot (free text, `Rename`) and the definition
annotation slot is an annotation slot (`SetDefAnn`, same type grammar).
Nothing new to learn; two more rows of the same table.

Arrows move over this editor-level child list, so binder parts cost no new
bindings. A slot is not a mode: the cursor is visibly sitting on the thing
the keys will affect, and the status line always reads
`slot · expected type · candidates`.

The one piece of transient state is the **name run**: the identifier
characters typed since the cursor last moved. It is not hidden state,
because every keystroke of a run is **committed to the program
immediately** (see Literal entry); the buffer exists only so the next
character can re-derive what the last one built.

---

## The grammar, on one screen

Every **form** key wraps the focus into the new form's principal position
and leaves the cursor on the form's first empty child. On an empty hole,
"wrap" and "insert" coincide.

```
MOVEMENT                                   LITERALS & NAMES
  ↓      into child/slot 0   MoveChild(0)    0-9   number: append to a focused
  ↑      to parent           MoveParent            Num, else ConstructNum(d)
  →      next sibling/slot   MoveNextSib     ~     negate the focused Num
  ←      prev sibling/slot   MovePrevSib     a-zA-Z_  name run: live-filtered,
  Tab    next hole, either kind (wraps)            type-ranked, commit-live
  S-Tab  previous hole, either kind                (true/false are candidates)
                                             "    string run: open, and close
OPERATORS  (climb, then wrap)                     with " again. Inside it every
  +   add        e + ⦇⦈    ConstructBinOp        printable key is one character
  -   subtract   e - ⦇⦈         〃                of text; \" and \\ escape
  *   multiply   e * ⦇⦈         〃
  <   less than  e < ⦇⦈         〃         FORMS  (wrap the focus)
  =   equals     e == ⦇⦈        〃           space  apply       e ⦇⦈   ConstructAp
  &   join text  e ++ ⦇⦈        〃           \      lambda      λ⦇⦈:?. e   …Lam
                                             ?      if e then ⦇⦈ else ⦇⦈   …If
IN A BINDER-NAME SLOT                        ;      let ⦇⦈ = e in ⦇⦈       …Let
  a-zA-Z0-9_  name the binder  Rename        ,      pair        (e, ⦇⦈)   …Pair
  :   → annotation slot                      [  ]   fst e / snd e   ConstructProj
  =   → bound expression (let)               !      quarantine  ⦇e⦈   …NonEmptyHole
  .   → body
                                          HOLES & HISTORY
IN AN ANNOTATION SLOT (re-issues SetAnn)    Bksp  run: un-type one char ·
  n Num  b Bool  s Str  ? unknown                 empty hole: ascend · else Delete
  > arrow  * product  ( ) grouping          Del   focus → ⦇⦈           Delete
  Bksp drop tok  .  → body                  Enter Finish the ⦇e⦈ on or around
  Tab/Enter → next hole                           the cursor, else next hole
                                            Esc   end the run (no-op otherwise)
DEFINITIONS  (the document is a list of      C-z / C-r  undo / redo · C-q quit
  them; a body may call any of them)
  C-↓  next definition      MoveNextDef      C-l  → definition-name slot
  C-↑  previous definition  MovePrevDef      C-t  → definition-type slot
  C-n  new definition, cursor in its name    CreateDefinition
  C-d  drop this definition (never the last) DeleteDefinition
```

39 bindings. Deliberately unbound and held in reserve for Phase 6+:
`( ) { } / | ^ % $ # @ ' . >` outside the slots, and `` ` ``.

`&` rather than a doubled `+`: `++` cannot be two keystrokes, because the
first `+` would already have committed an addition, and a key whose meaning
depends on what the previous key built is the lookahead this grammar spends
nothing on. `&` is the join-text operator most non-programmers have already
met (spreadsheets, Basic), it is not an operator on numbers, so it never means
two things, and `+` stays arithmetic so `1 + 2` never has to be disambiguated.

---

## The printable-character matrix

Normative. Eight contexts cover every place a cursor can be; number entry
is not a context because it is a pure function of the focused node.

| | **A** empty hole `⦇⦈` | **B** written expr `e` | **C** focused `Num n` | **D** mid-name run | **E** annotation slot | **F** binder-name slot | **G** non-empty hole `⦇e⦈` | **H** inside a string `"s»«"` |
|---|---|---|---|---|---|---|---|---|
| **digit** `0-9` | `ConstructNum(d)` | replace: `ConstructNum(d)` | **append**: `n·10±d` | append to run, re-filter, re-commit | exit → body, reprocess | append to name | descend into `e`, then as its column | append the character |
| **letter / `_`** | start name run | start name run (commit replaces `e`) | start name run | append, re-filter, re-commit | `n`/`b`/`s` set base type (spelled `num`/`bool`/`str` also works — unknown letters inside a spelled name are swallowed); other letters exit → body, reprocess | append to name | descend, then as inner | append the character |
| **op** `+ - * < = &` | wrap: `⦇⦈ op ⦇⦈` | climb, then wrap | climb, then wrap | end run, then as B | `*` product; others exit → body, reprocess | `=` on a `let` → bound slot; others exit → body, reprocess | descend, then as inner | append the character |
| **form** `space \ ? ; ,` | insert the form | climb (`\ ? ; ,`), then wrap | climb, then wrap | end run, then as B | `?` = the unknown type; others exit → body, reprocess | exit → body, reprocess | descend, then as inner | append — except `\`, which arms the escape |
| **`"`** | `ConstructStr("")`, run opens | replace: `ConstructStr("")`, run opens — on a focused `Str`, **re-open** at its end | replace: `ConstructStr("")`, run opens | end run, then as B | exit → body, reprocess | exit → body, reprocess | descend, then as inner | **close the run**; armed, append `"` |
| **`[` `]`** | `fst ⦇⦈` / `snd ⦇⦈` | climb (app-level), then wrap | wrap: `fst ⦇n⦈` (quarantined) | end run, then as B | exit → body, reprocess | exit → body, reprocess | descend, then as inner | append the character |
| **`!`** | wrap the hole: `⦇⦇⦈⦈` | quarantine: `⦇e⦈` | `⦇n⦈` | end run, then as B | exit → body, reprocess | exit → body, reprocess | **acts on the wrapper**: `⦇⦇e⦈⦈` | append the character |
| **`~`** | no-op, hint | no-op unless `Num` | negate: `ConstructNum(-n)` | end run, then as B | exit → body, reprocess | no-op | descend, then as inner | append the character |
| **`:`** | no-op, hint "annotations live on binders" | focused `Lam`: → its annotation slot; else no-op + hint | no-op, hint | end run, then as B | no-op | → annotation slot (`Lam` only) | descend, then as inner | append the character |
| **`.`** | no-op, hint | no-op, hint | no-op, hint | end run, then as B | → body slot | → body slot | descend, then as inner | append the character |
| **anything else** | no-op, status-line hint | no-op, hint | no-op, hint | end run, then as B | exit → body, reprocess | exit → body, reprocess | descend, then as inner | printable: append; otherwise close the run, reprocess |

Five rules generalise the table:

- **Typing replaces the selection.** A leaf construction overwrites a
  written expression; `C-z` is one key. The one exception is a digit on a
  `Num`, which appends — editing `100` into `1000` must not require a
  delete first.
- **A non-empty hole is transparent to typing.** Anything typed at `⦇e⦈`
  is typed at `e`; you quarantined it to keep editing it. Only `!`
  (re-wrap), `Enter` (`Finish`), `Del`/`Bksp` (delete the wrapper) address
  the wrapper itself.
- **"Exit and reprocess" is never a refusal.** Slots have small alphabets;
  a character a slot does not understand means "I am done here", and the
  character gets its normal meaning one step out. No keystroke is ever
  spent purely on leaving anything.
- **"End run, then as B"** means the name run is already committed (see
  below), so the key simply acts on the committed focus. No keystroke is
  consumed by the run ending.
- **Column H is one rule, not eleven.** Inside a string every printable
  character is a character of the string; only `"` (close), `\` (arm the
  escape) and `Bksp` (un-type one character) mean anything else, and every
  non-printable key closes the run and is reprocessed. The column is spelled
  out row by row anyway because a matrix that had an exception in it would be
  worth reading, and this one does not.

---

## Literal entry — the 80% path

Designed first, because it is most of the keystrokes. The invariant is
**commit-live**: every keystroke produces a real action against the real
program, so there is never a moment where the render disagrees with the
AST, and there is never a confirm key.

**Numbers — no state at all.** A digit on a focused `Num(n)` re-issues
`ConstructNum(n·10 + d)` (`n·10 − d` for negative `n`); anywhere else it
issues `ConstructNum(d)`. `4` then `2` gives `42` because `42 = 4·10 + 2`,
not because a buffer was open — come back a week later and type `7`, you
get `427`, which is also the right answer to "extend this number". `Bksp`
on a `Num` drops the last digit (one digit left ⇒ `Delete`). `~` negates.
This is why `-` stays subtraction: if `-` negated, `1 - 2` would be
untypable.

**Strings — delimited, because nothing else can delimit them.** `"` issues
`ConstructStr("")` and opens the string run; every printable key after it
re-issues `ConstructStr(s + c)`, so the literal in the program is the buffer
and there is nothing else to keep. `Bksp` re-issues `ConstructStr(s[..−1])`
and `Delete`s at the empty string, exactly as a digit-drop does on a `Num`.
`"` again closes the run; so does `Enter`, `Esc`, an arrow, `Tab`, or any
other key with a binding of its own — those close it and are then reprocessed,
which is rule 3 applied one level in.

The run is delimited where the name run is not, and that is forced, not
chosen: the name run ends at the first character outside its alphabet, and a
string's alphabet is *every* character, so there is no such character to end
it with. That is the whole cost of strings in this grammar — one reserved key
spent on a delimiter — and it buys the property that space, `+`, `;` and `?`
are ordinary text inside a literal without a single escape.

Escapes are the delimiter's own consequence and stop there: `\"` for a quote
and `\\` for a backslash, which is exactly what the projection prints between
the quotes, so what you read back is what you type. `\` **arms** the escape
rather than committing one — the single keystroke in this document that does
not change the program, shown as a pending `\` in the focus position and
undone by `Bksp`. `\` followed by anything other than `"` or `\` appends the
backslash and reprocesses the character as text, so `\n` is the two characters
you can see and no keystroke is refused. The alternative — commit the
backslash immediately and let a following `"` retract it — keeps commit-live
at the price of making `"\\"` and `"\""` type differently from how they read,
and a literal you cannot copy off the screen is worse than one pending key.

**Names — the one real run.** A letter starts a run **anchored** at the
focus as it was. Candidates are the in-scope binder display names
(`ctx_at`) plus `true`/`false`, filtered by prefix and ranked:

1. exact match first;
2. then by **type consistency with `expected_ty_at(cursor)`** — exact
   match above consistent-via-`?` above inconsistent. This is the payoff
   of bidirectional typing: at a hole expecting `Num → Num`, a binder of
   that type outranks an unrelated `Bool`, and at a `Bool` hole, `false`
   outranks `f : Num → Num`;
3. then innermost scope, then shortest name.

After **each** keystroke the top-ranked candidate is committed
(`ConstructVar`/`ConstructBool`) from the anchor, replacing what the
previous keystroke committed. You are never wrong for more than one
keystroke and you never press Enter to accept. With one binder `n` in
scope, `n` is one keystroke; `true` at a `Bool` hole is one keystroke.

If nothing matches, the anchor is left alone and the buffer renders in
unresolved styling; this is the one place typing does not change the
program, and it is forced by the calculus — `ConstructVar` on an
out-of-scope id returns `None` because a free variable has no meaning to
quarantine. Pressing `\` or `;` next uses the buffer as the new binder's
name, which is what you meant.

**Types.** The annotation slot re-issues `SetAnn` with the whole token
buffer on every keystroke, parsed by the `script::parse_ty` grammar with
`>` for `->`. Every prefix parses because a trailing operator takes `?` as
its missing operand: `:n` → `Num`, `:n>` → `Num -> ?`, `:n>n` →
`Num -> Num`, `:n*n` → `Num * Num`, `:s` → `Str`. Letters that are not
`n`/`b`/`s` are swallowed inside a spelled base type, so `num > bool` works
too.

**Binder names.** Free text, one keystroke per character. Since Phase 5 the
slot is a name-table write: every keystroke re-issues `Rename(id, buffer)`
against the focused binder's id, and the projection reads the name back out
of the table (`core::names::NameTable`, `core::render::render_id`). A binder
may be called anything, a reference costs one key per character of the name,
and not one binding in this file changed when the slot switched over.

---

## Operator climbing

Without it, typing `1 * 2 + 3` left to right builds `1 * (2 + 3)`,
because the cursor sits on the `2` when `+` arrives — wrong in a way no
key choice compensates for.

> Before an operator or low-precedence form key wraps, ascend from the
> focus while **(a)** the parent frame is a `BinOp`, `Ap`, or `Proj`
> frame, **(b)** the focus is that frame's **rightmost** child, and
> **(c)** the parent's precedence ≥ the arriving key's.

Precedence ladder (matching `core::render`'s public `PREC_*` table):
`space [ ]` 4 (`PREC_APP`) · `*` 3 · `+ - &` 2 · `< =` 1 · `? ; , \` 0.

`1 * 2` + `+` → `1 * 2 + ⦇⦈`; `1 + 2` + `*` → `1 + 2 * ⦇⦈`; `f 1` +
`space` → `f 1 ⦇⦈` (left-associative, because ≥ not >). Climbing never
crosses `Lam`, `Let`, `If`, or `NonEmptyHole` frames — those forms extend
as far right as possible, exactly as in a text grammar — and the
rightmost-child restriction confines the rule to the "typing at the end"
case, the only case where text intuition applies; editing a non-final
child wraps in place, where climbing would be a surprise.

Climbing is not a new action: it expands to `MoveParent`s followed by the
construction, so the action log stays primitive and the Phase 2
sensibility proptest covers every step.

---

## Which keys can decline

Auto-quarantine means every construction key applies at every focus:
`<` on `true` → `⦇true⦈ < ⦇⦈`; `space` on `1` → `⦇1⦈ ⦇⦈`. Exactly two
things can decline, each with visible feedback:

| what | when | feedback |
|---|---|---|
| annotation slot (`SetAnn`) | the annotation would break the body (`λx:?. x + 1` as `Bool`) — a type is not an expression, there is nothing to quarantine | slot stays open, offending type marked, status line says why |
| `Enter` (`Finish`) | the contents of `⦇e⦈` still do not fit | hole marked "does not fit yet: expected τ" |
| definition annotation slot (`SetDefAnn`) | the annotation would break this definition's body, or a *caller's* — a definition's type is the only thing its callers know about it | slot stays open, status line says why |
| `C-d` (`DeleteDefinition`) | there is one definition left | "a document keeps at least one definition" |

The binder-name slot used to be a third row, declining a name that would
capture or orphan a reference (settled item 13, `FRICTION.md` #7). Phase 5
retired it. A name is metadata now: the binder's identity is a uuid the
keyboard never types, references point at that uuid, and `Rename` writes one
row of the name table. It cannot orphan a reference, because no reference
resolves by name; it cannot capture one, because two binders wearing the
display name `x` are still two binders and the inner `x` in the body still
points where it always pointed. So the slot no longer refuses anything: the
TUI applies `Rename` with an `expect`, because there is no failure for it to
report.

`ConstructVar` is the third fallible action, but the candidate list is
drawn from `ctx_at`, so the failing case is unreachable from the keyboard.

---

## Worked examples

Counts are keystrokes; one keystroke may expand to several primitive
actions (climbing, `Tab`), and the action log records the primitives.
Names are written as intended and cost one key per character; the counts in
parentheses are what the same program cost before Phase 5, when a binder
reference was `x`+digit.

### `1 + 2` — 3 keystrokes, 3 actions

| # | key | program | cursor |
|---|---|---|---|
| 1 | `1` | `1` | on `1` |
| 2 | `+` | `1 + ⦇⦈` | rhs |
| 3 | `2` | `1 + 2` | on `2` |

The spec's own criterion, met exactly. No mode key, no commit key, no
movement key.

### `λn:Num. if n < 2 then 1 else n * ⦇⦈` — 13 keystrokes (15 pre-Phase-5)

| # | key | program after | cursor |
|---|---|---|---|
| 1 | `\` | `λ⦇⦈:?. ⦇⦈` | binder name |
| 2 | `n` | `λn:?. ⦇⦈` | binder name |
| 3 | `:` | — | annotation |
| 4 | `n` | `λn:Num. ⦇⦈` | annotation |
| 5 | `.` | — | body |
| 6 | `n` | `λn:Num. n` | on `n` (sole candidate: 1 key) |
| 7 | `<` | `λn:Num. n < ⦇⦈` | rhs |
| 8 | `2` | `λn:Num. n < 2` | on `2` |
| 9 | `?` | `… if n < 2 then ⦇⦈ else ⦇⦈` | then (climbed out of the `<`) |
| 10 | `1` | `… then 1 else ⦇⦈` | on `1` |
| 11 | `Tab` | — | else |
| 12 | `n` | `… else n` | on `n` |
| 13 | `*` | `… else n * ⦇⦈` | rhs — the unwritten recursive call |

**13 keystrokes, 16 primitive actions.** The Phase 3 fixture is 16 actions,
five of which are the `construct-lam` / `move-parent` / `rename` /
`set-ann` / `move-child 0` dance that the binder slots collapse into keys
1–5. Neovim baseline 84 → **0.15×** (0.18× pre-Phase-5).

### `let mkPoint = λx:Num. λy:Num. (x, y) in λp:Num * Num. fst p` — 32 keystrokes

| # | keys | program after | cursor |
|---|---|---|---|
| 1 | `;` | `let ⦇⦈ = ⦇⦈ in ⦇⦈` | binder name |
| 2–8 | `mkPoint` | `let mkPoint = ⦇⦈ in ⦇⦈` | binder name |
| 9 | `=` | — | bound expression |
| 10–14 | `\` `x` `:` `n` `.` | `… = λx:Num. ⦇⦈ in ⦇⦈` | inner body |
| 15–19 | `\` `y` `:` `n` `.` | `… λx:Num. λy:Num. ⦇⦈ …` | innermost body |
| 20 | `x` | `… (x` | on `x` |
| 21 | `,` | `… (x, ⦇⦈)` | snd |
| 22 | `y` | `… (x, y)` | on `y` |
| 23 | `Tab` | — | the `let` body — the fixture's four consecutive `move-parent`s, in one key |
| 24–30 | `\` `p` `:` `n` `*` `n` `.` | `… in λp:Num * Num. ⦇⦈` | body |
| 31 | `[` | `… fst ⦇⦈` | operand |
| 32 | `p` | `… fst p` | on `p` |

**32 keystrokes, ~41 primitive actions** against the fixture's 30 actions
(14 of them movement). Neovim baseline 65 → **0.49×**. `mkPoint` is 7 of
the 32 keys; names dominate this program, which is a property of the
reference, not the grammar.

### Predicted benchmark, all five references

Hand-counted predictions to be falsified by the real TUI run; they enter
`RESULTS.md` only when `cargo run -p nothing-bench` produces them from
recorded keystrokes. The guard from Phase 0 is 3×.

| # | program | Neovim | Phase 3 actions | predicted keys | predicted ratio |
|---|---|---:|---:|---:|---:|
| 1 | factorial | 84 | 16 | 13–15 | ~0.17× |
| 2 | list_map | 114 | 22 | ~26 | ~0.23× |
| 3 | record | 65 | 30 | 32 | 0.49× |
| 4 | state_machine | 151 | 24 | ~20 | ~0.13× |
| 5 | nested_conditional | 146 | 35 | ~29 | ~0.20× |

All far inside the guard. `record` is the number to watch — worst here for
the same reason it was worst in Phase 3 (deep nesting, four binders).

---

## Rejected — bindings and features talked out of

1. **A modal verb layer** (Vim's `d`/`c`/`i`, letters as verbs in a normal
   mode, insert entered at holes). The strongest rejected candidate. A
   modal grammar spends the letters on verbs, and the letters are the
   literal-entry path — 80% of keystrokes. Even with the clever variant
   where an empty hole auto-enters entry (so the mode key is usually
   free), `l` means *move* or *the letter l* depending on state, and
   mid-name motions need an `Esc` first. Vim's grammar is right when the
   document is a string; the premise here is that it is not. What survives
   is the object-verb idea with the object free: the cursor is always the
   selection.
2. **Automatic advance to the next empty hole after every construction.**
   Proposed by `bench/RESULTS.md` on real evidence (movement was 39% of
   actions). Rejected because it fights the 80% path directly:
   auto-advancing off a freshly typed `1` makes `12` impossible, and
   advancing off a `Var` makes climb-and-wrap (`n` then `*`) unreachable.
   Replaced by explicit `Tab`, which is one key, never surprising, and
   collapses the same `move-parent` runs (worked example 3, key 23).
3. **`>` as a reversed `<`** (so `x > 0` types in the user's order — the
   references want it three times). Rejected because the operand swap is
   not expressible as a Phase 2 action: auto-wrap always puts the focus in
   child 0, and faking it would make the action log lie about what was
   typed. The honest fix is `Op::Gt` in Phase 1, which the spec forbids
   for now. Type `0 < x`.
4. **Vim-style numeric counts** (`3↓`, `2Tab`). Digits are the literal
   path; putting a prefix ambiguity on the single most common key class to
   save a repeated arrow is a bad trade. Motions repeat by repetition.
5. **Unbound-name-then-`=` minting a `let`** (type `total`, nothing
   matches, `=` means `let total = ⦇⦈ in ⦇⦈`). Elegant and rejected: the
   identical keystrokes `n` `=` would mean *equality* when `n` is bound
   and *a new binding* when it is not — a key whose meaning flips on an
   invisible property is the MPS failure mode the spec names. `;` is one
   key and always means the same thing.
6. **`.` as `fst` with a `.1` peek for `snd`** (and `-` as a pending sign
   peeked ahead one key). Both were in the strongest draft and both are
   undo-or-lookahead machinery bought to save one key. `[`/`]` cost the
   same and mean one thing each; `~` negates with no pending state. This
   also removes the draft's own flagged friction point where `.` meant
   "leave the annotation slot" and "construct fst" on adjacent keystrokes.
7. **Commit-on-unique-prefix in name entry** (`x` uniquely names `xs`, so
   end the run). Rejected: it makes the run's *termination* depend on
   unrelated program content — bind another `x…` name elsewhere and the
   keystroke silently changes meaning. Commit-live keeps re-deriving from
   the buffer instead, so the ranking may change what is committed but
   never what your next keystroke does.
8. **A `:command` line** (`:set-ann Num -> Num`). `:` is worth more as the
   annotation key — the character the language itself renders — and a
   command line re-introduces the stringly-typed intermediate this project
   exists to eliminate. `action/src/bin/repl.rs` already exists for
   verbose scripted actions.

---

## Coverage — every Phase 2 action has a binding

| `Action` | key | `Action` | key |
|---|---|---|---|
| `MoveChild(0)` | `↓` (`MoveChild(n)` = `↓` `→`×n; `move_sibling` is parent+child in the zipper, so the composition is exact) | `ConstructIf` | `?` |
| `MoveParent` | `↑` | `ConstructLet` | `;` |
| `MoveNextSibling` | `→` | `ConstructPair` | `,` |
| `MovePrevSibling` | `←` | `ConstructProj(L/R)` | `[` / `]` |
| `Delete` | `Del`; `Bksp` off a run/hole | `ConstructNonEmptyHole` | `!` |
| `ConstructNum(i64)` | `0`–`9`, `~` | `SetAnn(Ty)` | `:` + annotation slot |
| `ConstructBool(bool)` | `t`/`f` (top-ranked candidates) | `Rename(Id, String)` | typing in the binder slot |
| `ConstructVar(Id)` | letters (name run) | `Finish` | `Enter` on or inside `⦇e⦈` |
| `ConstructStr(String)` | `"` opens the run; every printable key re-issues it with one more character | undo / redo | `C-z` / `C-r` |
| `ConstructLam` | `\` | quit | `C-q` |
| `ConstructAp` | `space` | | |
| `ConstructBinOp(Op)` | `+ - * < =`, and `&` for `Op::Concat` | | |
| `CreateDefinition` | `C-n` | `MoveNextDef` | `C-↓` |
| `DeleteDefinition` | `C-d` | `MovePrevDef` | `C-↑` |
| `SetDefAnn(Ty)` | `C-t` + annotation slot | `MoveToDef(Id)` | `C-↑`/`C-↓` repeated (the pane shows how far); the protocol has it by name |
| `Rename(Id, String)` of a definition | typing in the `C-l` slot | | |

Editor-level, backed by the action log: `Tab`/`S-Tab` (next/previous hole,
either kind), undo as truncate-and-replay **per keystroke** (one `C-z` undoes
one key, even when the key expanded to several actions), `Esc` (end the name
run — the program is already committed, so this only clears the buffer).
The bench harness should record both keystrokes and primitive actions per
program from now on: the ratio and the composition answer different
questions.

## What the status line must show

Not bindings — the grammar does not work without these:

- **the expected type at the cursor** (`expected_ty_at`), always — it is
  what makes candidate ranking legible rather than magic;
- **the current slot** when the focus is a binder part;
- **the candidate list** during a name run, selected entry marked, each
  candidate's type shown, rendered beside the buffer *in the focus
  position* — entry must look like it is happening to the program,
  because it is;
- **that a string run is open**, and the pending `\` when the escape is
  armed — the run is the one piece of state a keystroke's meaning depends on
  that the projection does not already show, so it is the one thing the line
  may never omit;
- **a quarantine marker** on every non-empty hole, with "fits now — press
  Enter" whenever `Finish` would succeed — and the same answer from *inside*
  the wrapper (`inside ⦇⦈ · fits now — press Enter`), because that is where
  the keystroke that repaired the expression leaves the cursor;
- **how many quarantines the program still contains**, whenever there are
  any: "am I done?" is the question the editor was answering by counting
  empty holes alone.

And the projection itself must show **how much is selected**: the focus is
drawn as a highlighted span, not as two thin markers in a long line. A
whole-program focus has to *look* like one — typing replaces the selection,
so the size of what a letter is about to replace cannot be invisible — and
`⦇»e«⦈` (the cursor on a quarantine's contents) has to be legibly different
from `»⦇e⦈«` (the cursor on the wrapper).

---

## Settled by the implementation

Written after wiring the construction keys (`nothing-tui`). The matrix above
is normative and unchanged; these are the cases it did not pin down, decided
once and pinned by `tui/tests/matrix.rs`, which is the matrix as a table.

1. **Where the cursor lands after a wrap is the calculus's rule, not a new
   one**: the form's first empty child in source order. So `+` at an empty
   hole gives `»⦇⦈« + ⦇⦈` (the *left* operand), and `[` on a written `e`
   gives `»fst ⦇e⦈«` — no empty child, so the cursor rests on the
   projection. Worth knowing when reading the table: `1 + *` then `2` is
   `1 + 2 * ⦇⦈`.
2. **`!` never climbs.** Quarantine addresses the focus itself; every other
   wrapping key carries its precedence into the climb rule.
3. **`:` inside the annotation slot is inert** (it is the key that reaches
   the slot), and a `)` with no `(` is inert too — exiting the slot would be
   a worse reading of "I am done here" when the user is plainly still
   writing a type. Everything else the slot does not understand still exits
   and is reprocessed.
4. **`Del` in a binder slot declines with a hint.** `Del` deletes an
   *expression*, and a name is not one; `↑` first.
5. **A name run survives the descent into a non-empty hole.** Typing `x` at
   a `Bool` hole quarantines the variable and leaves the cursor on the
   wrapper; the next character goes on refining the name inside it rather
   than starting again.
6. **A run's re-commit is `Delete` + the new construction.** That is the
   anchored re-derivation this document describes, expressed in primitive
   actions, so the log stays primitive and one `C-z` still undoes one key.
   When the refined buffer matches nothing, the run's own commit is
   withdrawn and the hole it started from comes back.
7. **Undo covers a keystroke that changed only the cursor** — `:` into the
   annotation slot applies no action, and `C-z` must still take it back —
   but not one that changed nothing at all.
8. **The binder-name slot writes whatever you type**, character by
   character, as `Rename(id, buffer)` — the id is the binder's own, never
   typed and never parsed out of the name. Before Phase 5 the slot read the
   digits of the name as the identity (`x0` → `SetBinderId(Id(0))`) and a
   name with no digits could only sit in the buffer and say so on the status
   line. The name table removed the parse, and with it the one case where
   what you had typed was not yet in the program.
9. **An unknown expected type ranks nothing.** §"Literal entry" rank 2
   reads "exact match above consistent-via-`?` above inconsistent"; taken
   literally at a hole whose expected type *is* `?`, a `?`-typed binder
   would score "exact" and be offered first — promoting the binder the
   editor knows least about at exactly the position where it knows
   nothing. So when `expected_ty_at(cursor)` is `?` every candidate scores
   alike on rank 2 and the innermost-scope tie-break decides, which is the
   pre-completion order. Ranking never *filters*: a name that does not fit
   is still offered (marked `✗`) and still commits, because the calculus
   quarantines it rather than refusing it. Pinned by
   `complete::tests::an_unknown_expected_type_ranks_nothing_and_filters_nothing`.
10. **There is still no selection key and no confirm key.** Commit-live
   already means the top-ranked candidate is *in the program* after every
   keystroke, so "select" is "type another character" and "commit" is
   "the keystroke you just pressed"; `Esc` ends the run and leaves the
   commit standing. The status line marks the committed entry `‹…›` and
   shows the rest of the ranked list beside it, which is the whole of the
   completion UI. A cycle-the-selection key was considered and dropped for
   the reason §Rejected item 7 gives about commit-on-unique-prefix: it
   would make what a keystroke means depend on program content elsewhere.

Items 11–14 are dated **2026-08-27** and are the grammar's answer to the
friction session (`FRICTION.md`); each is pinned by a test named after it.

11. **`Tab`/`S-Tab` walk both kinds of hole**, in the same source order
   (`FRICTION.md` #12). A quarantine is the editor's own note that an
   expression does not fit yet; a key that walks what is left to do cannot
   skip the one construct that means there is something left to do. The
   consequence is the message: with nowhere left to go these keys now say
   "nothing unfinished: this program has no holes", where they used to say
   "no empty hole in this program" with two `⦇e⦈` on screen. Pinned by
   `movement::tab_reaches_every_hole` and
   `keys::tests::tab_walks_quarantines_too_and_says_when_nothing_is_left`.
12. **`Enter` finishes the quarantine the cursor is *inside*, not only the
   one it is on** (`FRICTION.md` #10). The keystroke that finally makes a
   quarantined expression fit leaves the cursor on the contents, one step in;
   requiring `↑` first charged two keystrokes for a repair the editor had
   already noticed. The climb expands to `MoveParent`s, so one `C-z` still
   undoes one key. From inside a quarantine that does *not* fit, `Enter` says
   what it expected instead of falling through to "next hole": teleporting
   the cursor across the program is how six keystrokes went into the wrong
   subtree during the session (#13). Pinned by
   `keys::tests::enter_finishes_the_quarantine_the_cursor_is_inside`, which
   also asserts it is exactly one keystroke cheaper than walking out.
13. ~~**The binder-name slot declines a rename that would capture.**~~
   **Retired by Phase 5** (2026-08-28). The check made sense while an `Id`
   *was* the display name: renaming onto an id already in scope silently
   repointed references, and made the outer binder unreachable from the
   keyboard besides — the candidate list offered one `x0` and no keystroke
   produced a reference to the other (#8). With names in a side table
   neither half survives: `Rename` writes metadata, no reference resolves by
   name, and two binders displayed as `x` are two binders. The prediction in
   the old wording — "this check is about identity and stays" — was wrong,
   because the keyboard stopped setting identity at all. The three tests
   that pinned it are gone; what stands in their place is
   `keys::tests::two_binders_may_wear_the_same_display_name_without_capture`
   and `keys::tests::renaming_a_binder_renames_every_reference_to_it_at_once`.
14. **The program pane is a viewport, and the focus is a span.** The
   projection is wrapped by the editor rather than by `Paragraph`, so the
   line the cursor is on is known and the window is chosen to contain it
   (#3); the border title says which lines are on screen when not all of them
   are. The focus is drawn as a highlighted span rather than two thin markers
   (#13, #19). Neither is a binding change and neither costs a keystroke,
   which is why they are here rather than in the grammar table.

Item 15 is dated **2026-08-28** and belongs to Phase 6.

15. **There is no run key, because there is no run.** A line under the
   program pane shows the value of the expression the cursor is on,
   recomputed on every keystroke (`tui/src/live.rs`). It reads `⇒ v` for
   the focus, or `program ⇒ v` when the focus is a bare hole or mentions a
   binder from further out and therefore has no value of its own. Three
   outcomes are visibly different: a value (`⇒ 120`), an indeterminate
   result — the program ran until it needed a hole and stopped, printing
   the partially-evaluated expression, which hole it wants, and what was
   bound where it stopped (`⇒ 1 + ⦇⦈ · blocked on ⦇⦈#a1b2c3d4 · n = 5`) —
   and exhaustion (`⇒ … still running after 4000 steps`), which exists
   because recursion arrived in Phase 6 and the editor must not hang on a
   half-typed loop. A key was considered for "evaluate now" and rejected
   for the same reason there is no confirm key on a name run: if the value
   is only correct after you press something, the screen is allowed to lie
   in between. No binding changed; the reserved characters listed above are
   still reserved. Pinned by
   `live::tests::editing_an_expression_updates_its_displayed_value_with_no_run_command`
   and `render::tests::the_live_value_is_on_screen_under_the_program_and_follows_every_edit`.

Item 16 is dated **2026-08-28** and belongs to Phase B1.

16. **Definitions reuse the slot grammar rather than adding one.** A
   document's definition head is two more rows of the §Slots table, so the
   only genuinely new bindings are the four that have no positional
   equivalent: `C-n`/`C-d` to add and drop, `C-↑`/`C-↓` to walk the list.
   `C-l` and `C-t` are shortcuts, not new grammar — `↑` off the root of a
   body reaches the name slot and `↓` reaches the type slot, exactly as `:`
   and `.` reach a lambda's. Three consequences worth pinning:
   **(a)** the definition the cursor is in is in its own scope, so writing a
   recursive call is a name run like any other (`m` completes to `main`) and
   costs no keystroke of ceremony — this is what took factorial from a
   fixture with a hole in it to the reference program, at the price of
   twelve keystrokes recorded in `bench/RESULTS.md`.
   **(b)** `C-d` never leaves a dangling reference: every call to the dropped
   definition becomes an empty hole in the same action, so the document is
   well-typed before and after and one `C-z` puts it all back
   (`DECISIONS.md`, 2026-08-28).
   **(c)** the definition list is a pane beside the program, not a mode: it
   only appears when there is more than one definition and the terminal is
   wide enough for both, and it is never focusable — `C-↑`/`C-↓` move the
   real cursor, and the pane is showing you where it went. Pinned by
   `tui/tests/definitions.rs`.

Item 17 is dated **2026-08-29** and belongs to Phase B2.

17. **The string run is a run, not a slot.** It adds no row to the §Slots
   table and no `Frame` to the zipper: the open flag lives beside the name
   run's buffer in the editor, and the *literal in the program is the
   buffer*, so there is nothing to keep in sync. Four cases the matrix
   leaves to the implementation:
   **(a)** `"` on a focused `Str` **re-opens** the run at the end of the
   string rather than replacing it, which is the digit-on-a-`Num` rule
   generalised — come back a week later and add a word. Every other focus
   replaces, because that is what typing a literal has always done.
   **(b)** A string run survives the descent into a quarantine, exactly as a
   name run does (item 5). `"` at a `Num` hole gives `⦇""⦈` with the cursor
   on the wrapper and the run still open, so the next character goes on
   building the string inside it.
   **(c)** The armed `\` is the only keystroke in this file that changes
   nothing — not the program and not the cursor — so `C-z` is not spent on
   it (item 7's second half) and `Bksp` disarms it. The status line says
   `string · pending escape · \" or \\`, and the projection shows the
   pending backslash in the focus position, because a keystroke that has not
   landed anywhere has to be visible somewhere.
   **(d)** `&` climbs like `+` and quarantines like every other operator:
   `1` then `&` gives `⦇1⦈ ++ ⦇⦈`, and `"a" ++ 1` cannot be typed without the
   quarantine showing. `=` is the one operator whose operand type is not
   fixed by the key — it compares at whichever of `Num`, `Bool` or `Str` its
   operands are (`DECISIONS.md`, 2026-08-29), so `"a"` then `=` gives
   `"a" == ⦇⦈` with the hole expecting `Str`, and the ranking at that hole is
   about strings without the user having said so.
   Pinned by `keys::tests::a_string_run_takes_every_printable_key_as_text`,
   `keys::tests::a_quote_reopens_a_finished_string_at_its_end`, and
   `tui/tests/matrix.rs`, which is column H as a table.

Measured, replacing the predicted table's middle column (`tui/tests/keys/`,
one keystroke per line). The 2026-08-28 column is the definition era: the
programs are documents now, and `factorial` and `record` were rebuilt to say
what they always meant rather than what Phase 1 could express.

| # | program | Neovim | keystrokes | actions | ratio | was (2026-08-27) |
|---|---|---:|---:|---:|---:|---:|
| 1 | factorial | 84 | 28 | 33 | 0.33× | 16 / 0.19× |
| 2 | list_map | 114 | 29 | 35 | 0.25× | unchanged |
| 3 | record | 65 | 46 | 40 | 0.71× | 33 / 0.51× |
| 4 | state_machine | 151 | 24 | 33 | 0.16× | unchanged |
| 5 | nested_conditional | 146 | 31 | 42 | 0.21× | unchanged |
| 6 | greeting | 127 | 52 | 56 | 0.41× | new (2026-08-29) |

Row 6 arrived with strings on 2026-08-29 and is the first reference whose
cost is mostly *content*: 27 of its 52 keystrokes are the characters and
quotes of four string literals, which no projection can make cheaper. The
structure cost 25. The other five rows are byte-identical to 2026-08-28 —
the string run taxed nothing that existed.

`record` is still the number to watch, and it got worse for a good reason:
its constructor is a named, annotated top-level definition instead of a
`let`, which is thirteen more keystrokes and five *fewer* primitive actions.
The three unchanged rows are the control — the definition bindings cost
nothing in a program that does not use them. These are the editor's own
numbers; the dated ratios in `bench/RESULTS.md` are written by the benchmark
re-run.

---

## Phase 11 — projections

One binding, added on top of everything above rather than into it: `C-p`
cycles which *projection* renders the focused program. A projection is a
view of the AST and an edit surface at once — it still turns keys into the
same `Action`s from §Coverage, just sometimes by a different route.

- **`C-p` cycles `auto → text → state machine → beginner → auto`.** "auto"
  is not a fourth look, it is the absence of an override: the editor picks
  a projection by matching the shape of the whole program, currently
  recognising one shape (a function whose body is two or more chained
  `if x == k then … else …` tests against its own parameter, ending in a
  default — the three-case state machine reference program is exactly this
  shape). A program that does not match renders as text whether or not
  there is an override. Pressing `C-p` once always **forces text**,
  because the first thing an override needs to do is get you out of a
  table you did not ask for; the next three presses walk state machine,
  beginner, and back to auto. `C-p` is not an `Action` and is not logged —
  it changes what you see, not the program — so it costs nothing to press
  and nothing to undo.
- **The state machine projection is a table, not a diagram of lines.**
  Each row is `condition  ->  result`; the last row is always `else`, the
  chain's final default. Moving `↑`/`↓` inside the table moves between
  rows; `↓`/`↑`/`←`/`→` on a fresh table (nothing selected yet) start on
  row 0's condition. Landing on a cell moves the *real* cursor to the
  subexpression that cell displays — the constant a condition compares
  against, or the row's result — so every other key (digits, `~`, `!`,
  `Tab`, `Enter`, …) reaches it exactly as it would in the text projection,
  because it is the same node. `←`/`→` switch between a row's condition
  and result cell; the `else` row has no condition cell, so `←` on it goes
  to the result instead of nowhere. Editing a cell is editing the AST:
  typing `5` on a condition changes which case it matches, typing on a
  result changes what that case produces, and both show up immediately in
  the text projection because there is only one program underneath the two
  views. Pinned by `tui/tests/projections.rs`, which drives an edit through
  the table and reads it back as text, then the other way around.
- **The beginner projection is the same AST in sentences.** No operator
  symbols anywhere — `+` is "the sum of _ and _", `==` is "whether _
  equals _", `if`/`then`/`else` is "if _ then _ otherwise _", a lambda is
  "a function taking _ (_) and returning _", an empty hole is `(blank)`
  and a quarantine is "(not yet fitting: _)". It is read-only in the sense
  that it adds no new keys: typing still edits the focused node through
  the ordinary grammar, and the sentence around it changes to match, the
  same way the text projection's parentheses change when a sibling is
  added. Snapshotted for all ten `core::examples` plus the factorial and
  state-machine fixtures in `tui/src/beginner.rs`; whether it actually
  reads clearly to someone who has never seen the language is a question
  a test cannot answer and still wants a person to look at it.
- **Both new projections are the same `Projection` trait as the text one**
  (`tui/src/projection.rs`): given the state, produce the marked-up text
  (the cursor is `»…«`, exactly as in the text projection, so the shared
  wrapping/scrolling/highlighting pipeline in `render.rs` does not need to
  know which projection it is drawing), and given a keystroke, either
  handle it in the projection's own vocabulary or hand it back to the
  ordinary grammar. The text projection's `handle_key` always hands every
  key back — it has no vocabulary of its own — which is what makes this
  phase additive: every binding and every test above this line is exactly
  as it was.
