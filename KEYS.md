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
| `Record` | field 0's **name** | field 0's value | field 1's **name**, … |
| `Match` | scrutinee | arm 0's **constructor** | arm 0's **binder**, arm 0's body, … |
| everything else | child 0 | child 1 | child 2 |

The `Record` and `Match` rows are the ones with no fixed width: a record has as
many fields as it has and a match as many arms as its variant has constructors,
so their editor-level child lists are `name₀ value₀ name₁ value₁ …` and
`scrutinee ctor₀ binder₀ body₀ ctor₁ binder₁ body₁ …`. It is still the same
table — a name slot beside the thing it names — and `←`/`→` walk it in that
order.

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
  S-Tab  previous hole, either kind                (true/false/nil/readline
                                             "    are candidates, not keys)
OPERATORS  (climb, then wrap)                     string run: open, and close
  +   add        e + ⦇⦈    ConstructBinOp        with " again. Inside it every
  -   subtract   e - ⦇⦈         〃                printable key is one character
  *   multiply   e * ⦇⦈         〃                of text; \" and \\ escape
  <   less than  e < ⦇⦈         〃
  =   equals     e == ⦇⦈        〃         FORMS  (wrap the focus)
  &   join text  e ++ ⦇⦈        〃           space  apply       e ⦇⦈   ConstructAp
  :   cons       e :: ⦇⦈   ConstructCons     \      lambda      λ⦇⦈:?. e   …Lam
                                             ?      if e then ⦇⦈ else ⦇⦈   …If
IN A BINDER-NAME SLOT                        ;      let ⦇⦈ = e in ⦇⦈       …Let
  a-zA-Z0-9_  name the binder  Rename        ,      pair        (e, ⦇⦈)   …Pair
  :   → annotation slot                      [  ]   fst e / snd e   ConstructProj
  =   → bound expr (let, bind)               /      fold e ⦇⦈ ⦇⦈    ConstructFold
  .   → body                                 !      quarantine  ⦇e⦈   …NonEmptyHole
                                             {      record  {f = e}  ConstructRecord
                                             .      field   e.f     ConstructField
                                             `      inject  `C0 e   ConstructInj
                                             |      match   match e {…}  …Match
                                             $      print   print e  …Print
                                             '      pure    pure e   …CmdPure
                                             >      bind  bind x <- e in ⦇⦈  …CmdBind

IN AN ANNOTATION SLOT (re-issues SetAnn)  HOLES & HISTORY
  n Num  b Bool  s Str  ? unknown           Bksp  run: un-type one char ·
  > arrow  * product  [ list  c Cmd               empty hole: ascend · else Delete
  ( ) grouping                              Del   focus → ⦇⦈           Delete
  Bksp drop tok  .  → body                  Enter Finish the ⦇e⦈ on or around
  Tab/Enter → next hole                           the cursor, else next hole
                                            Esc   end the run (no-op otherwise)
DEFINITIONS  (the document is a list of      C-z / C-r  undo / redo · C-q quit
  them; a body may call any of them)
  C-↓  next definition      MoveNextDef      C-l  → definition-name slot
  C-↑  previous definition  MovePrevDef      C-t  → definition-type slot
  C-n  new definition, cursor in its name    CreateDefinition
  C-d  drop this definition (never the last) DeleteDefinition

RECORDS  (a record's field list is the other list a cursor can be inside, so
  C-n / C-d address a field there and a definition everywhere else)
  C-n  one more field, cursor in its name    AddField
  C-d  drop this field; each e.f becomes ⦇e⦈ RemoveField
  C-←  move this field one place earlier     MoveFieldPrev
  C-→  move this field one place later       MoveFieldNext

IN A FIELD SLOT  (reached by ← from a field's value, or opened by `.`)
  a-zA-Z0-9_  on a record's field: name it            Rename
              on a projection: pick the field, ranked SetField
  =   → the field's value        anything else  exit → the node, reprocess

VARIANTS  (a match's arms are the third list a cursor can be inside, so C-n /
  C-d address an arm there, a field in a record, and a definition elsewhere)
  C-n  one more constructor: a hole arm in every match on it   AddArm
  C-d  drop this arm (refused while the scrutinee still needs it)  RemoveArm

IN A CONSTRUCTOR SLOT  (reached by ← from an arm's body, or opened by `` ` ``)
  a-zA-Z0-9_  on a match arm: name the constructor    Rename
              on an injection: pick it, ranked        SetConstructor
  =   → the payload / the arm's body
                                 anything else  exit → the node, reprocess
```

49 bindings. Deliberately unbound and held in reserve for Phase 6+:
`( ) } ^ % # @` outside the slots. B3 spent `$`, `'` and `>` and left
`readline` to the completion path (item 21).

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
| **letter / `_`** | start name run | start name run (commit replaces `e`) | start name run | append, re-filter, re-commit | `n`/`b`/`s` set base type and `c` opens `Cmd`, a prefix taking the next type (spelled `num`/`bool`/`str`/`cmd` also works — unknown letters inside a spelled name are swallowed); other letters exit → body, reprocess | append to name | descend, then as inner | append the character |
| **op** `+ - * < = &` | wrap: `⦇⦈ op ⦇⦈` | climb, then wrap | climb, then wrap | end run, then as B | `*` product; others exit → body, reprocess | `=` on a `let` → bound slot; others exit → body, reprocess | descend, then as inner | append the character |
| **form** `space \ ? ; ,` | insert the form | climb (`\ ? ; ,`), then wrap | climb, then wrap | end run, then as B | `?` = the unknown type; others exit → body, reprocess | exit → body, reprocess | descend, then as inner | append — except `\`, which arms the escape |
| **`"`** | `ConstructStr("")`, run opens | replace: `ConstructStr("")`, run opens — on a focused `Str`, **re-open** at its end | replace: `ConstructStr("")`, run opens | end run, then as B | exit → body, reprocess | exit → body, reprocess | descend, then as inner | **close the run**; armed, append `"` |
| **`[` `]`** | `fst ⦇⦈` / `snd ⦇⦈` | climb (app-level), then wrap | wrap: `fst ⦇n⦈` (quarantined) | end run, then as B | `[` = `List`, a prefix taking the next type; `]` exits → body, reprocess | exit → body, reprocess | descend, then as inner | append the character |
| **`/`** | `fold ⦇⦈ ⦇⦈ ⦇⦈` | climb (app-level), then wrap | climb, then wrap: `fold n ⦇⦈ ⦇⦈` (quarantined) | end run, then as B | exit → body, reprocess | exit → body, reprocess | descend, then as inner | append the character |
| **`!`** | wrap the hole: `⦇⦇⦈⦈` | quarantine: `⦇e⦈` | `⦇n⦈` | end run, then as B | exit → body, reprocess | exit → body, reprocess | **acts on the wrapper**: `⦇⦇e⦈⦈` | append the character |
| **`~`** | no-op, hint | no-op unless `Num` | negate: `ConstructNum(-n)` | end run, then as B | exit → body, reprocess | no-op | descend, then as inner | append the character |
| **`:`** | wrap: `⦇⦈ :: ⦇⦈` | focused `Lam`: → its annotation slot; else climb, then wrap `e :: ⦇⦈` | climb, then wrap | end run, then as B | no-op | → annotation slot (`Lam` only) | descend, then as inner | append the character |
| **`{`** | `{f = ⦇⦈}`, cursor in the field slot | wrap: `{f = e}`, cursor in the field slot | wrap: `{f = n}` | end run, then as B | exit → body, reprocess | exit → body, reprocess | descend, then as inner | append the character |
| **`.`** | `⦇⦈.f`, field slot opens | wrap: `e.f`, field slot opens (never climbs) | wrap: `⦇n⦈.f` (quarantined) | end run, then as B | → body slot | → body slot | descend, then as inner | append the character |
| **`` ` ``** | `` `C0 ⦇⦈ ``, constructor slot opens | wrap: `` `C0 e ``, constructor slot opens | wrap: `` `C0 n `` | end run, then as B | exit → body, reprocess | exit → body, reprocess | descend, then as inner | append the character |
| **`\|`** | `match ⦇⦈ {}`, cursor on the scrutinee | wrap: `match e {…}`, one hole arm per constructor | wrap: `match ⦇n⦈ {}` | end run, then as B | exit → body, reprocess | exit → body, reprocess | descend, then as inner | append the character |
| **`$`** | `print ⦇⦈` | climb (app-level), then wrap: `print e` | climb, then wrap: `print ⦇n⦈` (quarantined) | end run, then as B | exit → body, reprocess | exit → body, reprocess | descend, then as inner | append the character |
| **`'`** | `pure ⦇⦈` | climb (app-level), then wrap: `pure e` | climb, then wrap: `pure n` (never quarantined) | end run, then as B | exit → body, reprocess | exit → body, reprocess | descend, then as inner | append the character |
| **`>`** | `bind x <- ⦇⦈ in ⦇⦈` | climb (binder-level), then wrap: `bind x <- e in ⦇⦈` | climb, then wrap: `bind x <- ⦇n⦈ in ⦇⦈` (quarantined) | end run, then as B | **arrow**, as it always has been | exit → body, reprocess | descend, then as inner | append the character |
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
| `.` (`ConstructField`) | the document has no field to name — a field is an identity, not a string, so there is nothing to project | "`.` projects a field; this document has none yet" |
| `C-d` (`RemoveArm`) | some scrutinee still injects this constructor — a match with a missing arm is not a program this editor can hold | "this match still has to answer for that constructor" |

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
| `ConstructAp` | `space` | `ConstructCons` | `:` (except on a focused `Lam`, where `:` still opens the annotation slot) |
| `ConstructBinOp(Op)` | `+ - * < =`, and `&` for `Op::Concat` | `ConstructFold` | `/` |
| `ConstructNil` | `nil`, a name-run candidate like `true`/`false` | `Ty::List` in an annotation | `[` |
| `CreateDefinition` | `C-n` | `MoveNextDef` | `C-↓` |
| `DeleteDefinition` | `C-d` | `MovePrevDef` | `C-↑` |
| `SetDefAnn(Ty)` | `C-t` + annotation slot | `MoveToDef(Id)` | `C-↑`/`C-↓` repeated (the pane shows how far); the protocol has it by name |
| `Rename(Id, String)` of a definition | typing in the `C-l` slot | `ConstructRecord` | `{` |
| `AddField` | `C-n` with the cursor in a record | `RemoveField` | `C-d` with the cursor in a record |
| `ConstructField(Id)` | `.` | `SetField(Id)` | typing in the field slot `.` opened |
| `MoveFieldPrev` | `C-←` | `MoveFieldNext` | `C-→` |
| `Rename(Id, String)` of a **field** | typing in the field slot `←` reaches | `ConstructInj` | `` ` `` |
| `ConstructMatch` | `\|` | `SetConstructor(Id)` | typing in the constructor slot `` ` `` opened |
| `AddArm` | `C-n` with the cursor in a match | `RemoveArm` | `C-d` with the cursor in a match arm |
| `Rename(Id, String)` of a **constructor** | typing in the constructor slot `←` reaches | `SetArmBinderId(Id)` | no binding: an arm's payload binder is minted by `AddArm` and named by `Rename`, so the keyboard never sets its identity (the protocol has it by name) |
| `SetConstructor(Id)` re-aiming an **arm** | no binding: the same reason — an arm's case is minted by `AddArm`; the protocol has it, the keyboard does not | | |

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

Item 18 is dated **2026-08-29** and belongs to Phase B2's second half.

18. **Lists cost one new key, and two keys that meant nothing now mean
   something.** The three list constructions needed bindings and only one of
   them was a new character:
   **(a)** `:` is **cons**. Reading down the `:` row of the matrix, three of
   its eight cells said "no-op, hint" and a fourth said "no-op + hint unless
   the focus is a `Lam`"; those are the four cells that now build a cons
   cell, and *no cell that already meant something changed meaning*. The one
   exception stays: on a focused `Lam`, `:` still opens the annotation slot,
   because that is the shortcut item 16 describes and a list of functions is
   not why anyone reaches for `:`. Consing a written lambda is therefore the
   one thing this key cannot do; you type the cell first (`:` at the hole,
   then `\`), which is the order you would type it in anyway. `:` climbs on
   the ordinary rule at `PREC_CONS`, which sits between comparison and
   addition, so `1 + 2` then `:` gives `1 + 2 :: ⦇⦈` and `a < b` then `:`
   gives `a < (b :: ⦇⦈)`. It never climbs out of a `ConsTail`, which is what
   makes `1` `:` `2` `:` `n` type the right-nested `1 :: 2 :: nil` rather
   than fighting associativity.
   **(b)** `/` is **fold**, taken out of the reserve list — APL's reduce, and
   the only genuinely new binding. Fold could not be a completion candidate
   the way `nil` is: the name run commits by `Delete` + re-construct and
   assumes the action leaves the cursor *on* what it built, and
   `ConstructFold` descends into a child. A key it is, and the reserve list
   is one character shorter. The cost is that a future `Op::Div` would need
   a different character; that is a trade recorded rather than hidden.
   **(c)** `nil` is a **candidate, not a key** — the `true`/`false` rule
   (item 10's neighbourhood) applied to the third literal the language has.
   It types `List ?`, so at a `List Num` hole it ranks above anything that
   does not fit and one keystroke usually commits it; at a `Num` hole it is
   still offered and still quarantines, because ranking never filters.
   **(d)** `[` is **`List`** inside an annotation slot, a prefix taking the
   next type: `[n` is `List Num`, `[[n` is `List (List Num)`, `[(n>n)` is
   `List (Num -> Num)`. Outside the slot `[` is still `fst`; the annotation
   slot has always had its own small alphabet (`*` is product there and
   multiply outside), and this fills the cell where `[` used to mean "I do
   not understand this, exit". A `[` where the buffer already holds a
   complete type is swallowed and visible, the way `)` with no `(` is.
   Pinned by `keys::tests::cons_is_a_colon_and_a_written_lambda_keeps_its_annotation_slot`,
   `keys::tests::fold_and_nil_are_a_key_and_a_candidate`,
   `annot::tests::a_bracket_is_the_list_prefix_and_takes_the_next_type`, and
   the four new rows/columns of `tui/tests/matrix.rs`.

Item 19 is dated **2026-08-29** and belongs to Phase B2's third feature.

19. **Records cost one new printable key, one key that meant nothing, and
   nothing else.** A record is the first form with no fixed arity, so it is
   the first form that needs keys for *how many* as well as *what*; the
   grammar paid for that out of bindings it already had.
   **(a)** `{` is **record construction**, taken from the reserve list. It
   wraps like every other form key — `1` then `{` is `{f1 = 1}` — and it
   leaves the cursor in the new field's **name slot**, exactly as `\` leaves
   it in a binder's. That is the whole reason a record is cheap to type: the
   first thing you know about a field is what it is called.
   **(b)** `.` is **field projection**, and it is the `:` argument of item 18
   repeated: reading down the `.` row of the matrix, three of its eight cells
   said "no-op, hint" and those are exactly the three that now build a
   projection. The two cells where `.` already meant something — leaving a
   binder-name or annotation slot for the body — are untouched, which is why
   the friction the draft grammar flagged in §Rejected item 6 does not come
   back: `.` means "the thing named next", in a slot and out of one.
   `.` **never climbs**, because a projection binds tighter than application:
   with the cursor on the `p` of `f p`, `.` has to give `f (p.x)` and not
   `(f p).x`. It is the first key whose precedence is `PREC_ATOM`, and no
   frame is climbable above `PREC_APP`, so the climb rule is a no-op for it
   by construction — including out of a projection's own subject, which is
   why `p.x` then `.` wraps in place and reads `p.x.y`.
   **(c)** `C-n` and `C-d` **generalise instead of multiplying**. They meant
   "add / drop one row of the list you are in", and until now the document
   was the only such list; a record's fields are the second, so inside a
   record they add and drop a *field* and everywhere else they still add and
   drop a *definition*. The one asymmetry is deliberate: `C-n` appends a
   field from anywhere in a record, including the record node itself, but
   `C-d` drops the field the cursor is actually *in*, so on the record node
   it is still the definition's — which is the only way to delete a
   definition whose whole body is a record.
   No new binding, no change to the key-hint line, and
   the reserve list keeps every character `{` did not take. `C-←`/`C-→` move
   the field the cursor is in one place earlier or later — the only genuinely
   new pair, and the only way to reorder anything from the keyboard, which
   the merge story needs to be real rather than an API-only capability. Like
   `C-p`, `S-Tab`, `Del`, `Esc` and `C-r`, they are documented here and not
   in the 80×12 hint line, which the lists entry warned was four characters
   from full and which now carries `{.` and nothing more.
   **(d)** The field slot is **one slot to the reader and two flavours to the
   implementation**, and the split is forced: a projection can itself be the
   value of a record's field, so the cursor alone cannot say which field the
   slot is about. The flavour therefore follows how the slot was *entered*,
   not where it sits. Both are labelled `field`, both are free-running name
   runs, and both leave on anything they do not understand.
   `←` from a field's value opens the **rename** flavour, where the buffer
   *is* the field's display
   name and every keystroke is `Rename(field, buffer)` — free text, byte for
   byte the binder-name slot, and the payoff of the design: the same
   keystroke renames the field at every construction site and every
   projection in the document, because they are all the same `Id`. `.` opens
   the **pick** flavour, where the buffer *selects* a field and every
   keystroke is `SetField(id)` — ranked by prefix over the fields of the
   record being projected, then over every other field in the document,
   which is the name run with a different candidate list. `=` leaves either
   flavour for the value, and anything the slot does not understand exits and
   is reprocessed, as everywhere else.
   **(e)** `.` is the fifth thing that can decline, and it declines for a
   reason no other key has: a field is an identity, and the keyboard cannot
   invent one. In a document with no record anywhere there is no field to
   name, so `.` says so rather than minting a field that belongs to nothing.
   The same argument is why `{` in an *annotation* slot is not a binding at
   all (`DECISIONS.md`, 2026-08-29): a record type is a list of field
   identities, a slot is free text, and a type that mints a fresh field on
   every keystroke of a commit-live run would name fields no record has.
   Record types are synthesised and displayed; they are not spelled.
   **(f)** `{` **reads the expectation before it mints anything**, which is
   the one place records differ from every other form key. A fresh field id
   is a fresh identity, and a fresh identity is never consistent with a
   record type that is already known, so a `{` that always minted one field
   would be quarantined the instant it landed in a position that knew which
   record it wanted — a definition with a record annotation, a field of
   another record, the hole `C-n` just made. So: where a record type is
   expected, `{` lays out *that* record's fields as holes and puts the cursor
   in the first (or, wrapping a value that fits the first field, keeps the
   value there and holes the rest); where none is, it mints one field, names
   it `f0`, and puts the cursor in the value. The keystroke is the same and
   the wrapping rule is the same; only what it writes reads the type. The
   reachability suite found this, not the design: `{` in an annotated
   position was unreachable until it did.
   Pinned by `keys::tests::a_brace_writes_a_record_and_lands_in_its_field_name`,
   `keys::tests::a_dot_projects_and_the_field_slot_picks_by_prefix`,
   `keys::tests::control_n_and_d_address_a_field_inside_a_record_and_a_definition_outside_one`,
   `keys::tests::renaming_a_field_renames_every_use_of_it_at_once`,
   `complete::tests::the_fields_of_the_record_being_projected_outrank_the_rest_of_the_document`,
   and the new `{` row of all eight columns of `tui/tests/matrix.rs` (the
   `.` row was already there and is unchanged, because none of the eight
   contexts contains a record for `.` to name a field of — which is item
   19(e) showing up in the matrix rather than being argued for).

Item 20 is dated **2026-08-29** and belongs to Phase B2's fourth and last
feature.

20. **Variants cost the last two reserved punctuation keys, and `C-n`/`C-d`
   generalise a third time.** A match is the first form whose *shape* the editor
   maintains rather than the user, so the keys had to be about constructors, not
   about arms.
   **(a)** `` ` `` is **inject**, the last character in the reserve list and the
   tag marker OCaml's polymorphic variants already spell it with. Reading down
   its row of the matrix, five of its eight cells said "no-op, hint"; those are
   the five that now build an injection, and no cell that already meant
   something changed. Like `{` it leaves the cursor in a *name* slot rather than
   in the payload, because the first thing you know about a case is what it is
   called; and like `{` it reads the expectation before it mints anything —
   where a variant type is expected it adopts that variant's first constructor,
   and only where none is expected does it mint `C0`. A fresh constructor is a
   fresh identity, and a fresh identity is never consistent with a variant the
   context already knows, so a `` ` `` that always minted would be quarantined
   the instant it landed anywhere that knew what it wanted (item 19(f), the same
   argument, found the same way).
   **(b)** `|` is **match**, taken from the reserve list because it is the
   character the projection itself prints between the arms. `m` was the obvious
   candidate and is rejected for the reason no letter is ever a verb here: `m`
   starts a name run, and `main` is the name every document has. `|` climbs at
   `PREC_BINDER` — a match extends as far right as its last arm's closing brace,
   which is to say it does not extend at all, being delimited — so in practice
   the climb rule never fires for it and `1 + 2` then `|` gives
   `match ⦇1 + 2⦈ {}`, wrapping the whole sum rather than just its right operand,
   which is what "match on this" has to mean — quarantined, because a number is
   not a variant and the editor says so rather than refusing the key. A
   scrutinee that *is* a variant is not parenthesised unless it is bigger than
   an atom: the scrutinee is projected at `PREC_ATOM`, which is what keeps
   `match e { … }` unambiguous when `e` is an application.
   **(c)** `C-n` and `C-d` **generalise a third time**, and the third reading is
   the one that finally names the rule: they add and drop one row of the
   innermost list the cursor is in — an arm inside a match, a field inside a
   record, a definition anywhere else. In a match `C-n` is `AddArm`, which is
   the *only* way to add a constructor to a variant, and it is deliberately not
   local: it appends a hole arm to the focused match **and to every other match
   in the document whose arm set is the same one**, in a single action and a
   single log entry, exactly as `C-d` on a field quarantines every projection of
   it (item 19(c), `DECISIONS.md`, 2026-08-29). That is what makes
   exhaustiveness hold by construction rather than by warning: the arm exists
   before the constructor it names can be injected anywhere. `C-d` in an arm is
   `RemoveArm`, the same sweep in reverse, and it is refused — with a hint —
   whenever any match's scrutinee still injects that constructor, because
   removing the arm would leave a match that could not answer.
   **(d)** The constructor slot is **one slot to the reader and two flavours to
   the implementation**, byte for byte the field slot's split (item 19(d)) and
   forced by the same fact: an injection can be an arm's body, so the cursor
   alone cannot say which constructor the slot is about. `←` from an arm's body
   opens the **rename** flavour — the buffer *is* the constructor's display name
   and every keystroke is `Rename(ctor, buffer)`, so one keystroke renames it at
   every injection and every arm in the document. `` ` `` opens the **pick**
   flavour — every keystroke is `SetConstructor(id)`, ranked over the
   constructors of the variant expected here, then over every other constructor
   in the document. `=` leaves either flavour for the payload or the arm's body.
   `SetConstructor` itself reads *both* positions — on an injection it re-aims
   the injection, and with the cursor in an arm it re-aims the arm — but only
   the injection reading has a key. Re-aiming an arm is how the protocol says
   "this case is that identity", which is `SetFieldId`'s job on a record and
   what makes a match with dead arms reachable at all
   (`action/tests/reachability.rs`); at the keyboard it would be a way to break
   a match by hand, and it is refused the moment it would
   (`exhaustive.rs::an_arm_can_be_re_aimed_at_another_case_but_never_off_one_the_scrutinee_injects`).
   **(e)** Nothing else was needed, and three candidates were talked out of.
   There is **no arm-reordering pair** to match `C-←`/`C-→` on fields: a record's
   field order is observable (it is the order the projection prints and the
   thing `ReorderFields` merges), and an arm's order is not — the arms of a
   match are looked up by constructor, so reordering them is a no-op the merge
   layer would have to invent a reason to care about. There is **no key for the
   arm's payload binder**, because `Tab` already walks into the arm bodies and
   the binder is named through the ordinary binder-name slot. And there is **no
   `SetArmBinderId` binding**: `AddArm` mints the binder, `Rename` names it, and
   the keyboard has never set an identity it did not mint.
   Pinned by `keys::tests::a_backtick_injects_and_lands_in_the_constructor_slot`,
   `keys::tests::a_bar_writes_a_match_with_one_arm_per_constructor`,
   `keys::tests::control_n_adds_an_arm_to_every_match_on_the_same_variant`,
   `keys::tests::renaming_a_constructor_renames_every_use_of_it_at_once`,
   `complete::tests::the_constructors_of_the_expected_variant_outrank_the_rest_of_the_document`,
   and the two new rows of all eight columns of `tui/tests/matrix.rs`.

Item 21 is dated **2026-08-29** and belongs to Phase B3, effects.

21. **Effects cost three keys, not four, and the fourth is a word you type.**
   `Cmd` adds four expression forms — `print e`, `readline`, `pure e` and
   `bind x <- c in k` — and exactly three of them can be keys. The rule that
   decides it was already in the implementation and this is the first feature to
   *use* it as a design constraint rather than discover it as a bug.
   **(a)** A name run commits live and re-commits on every keystroke, which
   `commit_run` implements as `Delete` then the candidate's action. That is only
   sound for a **leaf** construction: it assumes the cursor ends up on the thing
   it just built, with nothing wrapped around it. `nil`, `true`, `false` and
   every variable satisfy that. `print`, `pure` and `bind` do not — they wrap
   the focus, so re-committing them would nest a second one inside the first and
   the run would build `print (print (print …))` as you typed. So they must be
   keys. `readline` **is** a leaf: `Exp::Readline` has no children, `Delete` then
   `ConstructReadline` lands exactly where it started, and it becomes a
   completion candidate alongside `nil` — typed `r`, `re`, `readline`, ranked
   above the rest wherever a `Cmd Str` is expected. It costs no key at all,
   which is item 18(b)'s argument (`fold` had to be a key *because* it wraps)
   read in the other direction for the first time.
   **(b)** `$` is **print**. It is the character every shell puts in front of
   the line where output happens, it is not an operator on anything in this
   language, and it was in the reserve list. It climbs at app level like `/`,
   because `print` binds its argument the way `fst` does: on `1 + 2` with the
   cursor on the `2` it takes the `2` alone, not the sum. On a focused number it
   quarantines — `12` then `$` is `print ⦇12⦈` — because a number is not text
   and the editor says so rather than refusing the key. Two of the three
   quarantine in column **C** and one does not, and that asymmetry is the type
   rule showing through: `print` analyses its payload against `Str` and `bind`
   demands a command, so both wrap a number in a quarantine, while `pure`
   accepts anything and does not.
   **(c)** `'` is **pure**. Lisp's quote, for the same reason Lisp chose it:
   the thing after it is handed back as-is, not performed. Its cell in column C
   is the one place in the whole matrix where a form key wraps a focused `Num`
   *without* quarantining it, because `pure 1 : Cmd Num` is simply true. `'`
   could have gone unbound — `pure` is only strictly needed to end a chain with
   a value — but a form no keystroke can reach is a form that is not in the
   language, and the coverage rule below admits no exceptions.
   **(d)** `>` is **bind**, and it is the one key in this workspace that means
   two different things in two contexts *on purpose*. In the annotation slot it
   has been the arrow since Phase 2; on a node it now builds
   `bind x <- e in ⦇⦈`. Those never collide, because the annotation slot's
   alphabet is types and a node's alphabet is expressions, and the same
   character reading as "→" in one and "then" in the other is the same joke
   twice rather than an ambiguity: both are sequencing, one in the type and one
   in the term. It climbs at `PREC_BINDER` and wraps exactly as `;` does,
   because `bind` *is* `let` for commands and every difference would have to be
   defended: focus becomes the bound command, the body is a fresh hole, the
   binder is minted, and the cursor lands in the **binder-name slot** so the
   first thing you type is what the result is called. On a focus that is not a
   command it quarantines, so `12` then `>` is `bind x0 <- ⦇12⦈ in ⦇⦈`, and `=`
   leaves the name slot for the bound command exactly as it does on a `let`.
   **(e)** Nothing else was needed, and three candidates were talked out of.
   There is **no key for `seq`**, because there is no `seq` (`DECISIONS.md`,
   2026-08-29): a bind whose binder you never mention is the same six
   keystrokes and one fewer form. There is **no `run` key**: performing the
   program is a *command-line* verb, and a key that made the editor perform
   effects would be the exact footgun the phase was written to avoid — the
   editor shows you the `Cmd`, the terminal runs it, and no keystroke crosses
   that line. And there is **no dedicated key for the empty record** that
   `print` yields, even though B3 is the feature that made `{}` common: `{`
   already builds a record and `C-d` already empties it, and a second way to
   spell a value you mostly *read* rather than write is a binding spent on
   nothing.
   **(f)** The key-hint line is now exactly full. It was 157 columns of the 160
   that two 80-column rows allow; `$`, `'` and `>` join the form group and take
   it to 160 with nothing left over. The variants entry predicted that the next
   feature would have to drop a hint rather than shorten one, and B3 got in
   under that by three characters — which means the prediction now holds
   literally: the feature after this one has no columns at all, and must retire
   a hint to earn any. The three-row `keys_area` will wrap a longer line without
   clipping it, but a hint line that needs a third row is a hint line nobody
   reads.
   Pinned by `keys::tests::a_dollar_prints_and_lands_on_the_text`,
   `keys::tests::a_quote_purifies_a_number_without_quarantining_it`,
   `keys::tests::an_angle_binds_and_lands_in_the_binder_name_slot`,
   `keys::tests::an_angle_in_an_annotation_slot_is_still_an_arrow`,
   `annot::tests::a_c_is_the_command_prefix_and_takes_the_next_type`,
   `complete::tests::readline_is_a_candidate_and_outranks_the_rest_where_a_command_is_expected`,
   `render::tests::the_key_hint_line_exactly_fills_the_two_rows_it_is_given`,
   and the three new rows of all eight columns of `tui/tests/matrix.rs`.

Measured, replacing the predicted table's middle column (`tui/tests/keys/`,
one keystroke per line). The 2026-08-28 column is the definition era: the
programs are documents now, and `factorial` and `record` were rebuilt to say
what they always meant rather than what Phase 1 could express.

| # | program | Neovim | keystrokes | actions | ratio | was (2026-08-27) |
|---|---|---:|---:|---:|---:|---:|
| 1 | factorial | 84 | 28 | 33 | 0.33× | 16 / 0.19× |
| 2 | list_map | 114 | 44 | 53 | 0.39× | 29 / 0.25× |
| 3 | record | 65 | 50 | 43 | 0.77× | 33 / 0.51× |
| 4 | state_machine | 151 | 41 | 53 | 0.27× | 24 / 0.16× |
| 5 | nested_conditional | 146 | 31 | 42 | 0.21× | unchanged |
| 6 | greeting | 127 | 52 | 56 | 0.41× | new (2026-08-29) |
| 7 | greeting_command | 66 | 30 | 41 | 0.45× | new (2026-08-29) |

Row 2 changed on 2026-08-29 when lists arrived: the fixture had been map
over a *pair* since Phase 1 and is now a real map over a cons list, so the
number went **up** (`bench/RESULTS.md`, 2026-08-29 lists entry, and
`bench/references.md` §2). A ratio that rises because the program got
honest is the only kind of regression this table welcomes.

Row 3 changed on 2026-08-29 when records arrived, for the same reason: the
fixture had been a positional *pair* since Phase 1 and is now a real record
built in one definition and projected by name in another, so the number went
**up** again (`bench/RESULTS.md`, 2026-08-29 records entry, and
`bench/references.md` §3). Four of its extra keystrokes are worth naming
here because two of them are this document's business: the constructor is
the first definition now and so has to be named, and two are `up` presses
that walk the cursor out of the record before `C-n` means "new definition"
rather than "new field" (item 19(c)). The annotations got *shorter*, which
is not a saving — a record type is never spelled, so two positions the pair
fixture typed as `Num * Num` are holes.

Row 4 changed on 2026-08-29 when variants arrived, and it is the largest of
these upgrades: the fixture had faked a three-case state machine with a chain
of equality tests on the numeric codes 0/1/2 since Phase 0, and now writes the
match the reference text actually describes. 24 keystrokes became 41 and the
ratio went from 0.16× to 0.27×, which is the price of three cases that are
identities rather than magic numbers. 18 of the 41 are the three constructor
names, which no projection can make cheaper; the whole of the rest — the
lambda, the match, the three arms, the three injections and their `{}`
payloads — is 23. The state-machine projection reads a match now
(`state_machine::tests::recognizes_the_reference_state_machine`), and still
reads the old if-chain (`…::the_chain_of_equality_tests_is_still_a_state_machine`),
because a program someone wrote before variants existed does not stop being a
state machine.

Row 7 arrived with effects on 2026-08-29 and is the first reference program
that *does* anything rather than computing a value: `bind line <- readline in
print ("hello, " ++ line)`. Eight of its thirty keystrokes spell `readline`,
which is item 21(a)'s rule showing up as a number — a leaf is a completion
candidate and a candidate is typed. The three keys the feature did add
(`$`, `'`, `>`) cost one keystroke each, and the binder name and the greeting
literal are content no projection can make cheaper.

Row 6 arrived with strings on 2026-08-29 and is the first reference whose
cost is mostly *content*: 27 of its 52 keystrokes are the characters and
quotes of four string literals, which no projection can make cheaper. The
structure cost 25. The other five rows were byte-identical to 2026-08-28 when the string run
landed — it taxed nothing that existed.

`record` is still the number to watch, and it has now got worse twice for
good reasons: in the definition era its constructor became a named,
annotated top-level definition instead of a `let`, and with records its
fields acquired names the pair never had. The three unchanged rows are the
control — the definition bindings cost
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
