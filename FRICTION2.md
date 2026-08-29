# FRICTION2.md — the second dogfooding session

**Date: 2026-08-29.** Phase B4, checkbox 4 ("build a real program against the
standard library, through the product interfaces, for at least two hours,
without fixing anything").

## How this session was run — a documented deviation

The spec asks for **two hours of a person building a real program against the
stdlib**. I am an agent, so the honest translation is the same one
`FRICTION.md` made: an intensive driven session against the real binary,
through the real interfaces, with nothing simulated and nothing reconstructed
in the harness.

- **`nothing protocol` over stdio** was the main surface — the JSON agent
  protocol, one request per line, the real `target/debug/nothing` process.
  State between batches was carried by replaying the accumulated step list
  through the `script` method, which is exactly what the action calculus
  promises; the harness never rebuilt a document itself.
- **329 steps attempted, 286 accepted** and in the saved log; 43 were
  rejected, no-ops, or undone. Every attempt, including every failure, went
  through the binary and is in the session transcript.
- **`nothing edit` in a real terminal**, driven with `tmux send-keys` at 120
  and 240 columns, to see the completion UI show a stdlib doc line.
- **`nothing run` six times against real piped stdin**, `nothing check`
  twice, `nothing doc` twice.
- **Worked from `nothing doc`, `--help` and the protocol's own `help` only.**
  The stdlib source, `KEYS.md` and the test fixtures were not consulted while
  building. Where I had to guess a function name and guess wrong, that is
  recorded below as friction.

**Nothing was fixed during the session.** No source file was touched between
the first step and the last. This document is the whole output; the next
checkbox ("fix the top five") comes after it.

### What was built

`todo.n` — a todo triage report. Seven definitions, all documented, all
well-typed, built from the empty program in 286 actions:

| | definition | type |
|---|---|---|
| | `main` | `Cmd ?` |
| | `todo_kept` | `Str -> Bool` |
| | `todo_bullet` | `Str -> Str` |
| | `todo_bar` | `List Str -> Str` |
| | `todo_status` | `List Str -> Str` |
| | `todo_report` | `List Str -> List Str` |
| | `todo_headline` | `List Str -> Str` |

```
main : Cmd ? =
  bind a <- ask "todo 1: " in bind b <- ask "todo 2: " in bind c <- ask "todo 3: " in
  let raw = a :: b :: c :: nil in
  bind ignored <- print "your list:" in
  bind shown   <- print_all (todo_report raw) in
  bind listed  <- print_labelled "kept" (todo_bar raw) in
  bind said    <- print (todo_status raw) in
  print_labelled "top" (todo_headline raw)
```

It reads three lines, builds a list, filters the blanks out, maps a bullet
over what is left, folds the rest into a report, and prints it. It uses
thirteen stdlib definitions — `not`, `is_blank`, `count`, `map`, `filter`,
`all`, `any`, `take`, `join`, `repeat_str`, `ask`, `print_all`,
`print_labelled` — and evaluates correctly on three different inputs:

```
$ printf 'buy milk\n\nwrite FRICTION2\n' | nothing run todo.n
your list:
- buy milk
- write FRICTION2
kept: **
some blanks
top: buy milk, write FRICTION2
```

Wrong turns were taken throughout and none of them were pre-planned: a
curried call written one argument short, a list built front-to-back that
quarantined every element, a fourteen-step subtree destroyed by a movement
that silently did nothing, and a helper that counted the wrong list and had
to be rewritten after `nothing run` printed three stars where two belonged.

---

## The friction points

Severity is 1 (papercut) – 5 (blocks real work).

> **2026-08-29 — five of these are fixed.** Phase B4's last checkbox is "fix
> the top five friction points". The five are **#1**, **#2**, **#3**,
> **#5** (with **#6**, which shares its fix) and **#9**, marked `**FIXED**`
> in place below with a one-line description of the fix; each is pinned by a
> test named after it. The benchmark was re-run after them and did not move:
> `bench/RESULTS.md` §"2026-08-29 — Phase B4".
>
> **Why those five, and not #4 or #11.** The ranking is severity first, then
> keystroke cost. #1 is the only entry where the editor silently destroyed a
> program and called it success — a correctness bug in the interface the
> whole session ran through — so it goes first regardless of anything else.
> #2 is a bug this phase itself introduced, in this phase's own deliverable:
> `nothing doc` was unreadable for exactly the documents the standard
> library exists to make possible. #3 is the difference between a session
> that fits in an agent's context and one that does not, and it is
> self-contained. #5 and #6 are one confusion seen from two angles — nothing
> can find a hole and nothing says where one is — and they share a fix. #9
> is a promise this phase made ("completion displays the doc") that was only
> true at 240 columns.
>
> **#4 (list building quarantines every element) and #7 (a curried call
> quarantines its head) are the most expensive frictions in the session in
> raw steps, and are deliberately not fixed here.** Both fixes mean changing
> what `ConstructCons` and `ConstructAp` *do* — folding a `Finish` into
> another action — and that is the action calculus, not the interface around
> it. The phase rules say not to change the calculus casually, and a friction
> pass is the definition of casually. They are the first thing the next
> phase should look at, and the fix sketches below are what to look at.
>
> **#11 (there is no `Num -> Str`) is ranked sev 4 and is not a fix this
> phase can make.** It is a primitive: a node tag, a `Ty` rule, an eval rule,
> a format version. B4 owns the standard library, and this is precisely the
> thing that cannot be written in the standard library. It is written down
> here, and in `DECISIONS.md`, as the largest single gap between this
> language and a program a person would keep.

### 1. The protocol answers `ok: true` for an action it did not apply — sev 5 — **FIXED**

**FIXED 2026-08-29.** `ok` is now false whenever a request did not do what
it asked: an action that does not apply, a step in a `script` that stops the
run, an `undo` with nothing to undo, a `redo` with nothing to redo. `applied`
stays as the finer-grained field and the error text is unchanged. Pinned by
`protocol::tests::an_action_that_does_not_apply_answers_not_ok` and
`protocol::tests::every_failure_answers_not_ok_whatever_shape_it_takes`,
which sweeps all five failure shapes and asserts each carries `ok: false` and
an error.


**Trying to:** move from the condition of an `if` to its then-branch with
`move-parent` then `move-next-sibling`, and keep typing.
**Happened:** `move-parent` reached the `if` itself — the root of the
definition body — and `move-next-sibling` from there has nowhere to go. The
protocol replied:

```json
{"ok": true, "applied": false, "error": "the action does not apply at this cursor"}
```

`ok: true` **and** an error string in the same object. The cursor had not
moved, so the next step, `construct-str "all good"`, overwrote the entire
`if all todo_kept xs then ⦇⦈ else ⦇⦈` I had just built. Fourteen steps of
work, gone, silently, and reported as success. It then happened a **second**
time in the same batch at the lambda root, destroying the whole application.
Twenty steps had to be undone one at a time to get back.

The rule is inconsistent, which is what makes it a trap: `construct-var zzz`
against an unknown name returns `ok: false`, but a movement off the end of
the tree returns `ok: true`. The single most common failure in a driven
session is the one dressed as success. A client that keys on `ok` — which is
what `ok` is for — is wrong exactly when it matters.
**Fix sketch:** an action that did not apply is not ok. Return `ok: false`
with the same error text, the way a name that does not resolve already does.
`applied` stays as the finer-grained field.

### 2. `nothing doc <file>` prints stdlib references as raw ids — sev 4 — **FIXED**

**FIXED 2026-08-29.** `doc_cmd::render_reference` renders through
`prelude().names_for(&doc.names)`, the same layered table `nothing run` and
`nothing check` already used. Pinned by
`cli/tests/stdlib.rs::doc_names_the_stdlib_functions_a_document_calls_rather_than_their_ids`,
which asserts a borrowed name renders as a name and that no definition
renders as a raw id. `stdlib/REFERENCE.md` is byte-identical after the
change, so the regeneration check is unaffected.


**Trying to:** read back the reference for the program I had just written.
**Happened:**

```
main : Cmd ? = bind a <- _d4648d54 "todo 1: " in … bind shown <- _6acc27d5 (todo_report raw) in …
In words: _d4648d54 applied to the text "todo 1: ", …
```

`ask` is `_d4648d54`, `print_all` is `_6acc27d5`, `not` is `_e75a816c`. Every
stdlib call in my own document rendered as a hex id. `nothing run` and
`nothing check` layer the prelude's names over the document's before
rendering; `nothing doc` renders with the document's own name table alone,
which by design does not contain a single stdlib name. The output is
unreadable for exactly the programs the stdlib exists to make possible.
**Fix sketch:** `doc_cmd::run` builds `prelude().names_for(&document.names)`
and renders through that, as `run_cmd` already does. Three lines.

### 3. Every protocol reply carries the whole standard library — sev 4 — **FIXED**

**FIXED 2026-08-29.** `state.names` and `state.docs` now carry the
document's **own** layer plus only the prelude ids the document actually
references, so every reply stays self-describing for the program at hand;
`state.stdlib` is gone and `state.stdlib_count` says how much more there is;
a new `stdlib` method hands over the catalogue once. An empty document's
`state` went from 17,555 bytes back to 838 — a 21× cut on every reply — and
`hole_context` from 40,588 to 23,871. Pinned by
`protocol::tests::state_carries_the_prelude_names_a_document_uses_and_no_others`
and `protocol::tests::the_stdlib_method_hands_over_the_whole_catalogue_once`.


**Trying to:** apply one step and read the result.
**Happened:** each reply is 17–52 KB. An empty document's `state` was 832
bytes before the stdlib and is 17,555 bytes after — a 21× growth on **every
single reply** — because `names`, `docs` and `stdlib` re-serialise all 37
definitions each time. `hole_context` went from 5,193 to 40,588 bytes. Over
329 steps that is roughly 8 MB of JSON, of which about 5.5 MB is the same
thirty-seven immutable entries repeated. For an agent with a context budget
this is the difference between a session that fits and one that does not.
**Fix sketch:** the stdlib is immutable for the life of the session, so send
it once. Either a `stdlib` method the client calls once, or a flag on the
session (`"stdlib": "omit"`) that drops the three arrays from every reply
after the first, or a content hash the client can use to skip re-reading.

### 4. Building a list front-to-back quarantines every element — sev 4

**Trying to:** write `a :: b :: c :: nil`.
**Happened:** `construct-cons` wraps the focus as the **head** and puts the
cursor on a fresh **tail** hole, which expects a list. So typing the next
element there is typing a `Str` into a `List Str` position, and it
quarantines — every time, by construction:

```
let x0 = a :: »⦇⦈« in ⦇⦈            construct-var b
let x0 = a :: »⦇b⦈« in ⦇⦈            ← quarantined the instant it was typed
let x0 = a :: ⦇b⦈ :: »⦇⦈« in ⦇⦈      construct-cons
let x0 = a :: ⦇b⦈ :: ⦇c⦈ :: »nil«    ← two quarantines for a three-element list
```

Every one of them fits perfectly once the *next* `construct-cons` arrives,
but nothing notices: they stay quarantined until I navigate back to each and
`finish` it by hand. A three-element list cost 8 steps to build and 7 more to
clean up. In a second list later in the session the *first* element
quarantined too, because the target position was already `List Str` rather
than `?` — so the cost is one repair per element, not per element after the
first.
**Fix sketch:** `construct-cons` at a hole whose expected type is `List T`
should offer to consume the element being typed as the head — or, cheaper and
more in the spirit of the calculus: when `construct-cons` wraps a quarantined
expression whose contents now fit the head position, unwrap it in the same
action. This is `Finish` applied automatically at the one moment the calculus
can prove it is safe.

### 5. There is no way to jump to the next hole or quarantine — sev 4 — **FIXED**

**FIXED 2026-08-29.** A `move_to_hole` protocol method, with
`{"forward": false}` for the other direction, walking to the next unfinished
node exactly the way the TUI's `Tab` does — and now literally by the same
code: `index_path` and `moves_between` moved from `tui::app` down into
`action::zipper`, and the TUI re-exports them. The moves it makes are
ordinary `MoveParent`/`MoveChild` actions in the log, not a hidden jump, so
undo and provenance see what they always saw. Pinned by
`protocol::tests::move_to_hole_walks_to_the_next_unfinished_thing_and_logs_ordinary_moves`,
which also asserts that a finished definition answers `ok: false` rather than
pretending.


**Trying to:** clean up the two quarantines in the list above.
**Happened:** the TUI has `Tab` and `S-Tab` for exactly this. The protocol
and the keyscript have nothing — no `move-next-hole`, no
`move-next-quarantine`, no `move-to-path`. The only way back to a quarantine
is to remember the shape of the tree and count `move-parent` / `move-child N`
steps, which is precisely the parenthesis-counting a projectional editor
exists to abolish. Six of my nine failed steps in this session were
navigation guesses.
**Fix sketch:** the keyscript grammar gains `move-next-hole` /
`move-prev-hole` (either kind, the way `Tab` already works) and
`move-next-quarantine`. `zipper::all_positions` and the TUI's hole walk
already exist; this is exposing them.

### 6. Nothing says *where* the holes and quarantines are — sev 3 — **FIXED**

**FIXED 2026-08-29**, with point 5. `state` gained `holes` — an array of
cursor paths, one per unfinished node in the current definition — and
`document_empty_holes` / `document_non_empty_holes` beside the existing
per-definition counts, so a document with a hole somewhere else no longer
looks finished. Pinned by
`protocol::tests::state_says_where_the_holes_are_and_counts_the_whole_document`.


**Trying to:** find out what was still unfinished after a long batch.
**Happened:** `state` reports `empty_holes: 2` and `non_empty_holes: 1` — and
those counts are for the **current definition only**, which is not stated
anywhere. A document with a hole in another definition looks finished. And
even within the current definition, a count is not a location: I know there
is one quarantine and nothing about where.
**Fix sketch:** report both — `empty_holes` for the definition and
`document_empty_holes` for the whole thing — and add a `holes` array of
cursor paths, which is what a client actually needs and what
`move-next-hole` (point 5) would consume.

### 7. Writing a curried call outside-in quarantines the head — sev 3

**Trying to:** write `repeat_str (length xs) "*"`.
**Happened:** I wrote one `construct-ap` instead of two, filled in
`repeat_str`, and the head quarantined on the spot:

```
λxs:List Str. »⦇repeat_str⦈« ⦇⦈
```

`repeat_str : Num -> Str -> Str` in a position expecting `? -> Str`, which is
correct and reasonable — but the quarantine is a *prediction* that I am done
applying arguments, made at the moment I have applied none. When I added the
second `construct-ap` and the `"*"`, the quarantine stayed, stale, and I had
to navigate back and `finish` it. Adding an argument to a partial application
is the most ordinary edit there is; it should not leave debris.
**Fix sketch:** `ConstructAp` on a subtree whose root is a quarantine that
would now fit should unwrap it — the same one-action `Finish` fold as point 4,
at the other end of the same problem.

### 8. `no binder named X is in scope` is a dead end — sev 3

**Trying to:** find the stdlib functions I half-remembered.
**Happened:** seven guesses, seven identical dead ends:

```
construct-var head       → no binder named `head` is in scope
construct-var last       → no binder named `last` is in scope
construct-var contains   → no binder named `contains` is in scope
construct-var lengthof   → no binder named `lengthof` is in scope
construct-var Length     → no binder named `Length` is in scope
construct-var to_str     → no binder named `to_str` is in scope
construct-var show       → no binder named `show` is in scope
```

`head_or` exists and is one edit away from `head`. `length` is one case
change from `Length`. The error knows the whole name table — 37 stdlib names
plus the document's — and offers none of it, not even "did you mean", not
even "there are 37 definitions in scope; `nothing doc` lists them". With a
standard library, guessing names *is* the interface, and the interface says
nothing.
**Fix sketch:** the error carries the three nearest names by edit distance,
and a pointer to `nothing doc`. The ranking function in `tui::complete`
already scores candidates; this is the same data through a different door.

### 9. The doc line is the last thing on the status line, so it is off-screen — sev 3 — **FIXED**

**FIXED 2026-08-29.** The doc of the highlighted candidate gets a line of
its own, under the status line, rendered only when there is one — so no
existing layout changed and nothing competes with it for width. Pinned by
`render::tests::the_status_line_marks_a_prelude_candidate_and_shows_its_doc`,
which asserts the doc survives `render_to_string(&state, 60, 14)` — a
terminal half the width of the one where it used to disappear.


**Trying to:** see what `repeat_str` does while choosing it, in a 120-column
terminal.
**Happened:** at 120 columns the status line reads

```
node · expects ? -> Cmd ? · quarantined ⦇e⦈ · does not fit yet · 1 quarantined · typing `r` · ‹std·repeat_str:Num -> Str
```

— clipped mid-type, and the doc line never appears. Widening the terminal to
240 columns shows it:

```
… · ‹std·repeat_str:Num -> Str -> Str›  std·reverse:List ? -> List ?  readline:Cmd Str · a string written out n times
```

The doc is appended *after* up to four candidates with their full types, so
at any width a person actually uses it is the first thing cut. The one new
piece of information in the whole line is the one that never survives.
**Fix sketch:** put the doc immediately after the highlighted candidate,
before the alternatives, and truncate the alternatives instead of the doc.
Better: give the doc its own line under the status line when there is one —
it is the only part that is a sentence.

### 10. `construct-lam` ignores the annotation the definition already has — sev 3

**Trying to:** write the body of `todo_kept : Str -> Bool`.
**Happened:** `construct-lam` produced `λx0:?. ⦇⦈`. The definition's
annotation says the parameter is a `Str`; the editor knows it (it correctly
computed the body's expected type as `Bool` from the very same annotation)
and puts `?` on the binder anyway. Every one of the six lambdas in this
session cost the same three extra steps: `move-parent`, `rename`, `set-ann`.
Eighteen steps of typing something already written down.
**Fix sketch:** `ConstructLam` at a hole whose expected type is `T -> U`
annotates the fresh binder `T`. It is already the analysis result; nothing
new is computed. A person who wants `?` can still `set-ann ?`.

### 11. There is no `Num -> Str`, so a program cannot print a number — sev 4

**Trying to:** print how many todos survived the filter.
**Happened:** `print : Str -> Cmd ?`, and there is no conversion from `Num`
to `Str` anywhere — not in the language, not in the 37 stdlib definitions.
The count of a list is the single most obvious thing a report prints and it
cannot be printed. I shipped `todo_bar = repeat_str (count todo_kept xs) "*"`
— a bar chart of asterisks — because a bar chart was reachable and a numeral
was not. That is a workaround presented as a design choice, and it is the
single largest thing standing between this language and a real program.
**Fix sketch:** this is not a stdlib definition — there is no way to write it
in the language. It is a primitive: one node tag, one `Op`, or one more
`Cmd`. `Num -> Str` first; `Str -> Num` (partial, so `Str -> Num * Bool` or a
default) second.

### 12. A string literal cannot contain a newline — sev 2

**Trying to:** print a two-line banner.
**Happened:** `construct-str` documents exactly two escapes, `\"` and `\\`.
There is no `\n`, and the entry slot in the TUI is a single line, so there is
no way to type a literal newline either. Multi-line output has to be
decomposed into a `List Str` and pushed through `print_all`, which is what
`todo_report` ended up being. That is not a bad shape for this program, but
it was chosen by the escape table rather than by me.
**Fix sketch:** add `\n` and `\t` to the escape set. The format already
length-prefixes strings, so nothing downstream cares.

### 13. `print` always writes a whole line, so a prompt is never on the same line as its answer — sev 2

**Trying to:** ask three questions.
**Happened:** `ask s = bind ignored <- print s in readline` — my own stdlib
definition, and the best one available, because `print` is the only output
primitive and it always terminates the line. The session looks like this:

```
todo 1:
todo 2:
todo 3:
your list:
```

Three prompts, three empty answers, nothing where the typing went. Every
interactive program in this language reads as if it is talking to itself.
**Fix sketch:** a second primitive that writes without a newline, or a flag
on `print`. This is the same size of change as point 11 and shares its
justification: the effect vocabulary is one verb short.

### 14. `join` silently swallows a trailing empty string — sev 2

**Trying to:** understand what `join` would do with a list containing a blank.
**Happened:** it is a genuine bug in the stdlib I shipped four hours earlier,
and I found it by running the product:

```
main : Str = join "|" ("a" :: "b" :: "" :: nil)
$ nothing run join_probe.n
"a|b"
```

`join` folds with `if a == "" then x else x ++ (sep ++ a)`, using the empty
string as its "nothing accumulated yet" sentinel — so an element that *is*
the empty string is indistinguishable from the end of the list. The result is
`"a|b"` where `"a|b|"` is right.
**Fix sketch:** fold over `List (Str * Bool)` or, simpler, define `join` in
terms of a right fold that carries the separator on every element but tests
the *tail* rather than the accumulator: `fold xs "" (λx. λa. if is_empty …)`
does not work either without a list-aware fold, so the honest fix is a
`join` built from `append`/`concat_all` over an explicitly interleaved list.
Either way this needs a test, and there is no stdlib test that runs a
definition — see point 22.

### 15. A local definition shadows a stdlib name in total silence — sev 3

**Trying to:** find out what happens if I name something `min`.
**Happened:** `create-definition; rename-def min; set-def-ann Num;
construct-num 0` was accepted with no comment. `construct-var min` then
resolved to my `min : Num` (id `e9e9e555…`) rather than the stdlib's
`min : Num -> Num -> Num` (id `4633052f…`). That is the right rule — a
document's own definitions must win, and `Prelude::extend` is deliberately
built that way — but nothing anywhere says it happened. Not the state, not
the completion list, not `nothing check`. Every document that reads
`min a b` now means something different from every other document, and the
only way to find out is to read the ids.
**Fix sketch:** `state` marks a definition that shadows a prelude id, and
completion shows the shadowed one greyed out beneath it. `nothing check`
prints `shadows stdlib: min` the way it already prints `stdlib definitions in
scope: 37`.

### 16. The standard library is unreadable from inside a session — sev 3

**Trying to:** see what `join` actually does, without leaving the editor.
**Happened:** `move-to-def join` → `the action does not apply at this
cursor`. Stdlib definitions are in the typing context, in the name table, in
completion, and reachable by `construct-var` — but not by the cursor. There
is no way to read the body of a function you are calling. The answer is to
quit and run `nothing doc`, which is the right answer for a reference and the
wrong one for "what does the third argument of `fold` get here".
**Fix sketch:** `move-to-def` on a prelude id opens it **read-only** — the
cursor can walk it, every editing action refuses. The `Prelude` already holds
the `Def`s; the zipper does not know they are off-limits, which is the work.

### 17. `move-to-def NAME` that fails does not say what it could not find — sev 2

**Trying to:** the above.
**Happened:** `the action does not apply at this cursor`. The name I passed
does not appear. Whether the definition does not exist, or exists and is in
the prelude, or exists and something else is wrong, is not distinguished. The
same generic sentence covers every step in the grammar, so it says nothing
about any of them.
**Fix sketch:** `move-to-def` resolves the name itself and reports
`no definition named 'join' in this document (it is in the standard library)`
— which is both the diagnosis and the answer.

### 18. `set-doc` with an empty argument erases silently, and nothing on screen changes — sev 2

**Trying to:** see what `set-doc` with no text does.
**Happened:** it cleared `main`'s doc line and reported `ok`. That is
documented behaviour — "(empty clears)" — but a doc line is invisible in
every projection, so the only feedback that a sentence was deleted is that a
field in a 17 KB JSON reply went from a string to `null`. I lost the line and
did not notice until I looked.
**Fix sketch:** a doc line belongs on screen. The definition list pane shows
one per row, or the status line shows the current definition's. Deleting
something no view displays is a data-loss shape, however well documented.

### 19. `nothing edit` saves on quit, unconditionally, silently — sev 3

**Trying to:** look at `todo.n` in the TUI without changing it.
**Happened:** I typed two characters to see the completion list, pressed
`C-z` twice to undo them, and pressed `C-q`. The process exited with no
output and rewrote the file — 7.6 KB to 11 KB, because the navigation I did
while looking is in the action log now. There is no "saved to todo.n" line,
no prompt, and no way to quit without writing. `FRICTION.md` point 2 was
"nothing is ever saved"; the fix went to the other extreme, and the failure
mode is now "reading a file changes it".
**Fix sketch:** print `saved todo.n (N actions)` on the way out — the same
one line `merge -o` already prints. And bind a discard-and-quit, or prompt
when the log grew.

### 20. Definitions land after the cursor and can never be moved — sev 2

**Trying to:** keep the file in the order I think about it.
**Happened:** `create-definition` adds after the current definition, which is
documented and sensible. But I created `todo_status` while standing in
`todo_bar`, and the file order became `main, todo_kept, todo_bullet,
todo_bar, todo_status, todo_report` — the helper I wrote last sitting in the
middle. There is no move-definition action anywhere: not in the protocol, not
in the keyscript, not in the TUI. The merge engine has a whole concept of a
definition *move* and there is no way to perform one.
**Fix sketch:** `move-def-up` / `move-def-down`, two actions, both total.
`Doc` is an ordered `Vec`; the merge engine already models the result.

### 21. The definition pane truncates names and never shows a doc — sev 2

**Trying to:** tell `todo_bullet` from `todo_bar` in the sidebar.
**Happened:** at 120 columns the pane is 20 wide and every row is elided:
`todo_bullet : S…`, `todo_bar : List…`, `todo_status : L…`. The types are cut
at the point where they start being informative, and the doc line — one short
sentence per definition, which is exactly what a sidebar wants — is not shown
at all. `nothing doc` renders a beautiful table of exactly this information
into a file I have to leave the editor to read.
**Fix sketch:** the pane shows `name` and, on a second dim line, the doc
line; the type moves to the status line where there is room. That is the
information a person picks a definition by.

### 22. Nothing runs a stdlib definition, so point 14 shipped — sev 3

**Trying to:** explain to myself how `join` shipped with a bug.
**Happened:** the stdlib crate has nine tests. They assert it decodes, that
every definition is named, annotated and documented, that it has no holes,
that the action log replays to the committed document, and that the bytes
round-trip. Not one of them **runs** a definition — the crate cannot depend
on `nothing-eval` without inverting the dependency graph. `cli/tests` does
run four of them (`min`, `max`, `sum`, `print_labelled`) end to end through
`nothing run`, which is four out of thirty-seven and does not include `join`.
So `join` is well-typed, hole-free, documented, byte-stable, provably built
by the product — and wrong. Every property anything checked was true of the
bug.
**Fix sketch:** finish the `cli/tests` table that already exists: a
`(expression, expected value)` case for every one of the thirty-seven, not
four. It is a day's work and would have caught this in a minute — and the
cases that matter most are the edge ones the four happy-path calls skip: an
empty list, a list of one, and the empty string as an element.

---

## Summary

22 friction points; the spec asks for fifteen.

**Status, 2026-08-29:** five fixed — **1**, **2**, **3**, **5** and **9** —
plus **6**, which shares point 5's fix and is not claimed as one of the five.
Sixteen open, of which **4** (list building), **7** (curried calls) and **11**
(no `Num -> Str`) are the ones the next phase should take first, for the
reasons given in the note at the top of the list.

By severity: **5** — 1 (an unapplied action reports success); **4** — 2
(`nothing doc` prints ids), 3 (the stdlib in every reply), 4 (list building
quarantines every element), 5 (no jump-to-hole), 11 (no `Num -> Str`); **3**
— 6, 7, 8, 9, 10, 15, 16, 19, 22; **2** — 12, 13, 14, 17, 18, 20, 21.

Three observations that are not individually friction points but shape the
list:

- **The prelude design held.** Not once did a stdlib reference behave
  differently from a local one: `construct-var` resolved them the same way,
  the typing context contained them at every hole, saving wrote none of them
  into the file (`todo.n` is 13 KB against the stdlib's 54 KB), and
  `nothing run` found them without a single import. Every complaint above is
  about *feedback* — what the protocol reports, what the status line shows,
  what an error says — and not one is about the ambient-scope decision
  itself.
- **The `?` mush arrived on schedule, and it is worse than "irritating".**
  Seventeen of the thirty-seven stdlib signatures contain a `?`; fifteen of
  those are genuinely generic. The design commitments named "ten functions of
  `?` mush" as the revisit trigger and it has been passed by half again. The
  concrete cost is not aesthetic: `map not (todo_report nil)` — mapping
  `Bool -> Bool` over a `List Str` — was accepted with **zero quarantines**,
  because `map : (? -> ?) -> List ? -> List ?` cannot relate its two type
  arguments. The library's most-used function does not typecheck its own
  argument. This is written up as evidence, not a redesign, in `DECISIONS.md`
  under 2026-08-29.
- **The two worst problems are the two the product cannot express.** A
  program cannot print a number and cannot write a prompt without a newline.
  Both are one primitive each, both are outside the stdlib by construction,
  and until they land, "build something real" means "build something real
  that never shows you a number".
