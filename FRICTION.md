# FRICTION.md — the dogfooding session

**Date: 2026-08-27.** Phase 4, checkbox 8 ("use the editor to build something
real, for at least two hours, without fixing it").

## How this session was run — a documented deviation

The spec asks for **two hours of a human using the editor**. I am an agent, so
the honest translation is an *intensive driven session against the real
binary*, not a simulation of one:

- `tmux new-session -d -x 220 -y 50 target/debug/nothing` — the actual
  `nothing` TUI, in a real terminal, in raw mode;
- every keystroke sent individually with `tmux send-keys`, and
  `tmux capture-pane` taken **after each one**, so every observation below is
  something that was on screen, not something inferred from the source;
- **~730 keystrokes**, of which 362 were captured frame-by-frame (the rest
  were bulk runs: a 200-key undo walk and two long arithmetic chains used to
  probe rendering);
- the terminal was resized mid-session (220×50 → 60×14 → 46×12) to see the
  editor at sizes a person actually works at;
- **worked from `KEYS.md` only.** `tui/tests/keys/*.keys`, the bench fixtures
  and the REPL were not consulted while building. Where I had to re-read
  `KEYS.md` to continue, that is itself recorded below as friction.

**Nothing was fixed.** No source file was touched during or after the session;
this document and the one Phase 4 checkbox are the whole output. The next
checkbox ("fix the top five friction points") is somebody else's.

### What was built

Five programs, none of them a reference program, all from an empty hole:

| | program | what it exercises |
|---|---|---|
| A | `let x1 = λx2:Num. λx3:Num. x2 + x2 * x3 in x1 1000 5` | compound-interest-ish arithmetic, `let`, precedence climbing, currying |
| B | `λx0:Num. λx1:Num. λx2:Num. if x0 < x1 then (if x1 < x2 then x2 else x1) else if x0 < x2 then x2 else x0` | three-way max, nested `if`, `Bool` positions |
| C | `λx4:Num * Bool. (snd x4, fst x4)` | pair swap, product annotations, `[` / `]` |
| D | `λx0:Num. λx0:?. x0 + 1` and a number scratchpad | deliberate mistakes: shadowing, capture, `~`, `Bksp`, `!`, undo/redo |
| E | `let x1 = λx2:Num. if x2 < 0 then 0 else x2 in let x3 = λx4:Num. λx5:Num. if x4 < x5 then x5 else x4 in x3 (x1 -5) (x1 7)` | clamp ∘ max, two `let`s, nested application in argument position |

Wrong turns were taken on purpose throughout: mistyped names, deleted and
retyped subtrees, undone the whole session, navigated long distances, changed
my mind about a program's shape halfway through.

---

## The friction points

Severity is 1 (papercut) – 5 (blocks real work).

> **2026-08-27 — five of these are fixed.** Phase 4's last checkbox is "fix
> the top five friction points". The five are **#7**, **#3**, **#12**, **#13**
> and **#10**, marked `**FIXED**` in place below with a one-line description of
> the fix; `KEYS.md` settled items 11–14 carry the grammar-level consequences
> and each fix is pinned by a test named after it. The benchmark was re-run
> after them and did not move: `bench/RESULTS.md` §"2026-08-27 — Phase 4,
> post-fix re-run".
>
> **Why those five, and not #2.** The ranking is severity first, then
> keystroke cost. #7 is the only entry where the editor silently changed the
> meaning of a program — a correctness bug, and a documented promise `KEYS.md`
> was not keeping — so it goes first regardless of anything else. #3 is the
> other sev-5 that Phase 4 owns: an editor that cannot show you the expression
> you are typing is not one. #12, #13 and #10 are the quarantine cluster: they
> are one confusion seen from three angles (navigation cannot see quarantines,
> the projection does not distinguish a wrapper from its contents, and
> repairing one costs navigation keystrokes), they were the most expensive
> frictions *in keystrokes* in the session, and they share a fix.
>
> **#2 (no persistence) is sev 5 and is deliberately not among the five.** It
> is the one entry the spec assigns to another phase — Phase 7 is "Persistence
> and the file format" — and the stopgap sketched below is not a keyboard-
> grammar fix but a file format, a CLI, and terminal I/O on the key path, which
> the Phase 4 architecture rule (the key handler is pure; the terminal loop is
> a shim) says not to add casually. Fixing it here would mean choosing the
> format Phase 7 exists to choose. It stays open, at the top of the list.

### 1. There is no way to start a new program — sev 3

**Trying to:** open the editor and write my own program.
**Happened:** `nothing` always opens on the factorial reference program.
There is no argument, no "new" key, no file to open. The first thing I did in
a two-hour session was press `↑` to the root and `Del` to destroy the sample.
Every later program cost the same up-up-up-`Del` preamble.
**Fix sketch:** `nothing` with no argument opens `⦇⦈`; `nothing --example
factorial` opens the sample. One line in `src/bin/nothing.rs` plus argument
parsing.

### 2. Nothing is ever saved, and `C-q` discards it without asking — sev 5

**Trying to:** end the session.
**Happened:** `C-q` exited instantly. The tmux session was gone and ~730
keystrokes of work with it. There is no save key (`C-s` → "`C-s` is not
bound"), no autosave, no scratch file, no "you have unsaved work" prompt, and
no way to get any program back out of the editor other than reading it off the
screen and retyping it. Phase 7 owns persistence, but this makes the phrase
"use the editor to build something real" self-defeating *today*: nothing real
survives the session.
**Fix sketch:** before Phase 7 lands, the cheapest honest stopgap is a
keystroke that dumps the recorded primitive action log (`AppState::actions()`)
to a `.actions` file — `script::replay_script` already reads it back. `C-s`
writes, `nothing <file.actions>` replays. Zero new formats.

### 3. Long programs are clipped, and the cursor scrolls off screen — sev 5 — **FIXED**

**FIXED 2026-08-27.** The editor wraps the projection itself
(`render::wrap_lines`) instead of handing it to `Paragraph`, so it knows which
line the cursor is on, and `render::scroll_offset` picks the window containing
that line (one line of context below it where there is one); the border title
reads `nothing · lines 5-11 of 23` whenever the program does not fit.
Pinned by `render::tests::the_cursor_is_on_screen_in_a_small_terminal`, which
types the session's own 40-term arithmetic chain in a 46×12 terminal and
asserts the cursor is on screen after every keystroke.


**Trying to:** keep typing an arithmetic chain in a 46×12 terminal.
**Happened:** the program overflowed the bordered box and was simply cut off
at the bottom border. The cursor `»7«` — the thing I was typing — was not on
screen at all, and kept not being on screen for the next 40 keystrokes. There
is no viewport, no scrolling, and no scroll-to-cursor. The editor is
structurally unable to edit a program larger than the box.
**Fix sketch:** the `Paragraph` already wraps; give it a scroll offset
computed so the marker line is inside the visible range. Requires laying the
program out to lines before rendering — which is also what point 4 needs, so
they are one job.

### 4. The projection is one long wrapped line — sev 4

**Trying to:** read program E (two `let`s and two nested lambdas) and find the
sub-expression I wanted to edit.
**Happened:**
`let x1 = (λx2:Num. if x2 < 0 then 0 else x2) in let x3 = (λx4:Num. λx5:Num. if x4 < x5 then x5 else x4) in x3 (x1 -5) (x1 7)`
on one line, soft-wrapped at the terminal width mid-expression (`… 1 + 1 +` /
`1 + 1 …`). No line break at `in`, at `then`/`else`, or after a `λ`; no
indentation; nothing to show nesting depth. Finding the else-branch of the
inner `if` meant counting parentheses in wrapped text, which is precisely the
thing a projectional editor exists to stop me doing.
**Fix sketch:** `core::render` grows a layout pass (or the TUI gets a second
renderer over the same tree) emitting a line list with indentation and break
points at binder and conditional boundaries; the cursor markers then anchor a
line, not a column.

### 5. A typed name is thrown away when the binder is created — sev 3

**Trying to:** write `let interest = …` by typing `interest` at a hole and
then `;`, exactly as `KEYS.md` §"Literal entry" promises ("Pressing `\` or `;`
next uses the buffer as the new binder's name, which is what you meant").
**Happened:** eight keystrokes of `interest` reported "unresolved · no name in
scope starts with `interest`" (fine, expected), then `;` created
`let »x1« = ⦇⦈ in ⦇⦈` and carried the buffer into the slot — but the binder is
`x1`, a fresh id, and the name I typed is not used for anything. I then had to
type `1` on top of it. The document promises a behaviour the editor does not
have.
**Fix sketch:** either implement it pre-Phase-5 by taking the digits out of
the buffer as the id, or — better — mark the promise in `KEYS.md` as blocked
on the Phase 5 name table so the document stops over-promising.

### 6. Fresh binder ids are unpredictable, so naming is a guess-then-correct loop — sev 3

**Trying to:** name three lambda parameters `x0`, `x1`, `x2`.
**Happened:** `\` produced `λ»x4«`, then `λ»x6«`, then `λ»x7«`, then `λ»x8«` —
the `Fresh` counter keeps counting through everything I deleted, so the
offered name depends on session history rather than on the program. Twice I
typed `x` `2` onto a binder that was *already* `x2`, spending two keystrokes
to change nothing; once I typed `x2` onto `x6` and silently reused an id that
a previous program had used.
**Fix sketch:** offer the lowest id not in scope at the cursor, not the global
next. It is a one-line change to how the slot seeds itself and makes "type the
name you meant" a no-op most of the time.

### 7. Renaming a binder onto an id already in scope captures references, silently — sev 5 — **FIXED**

**FIXED 2026-08-27.** `AppState::rename_conflict` counts the free occurrences
of the incoming id in the binder's body before `SetBinderId` is offered to the
calculus, and the binder slot declines with the count on the status line —
"x0 is already in scope here — naming this binder x0 would capture 1
reference" — leaving the slot open for another id. The symmetric case (the
body's references escaping outwards to an outer binder of the same id) is
declined too; orphaning was already declined by the calculus, and now says so
in its own words. Pinned by
`keys::tests::renaming_a_binder_onto_an_id_already_in_scope_is_declined` plus
the two negative cases that keep the identity rename (`x0` onto `x0`, which
every reference fixture types) and a genuinely fresh id working.


**Trying to:** deliberately shadow — `λx0:Num. λx0:?. x0 + 1`.
**Happened:** typing `x` `0` in the inner binder-name slot renamed the inner
binder to id 0 and the body's `x0` **silently re-bound to the inner binder**.
No warning, nothing on the status line, nothing in the projection.
`KEYS.md` §"Which keys can decline" states this case explicitly: *"binder slot
(`SetBinderId`) — the identity would capture or orphan a reference — warned
live, before the keystroke lands."* It is not warned, and it is not declined.
The capture only became visible three keystrokes later, when `b` in the
annotation slot was refused with "`Bool` would leave the body untypable" — a
message about the *annotation*, for a problem caused by the *rename*.
**Fix sketch:** `SetBinderId` should compute the set of `Var(id)` occurrences
in the binder's body that currently resolve outside it; if renaming would
recapture any, decline with a hint naming the count. The typing machinery
already has everything needed (`ctx_at` over the body).

### 8. Two binders with the same id are indistinguishable on screen — sev 3 — **no longer reachable from the keyboard (#7's fix)**

**2026-08-27:** the state this describes can no longer be *created* by typing,
because the capturing rename that created it is declined (#7). A program that
already contains two binders with one id — loaded, or built by the action
calculus — still renders them identically, so the point stands for Phase 5's
disambiguation; it is no longer something the editor can walk you into.


**Trying to:** understand `λx0:Num. λx0:?. x0 + 1` after point 7 happened.
**Happened:** both binders render `x0`, the reference renders `x0`, and the
only way to discover which one the reference points at was to type `x` at it
and read the candidate list — which offered exactly one entry, `‹x0:?›`. The
outer `x0:Num` binder was **completely unreachable from the keyboard**:
filtered by prefix it does not appear, so there is no keystroke that produces
a reference to it. A binder you cannot refer to is a binder you cannot edit
your way out of.
**Fix sketch:** Phase 5's disambiguation (a subscript or a dimmed id suffix on
the second occurrence of a display name) fixes the rendering; the candidate
list should list shadowed binders too, marked as shadowed, rather than
deduplicating by name.

### 9. Typing a comparison operand-first quarantines it — sev 4

**Trying to:** write `if x1 < x2 then …` by typing `x` `1` `<` `x` `2`, i.e.
in reading order.
**Happened:** the moment `x1` landed in the scrutinee it became
`if »⦇x2⦈« then …` — a `Num` where a `Bool` is expected, correctly
quarantined. It happened **twice in program B and once in program E** before I
internalised the workaround: press the operator *first* (`<` at the empty hole
gives `»⦇⦈« < ⦇⦈`), then the operands. Same keystroke count, completely
different mental model, and nothing in the editor suggests it. This is the
friction point the previous agent already flagged in `state_machine.keys`; it
reproduces immediately in ordinary use.
**Fix sketch:** when the expected type is `Bool` and the committed leaf
synthesises `Num`, the status line should say so in the imperative — "expected
Bool: type `<` first, then the operands" — rather than only marking the
quarantine. A real fix (deferring the quarantine for one keystroke while an
operator could still arrive) was rejected in `KEYS.md` §Rejected as
speculative replay, and I agree; the cheap fix is the hint.

### 10. Repairing a quarantine costs three navigation keystrokes and requires knowing exactly where to stand — sev 4 — **FIXED**

**FIXED 2026-08-27.** `Enter` now finishes the quarantine the cursor is
*inside* as well as the one it is on (expanding to `MoveParent`s + `Finish`,
so one `C-z` still undoes one key), and the status line answers from in there
too: `inside ⦇⦈ · fits now — press Enter`. From inside a quarantine that does
*not* fit, `Enter` says what was expected instead of teleporting to an
unrelated hole. Pinned by
`keys::tests::enter_finishes_the_quarantine_the_cursor_is_inside`, which also
asserts the repair is exactly one keystroke cheaper than walking out to the
wrapper, and by
`keys::tests::enter_inside_a_quarantine_that_does_not_fit_says_so_instead_of_jumping`.


**Trying to:** clear `⦇x1 < x2⦈` once the contents had become a `Bool` and
therefore fit.
**Happened:** with the cursor *inside* the wrapper — on `x2`, where the
keystroke that finished the expression left it — the status line said
`node · expects Num · variable`. Nothing indicated a quarantine anywhere
nearby, and `Enter` did not finish it (see point 11). Only after `↑` `↓` onto
the wrapper itself did the status line read
`quarantined ⦇e⦈ · fits now — press Enter`. Three keystrokes and a
re-read of `KEYS.md` to undo something the editor already knew was fixed.
**Fix sketch:** either finish automatically the moment the contents fit (the
information is free — the check is already run for the status line), or show
"⦇⦈ fits now — Enter" on the status line from *inside* the wrapper too, with
`Enter` from within meaning "finish the enclosing hole".

### 11. `Enter` means two different things and picks silently — sev 3 — **partly addressed by #10's fix**

**2026-08-27:** the silent pick is gone from the case that caused the damage.
`Enter` on or inside a quarantine is always `Finish`, and when it cannot
finish it says why rather than teleporting; the jump-to-next-hole meaning now
applies only where there is no quarantine in sight. Whether the two meanings
should be split onto two keys is still open, and is a grammar change rather
than a repair.


**Trying to:** finish a quarantine.
**Happened:** `Enter` is `Finish` when the cursor is exactly on a
`NonEmptyHole` and "jump to the next empty hole" everywhere else. Standing one
node off, my `Enter` teleported the cursor to an unrelated hole halfway across
the program and I did not notice for two keystrokes. A key that does either of
two unrelated things depending on a distinction the cursor rendering does not
make (point 13) is worse than two keys.
**Fix sketch:** `Enter` = `Finish` only, with a hint when there is nothing to
finish; move "next empty hole" onto `Tab`, which already is that.

### 12. Hole navigation cannot see quarantines, and the status line denies they exist — sev 4 — **FIXED**

**FIXED 2026-08-27.** `Tab`/`S-Tab` walk both kinds of hole in the same source
order, so a leftover `⦇e⦈` is a stop rather than a skip; with nowhere left to
go they say "nothing unfinished: this program has no holes", which is now
true when it is said. The status line carries a running count — `· 2
quarantined` — so "am I done?" is answered without walking the tree. Pinned by
`movement::tab_reaches_every_hole` (whose `1 + ⦇true⦈` case used to assert
nothing at all, because that program has no empty holes),
`keys::tests::tab_walks_quarantines_too_and_says_when_nothing_is_left` and
`render::tests::the_status_line_counts_the_quarantines_left`.


**Trying to:** find the remaining problems in program B before calling it
done.
**Happened:** `Tab`/`S-Tab` walk *empty* holes only. A leftover `⦇x0 < x2⦈`
was skipped every time, and when no empty holes remained `Enter` reported
**"no empty hole in this program"** while two non-empty holes were on screen.
The editor's own summary of "is this program finished?" ignores the exact
construct that means "this program is not finished". In program E I carried
two nested quarantines for six keystrokes without noticing.
**Fix sketch:** `Tab` visits non-empty holes as well (they are holes); or add
`S-Tab`'s sibling — a "next unfinished thing" key — and change the message to
"no empty holes; 2 quarantined expressions remain".

### 13. The cursor markers do not distinguish a wrapper from its contents — sev 4 — **FIXED**

**FIXED 2026-08-27.** The focus is drawn as a highlighted span (reverse video
across everything between the markers, markers included), so `»⦇e⦈«` lights the
quarantine brackets and `⦇»e«⦈` does not — the difference is now the whole
wrapper rather than two characters' worth of bracket placement. The status
line says `inside ⦇⦈` from the contents (see #10). This is also the honest fix
for #19, which is about the same invisibility seen from the other end: a
whole-program focus now *looks* like a whole-program focus before a letter
replaces it. Pinned by
`render::tests::the_quarantine_wrapper_and_its_contents_look_different` and
`render::tests::the_whole_focus_is_highlighted_not_just_its_markers`, both of
which read the `TestBackend` cell *styles*, not just its text.


**Trying to:** stand on the quarantine wrapper in program E.
**Happened:** `x3 ⦇»x1 -5«⦈` (cursor on the *contents*) and
`x3 »⦇x1 -5⦈«` (cursor on the *wrapper*) differ by the position of two
brackets in a 120-character line. I misread it, pressed `Enter` (got "no empty
hole in this program"), then `space`, and built **six keystrokes of garbage**
— `x3 ⦇⦇x1 -5⦈ x1 7⦈` instead of `x3 (x1 -5) (x1 7)` — inside a wrapper I
thought I had already left. The repair was `↑` `Del` and retyping the whole
argument.
**Fix sketch:** the status line already knows (`quarantined ⦇e⦈` appears only
on the wrapper); the *projection* should too — colour or restyle the wrapper
brackets when they are the focus, rather than relying on marker placement.

### 14. A nested application in argument position cannot be typed left to right — sev 4

**Trying to:** write `x3 (x1 -5)`.
**Happened:** `x` `3` `space` gives `x3 »⦇⦈«`; typing `x1` there and then
`space` **climbs out** (the argument is the rightmost child of the `Ap`, and
`Ap` precedence ≥ `space`'s), producing `x3 x1 ⦇⦈` — a second argument to
`x3`, not an application of `x1`. Left-to-right typing cannot express the
grouping. The working recipe is inside-out: `space` *first* (giving
`(»⦇⦈« ⦇⦈)`), then the head, then `Tab`, then the argument. Same keystroke
count; a completely non-obvious order, and I had to re-read the climbing
section of `KEYS.md` to work out why.
**Fix sketch:** this is the honest cost of left-associativity and I do not
think the climb rule is wrong. The cheap mitigation is discoverability: when
`space` is about to climb, say so — "space applies `x3` again; press `space`
at an empty hole first to group". A `(` key that means "apply the focus to a
fresh argument without climbing" is the other option and `(` is unbound.

### 15. Re-associating an expression is impossible without retyping it — sev 3

**Trying to:** change `x2 + x2 * x3` into `(x2 + x2) * x3` after deciding
program A's formula was wrong.
**Happened:** the tree is `+(x2, *(x2, x3))`; the thing I want to wrap,
`x2 + x2`, is not a subtree, so no cursor position selects it. Standing on the
`+` node and pressing `*` climbs nowhere (the parent is a `Lam` frame) and
wraps the whole thing: `(x2 + x2 * x3) * »⦇⦈«`. The only route is `Del` and
retype. In a text editor this is two parentheses.
**Fix sketch:** a rotate/re-associate action at the calculus level
(`BinOp(a, BinOp(b, c))` ⇄ `BinOp(BinOp(a, b), c)` where the operators allow)
would be one keystroke and is well-typedness-preserving. That is a Phase 2
addition, not a keybinding change, so it needs a `DECISIONS.md` entry.

### 16. Reaching a binder's body costs three keystrokes — sev 3

**Trying to:** get from a focused `λ` to the expression inside it.
**Happened:** `↓` lands on the binder **name** slot, `→` moves to the
annotation slot, `→` again reaches the body. Three keys per lambda; program E
has four nested lambdas inside two `let`s, so re-entering the innermost body
from the root is eleven keystrokes of pure movement. `.` — the key that means
"go to the body" from inside a slot — is refused on a focused `λ` (point 17),
which is exactly where I kept pressing it.
**Fix sketch:** `.` on a focused `Lam`/`Let` node moves to the body. It is
free (currently a no-op there), it is the same meaning the key already has one
slot deeper, and it makes deep navigation two keys instead of three.

### 17. The `.` hint on a lambda is wrong — sev 2

**Trying to:** press `.` on `»λx0:Num. …«`.
**Happened:** status line: "`.` addresses a binder's body; the cursor is not
on a binder". The cursor is *on a binder* — a lambda is the binder. What the
message means is "the cursor is not in a binder slot". I read it three times
before deciding the editor was wrong rather than me.
**Fix sketch:** reword to "`.` moves from a binder slot to the body; press `↓`
first". Or implement point 16 and the message becomes unnecessary.

### 18. `=` means two unrelated things in two identical-looking slots — sev 3

**Trying to:** press `=` in a lambda's binder-name slot (out of habit from the
`let` I had just typed, where `=` moves to the bound expression).
**Happened:** on a `λ` the slot does not understand `=`, so it exits to the
body and reprocesses — wrapping the **entire body** in an equality:
`λx9:Num -> Num. (if ⦇⦈ then ⦇⦈ else ⦇⦈) == »⦇⦈«`. "Exit and reprocess" is a
good rule, but here it turns a wrong-context navigation key into a structural
edit of the largest subtree in reach, and `λ»x9«` and `let »x9«` look
identical on screen apart from the keyword.
**Fix sketch:** `=` in a `Lam` binder-name slot should be inert with a hint
("`=` names a `let`'s value; press `.` for the body"), the way `:` is inert
inside the annotation slot. This is the one place where "never refuse" costs
more than it buys.

### 19. One letter on a focused expression destroys the whole subtree — sev 4 — **partly addressed by #13's fix**

**2026-08-27:** the focus is now a highlighted span, which is exactly the fix
sketched below, so the stakes are visible before the letter lands. The rule is
unchanged and the point stays open at reduced severity: nobody has re-run the
session to say whether seeing the selection is enough.


**Trying to:** type `f` for `false` — with the cursor sitting on the root.
**Happened:** `»λx9:Num -> Num. ⦇⦈«` became `»false«`. The entire program,
gone, on one unmodified letter, with no confirmation and no indication
beforehand that the "selection" was the whole document. `C-z` gets it back and
the behaviour is exactly what "typing replaces the selection" promises — but
in a text editor the analogous keystroke replaces one character, and the size
of what is about to be destroyed is invisible until it is.
**Fix sketch:** don't change the rule; make the stakes visible. Render the
focus with a background highlight rather than two thin markers, so a
whole-program focus *looks* like a whole-program focus. (This helps point 13
too.)

### 20. A mistyped character mid-run withdraws the commit and leaves a hole — sev 2

**Trying to:** type `false`, fumbled to `falq`.
**Happened:** the committed `false` was withdrawn and the focus reverted to
`»⦇⦈«` with "unresolved · no name in scope starts with `falq`". Correct per
`KEYS.md` §Settled item 6, and one `Bksp` fixes it, but the program visibly
losing a value I had already written because of one bad character is
startling, and the status line does not say "press Bksp".
**Fix sketch:** on transition to unresolved, add "· Bksp to go back" to the
hint. Cheap, and it turns a surprise into an instruction.

### 21. There is no help, and the key strip is truncated when the terminal is small — sev 3

**Trying to:** remember whether `[` was `fst` or `snd`.
**Happened:** I alt-tabbed to `KEYS.md`. There is no help key — and `?` is
`if`, so the obvious one is spent. The bottom key strip is the only in-app
reference, and at 46 columns it renders as two lines ending "`… Enter fit`",
with `C-z undo · C-q quit` **cut off entirely**: the narrower the terminal,
the less of the help survives, and quit is the last thing to fit.
**Fix sketch:** `F1` (unbound) opens a full-screen cheat sheet — the `KEYS.md`
one-screen table is already written and already fits on one screen. And the
strip should shed items from the *middle*, keeping quit and undo.

### 22. The undo history has no visible extent and no landmarks — sev 2

**Trying to:** get back to a program I had abandoned three programs ago.
**Happened:** 200 × `C-z` walked back through the entire session — across
program E, program D, program C, the deletions between them — and stopped at
the factorial the editor booted with, reporting "nothing to undo". At no point
did the editor say where in history I was, how far back it went, or that I had
just crossed the boundary between two different programs. Then one new
keystroke discarded the whole redo tail, permanently.
**Fix sketch:** status line shows `undo 47/112`. Discarding a redo tail longer
than a few keystrokes deserves a one-line notice ("112 redo steps discarded").

### 23. `>` is unbound with no guidance — sev 2

**Trying to:** write `x > 0`.
**Happened:** "`>` is not bound here". Correct — `KEYS.md` §Rejected item 3
explains at length why `>` cannot be a reversed `<` and says "type `0 < x`" —
but the editor does not, and `>` *is* bound one slot away (in an annotation it
is the arrow). I looked it up.
**Fix sketch:** hint "`>` is not bound; write `0 < x`, or `>` in an annotation
slot is `->`". The rejection was reasoned; the reasoning should reach the
person who needs it.

### 24. Deep navigation has no shortcuts at all — sev 3

**Trying to:** clear the program and start the next one.
**Happened:** six consecutive `↑` presses to reach the root, then `Del`. There
is no go-to-root, no go-to-enclosing-binder, no jump-to-sibling-*n*, no
history of visited positions. Movement is strictly one step at a time in four
directions, and `↑` past the root reports "already at the root" rather than
doing anything useful. In program E, moving between the two `let` bodies was
seven keystrokes each way.
**Fix sketch:** `Home` = root (unbound and idiomatic); `S-↑` = ascend to the
nearest binder. `KEYS.md` §Rejected item 4 rules out *numeric counts*, which
is a different thing from named landmarks.

### 25. You cannot see what is in scope without typing a letter first — sev 2

**Trying to:** remember which of `x1`…`x5` was the clamp and which was the max
in program E.
**Happened:** the candidate list is excellent — `‹x3:Num -> Num -> Num›
x1:Num -> Num` with types and `✗` markers — but it only appears *during a name
run*. To see what is in scope I had to type `x`, read the list, and then press
`Esc`, which committed a variable I did not want and had to be undone. There
is no passive "what's in scope here" view.
**Fix sketch:** show the context on the status line (or a side panel) whenever
the focus is an empty hole, before any letter is typed. The data is already
computed for `expected_ty_at`.

### 26. The program pane is 95% empty and shows nothing but the program — sev 1

**Trying to:** use a 220×50 terminal.
**Happened:** a one-line program sits at the top of a 44-row bordered box.
Nothing else is displayed: not the program's type, not the action count, not
the number of holes, not values (Phase 6). The most valuable screen real
estate in the editor is blank.
**Fix sketch:** once Phase 6 lands this is where live values go. Until then,
the synthesised type of the whole program and a hole count are free and would
have answered "am I done?" — the question points 12 and 22 both left me asking.

### 27. `Bksp` on a one-digit negative number loses the sign — sev 1

**Trying to:** turn `-105` back into `-1` and then into `-9`.
**Happened:** `Bksp` `Bksp` gave `-1` as expected, and the next `Bksp` gave
`»⦇⦈«` — sign and all. Consistent with "one digit left ⇒ `Delete`", but the
sign is not a digit and I had to press `~` again afterwards.
**Fix sketch:** `Bksp` on a single-digit negative gives `Num(0)`-with-sign, or
more simply leaves an empty hole *and* remembers nothing — i.e. accept it, but
say "sign cleared" so it is not a surprise.

---

## Summary

27 friction points; the spec asks for fifteen.

**Status, 2026-08-27:** five fixed — **3**, **7**, **10**, **12**, **13**;
three more materially reduced by those fixes without being claimed as fixed —
**8** (no longer reachable by typing), **11** (the damaging half is gone),
**19** (the stakes are visible now). Nineteen open, of which **2** (no
persistence) is the only remaining sev 5 and belongs to Phase 7.

By severity: **5** — 2 (no persistence), 3 (clipping), 7 (silent capture);
**4** — 4 (no layout), 9 (Bool-position trap), 10 (quarantine repair), 12
(hole navigation blind to quarantines), 13 (wrapper vs contents), 14 (nested
application), 19 (one letter destroys the program); **3** — 1, 5, 6, 8, 11,
15, 16, 18, 21, 24; **2** — 17, 20, 22, 23, 25; **1** — 26, 27.

Three observations that are not individually friction points but shape the
list:

- **The calculus itself never got in the way.** Not once did the editor refuse
  an edit, lose well-typedness, or leave a state I could not type my way out
  of. Every point above is about *feedback, navigation, or rendering* — which
  is the outcome the design commitments predicted, and is worth recording as
  evidence for them.
- **The keystroke counts held up.** Program C (pair swap) was 15 keystrokes,
  clean. Program B (three-way max) took 61, of which about 15 were repairing
  the two quarantines of point 9 and one mis-navigation. Program E took about
  67, of which 16 were the wrong turn of point 13 that had to be deleted and
  retyped. Nothing here suggests the 3× guard is in danger — the cost of these
  frictions is measured mostly in *re-reads of `KEYS.md`*, and the keystrokes
  they do cost are concentrated in points 9, 10, 13 and 14.
- **The two worst problems are the two things Phase 4 does not own.**
  Persistence is Phase 7 and layout is nobody's yet. They should be pulled
  forward, or the phrase "build something real" should be honest that today it
  means "build something real and then watch it disappear".
