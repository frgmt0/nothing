# The agent edit protocol

`nothing-agentapi` exposes the Phase 2 action calculus as a serialisable protocol
so that a process outside the editor — a script, a test, or a language model —
can drive the editor without ever seeing or producing program text.

The commitment being honoured here is the one at the top of `spec.md`: *no
stringly-typed intermediate*. The protocol carries **actions**, not source. A
client never assembles a program and asks the editor to read it back; it names
edits, and the editor answers with the new program and the new cursor. Every
action either applies, leaving a well-typed program, or is refused. There is no
third outcome, and there is no parse step anywhere on this path.

Run it with:

```
cargo run -p nothing-agentapi --bin protocol
```

It reads **one JSON object per line** on stdin and writes **one JSON object per
line** on stdout. Nothing else appears on stdout. Blank lines and lines starting
with `#` are ignored and produce no response. `--author N` sets the default
author id recorded in the action log (default `1`).

---

## Protocol version 1

This document describes **protocol version 1**, and version 1 is frozen. A
client asks which version it is talking to with the cheapest request in the
protocol:

```json
{"method": "version"}
```

answers

```json
{"id":null,"ok":true,"applied":false,"protocol_version":"1","protocol_major":1,"protocol_minor":0,"implementation_version":"0.1.0","state":{…}}
```

- `protocol_version` is the **major** version as a string, and nothing else. A
  client may compare it to `"1"` and be right for the whole life of v1.
- `protocol_major` and `protocol_minor` are the same number split into integers,
  so a client can require a minimum minor version without parsing a string.
  Every additive change bumps `protocol_minor`; the string does not move.
- `implementation_version` is the crate version of the process answering — the
  build, not the contract. It moves on every release and means nothing to
  compatibility.

`help` reports the same `protocol_version` alongside the method list and the
whole step grammar; `version` exists because a handshake should not have to
carry a multi-kilobyte grammar to learn one number.

### What version 1 guarantees

For every method in the table below, and for every error response:

- every field named in this document is **present** on every response that
  document says carries it;
- every such field keeps the **JSON type** it has here — an integer stays an
  integer, an array stays an array, an object stays an object;
- a field documented as nullable may be `null` or its stated type, and nothing
  else;
- `id`, `ok`, `applied` and `state` are on **every** response, without
  exception, including `version` and every error.

### Additive versus breaking

**Additive, allowed inside v1** (bumps `protocol_minor`):

- a new field on a response object;
- a new method, added to `METHODS` and to the table below;
- a new optional `params` key whose absence keeps the old behaviour;
- a new member of an open-ended string set — a new `focus_kind`, a new
  `action`, a new step spelling.

**Breaking, forbidden inside v1** (would require a v2):

- removing or renaming any field;
- changing the JSON type at any documented path;
- making a nullable field non-nullable in a way that removes it, or a
  non-nullable field null;
- removing a method, or changing what an existing `params` key means;
- moving a field to a different nesting level.

### The compatibility test enforces this

`agentapi/tests/protocol_v1_compat.rs` drives the real `protocol::handle` with a
real `AgentSession` — no mock — over one case for every method in the table
below and five more for the error shapes, and compares each response against a
golden fixture in `agentapi/fixtures/protocol/v1/`, one file per case.

The fixtures pin the **shape**, not the values — ids, timestamps and hole ids are
freshly generated on every run, so pinning values would pin noise. Each fixture
records the `setup` requests, the pinned `request`, and a sorted
`response_shape`: one `path: type` line for every path in the response, where
an object field extends the path with `.name`, and every element of an array
extends it with `[]` and has its types **unioned** across the elements, so array
length can never make the shape wobble. A path whose only observed value was
`null` is pinned as `null`, which matches any later type — that is how a
documented nullable field stays pinnable.

The comparison is deliberately asymmetric. Every pinned path must still be
present with a compatible type: a path that disappeared, or whose type changed,
fails the test and names itself in the failure. A path present in the response
but absent from the fixture is an **addition**, so it is printed to stdout as an
allowed additive change and does not fail; run the test with `-- --nocapture` to
read that list. Fixtures cannot rot behind that leniency, though: a second test
asserts that every method in `protocol::METHODS` has a fixture, so adding a
method without pinning it fails, and a third asserts the fixture directory holds
exactly the pinned cases, so a stale file fails too.

Regenerate the fixtures — after a deliberate additive change, or when adding a
method — by running the test with `NOTHING_UPDATE_FIXTURES=1`:

```sh
NOTHING_UPDATE_FIXTURES=1 cargo test -p nothing-agentapi --test protocol_v1_compat
```

Review the resulting diff. Lines that only appear are additive. A line that
disappears, or whose type changed, is a break in v1 and the code is wrong, not
the fixture. Fixture requests may use the placeholder `$TMPDIR/` at the front of
a path string; the test substitutes a scratch directory it creates and removes,
so `save` and `load` fixtures are the same on every machine.

---

## Requests

```json
{"id": 7, "method": "apply", "params": {"step": "construct-binop mul"}}
```

`id` is optional and is echoed back verbatim (any JSON value). `params` is
optional. The methods are:

| method | params | what it does |
|---|---|---|
| `state` | — | re-report the current program and cursor |
| `apply` | `step` or `action`, optional `author` | apply one action |
| `script` | `steps` (array), optional `author` | apply a sequence, stopping at the first step that does not apply |
| `hole_context` | — | the hole-context query (below) |
| `stdlib` | — | the whole prelude catalogue, once (`stdlib`) |
| `move_to_hole` | optional `forward` (default `true`), optional `author` | walk the cursor to the next unfinished position, as ordinary logged moves (`actions`) |
| `undo` | — | truncate one entry and replay |
| `redo` | — | re-apply the entry undo removed |
| `reset` | — | back to the empty program with an empty log |
| `save` | `path` | write a `store` document (binary, `NTHG`) |
| `load` | `path` | read a `store` document and adopt it |
| `log` | — | the action log up to the undo cursor |
| `provenance` | — | per-node author and time, from the replayed log |
| `annotate` | `agents` (array of author ids), `style` | an author-annotated render (`annotated`, one definition; `annotated_document`, all of them) |
| `help` | — | the protocol version, the method list, the step grammar |
| `version` | — | the protocol version alone (`protocol_version`, `protocol_major`, `protocol_minor`, `implementation_version`) |
| `quit` | — | answer, then exit |

### Naming an action

Two spellings are accepted and mean exactly the same thing.

**Textual**, reusing the REPL step grammar from `action/src/script.rs` verbatim —
this is the same text that `bench/fixtures/*.actions` is written in:

```json
{"method": "apply", "params": {"step": "construct-num 42"}}
{"method": "apply", "params": {"step": "construct-var xs"}}
{"method": "apply", "params": {"step": "set-ann Num -> Bool"}}
{"method": "apply", "params": {"step": "create-definition"}}
{"method": "apply", "params": {"step": "rename-def helper"}}
{"method": "apply", "params": {"step": "set-def-ann Num -> Num"}}
{"method": "apply", "params": {"step": "move-to-def helper"}}
```

The definition steps are `create-definition`, `delete-definition`,
`rename-def NAME`, `set-def-ann TY`, `move-next-def`, `move-prev-def` and
`move-to-def NAME`. `construct-var NAME` resolves against the definitions of
the document as well as the binders in scope, which is how one definition
calls another and how a definition calls itself.

**Structured**, for clients that would rather not build strings, and for the one
case the textual grammar cannot express — referring to a binder that is shadowed
by a nearer binder with the same display name:

```json
{"method": "apply", "params": {"action": {"action": "ConstructNum", "value": 42}}}
{"method": "apply", "params": {"action": {"action": "ConstructVar", "id": "…-uuid-…"}}}
{"method": "apply", "params": {"action": {"action": "SetAnn", "ty": "Num -> Bool"}}}
```

`SetAnn` takes either the tagged form `{"ty":"Arrow","from":…,"to":…}` or the
string spelling the REPL uses. `construct-var NAME` resolves the display name
against the binders in scope at the cursor, innermost first, exactly as the REPL
does; the structured form names the `Id` and cannot be ambiguous.

## Responses

Every response — success, refusal, or error — carries `applied` and the
re-rendered program with the cursor in it. That is the invariant a client can
rely on: one line in, one line out, and the line out always says what the
program looks like now.

```json
{"id":7,"ok":true,"applied":true,"action":{…},"state":{…}}
```

- `ok` — the request was understood. A malformed request, an unparseable step,
  an unknown method or a failed file operation gives `ok:false` and an `error`.
- `applied` — the program changed. An action that parses but does not apply at
  this cursor gives `ok:true, applied:false` and an `error` explaining that; it
  is not an error in the request, it is the calculus declining. Nothing is
  written to the action log.
- `state` — always present, on every response:

```json
{
  "render": "λn:Num. if n == 0 then 1 else n * main (n - 1)",
  "render_document": "main : Num -> Num = λn:Num. if n == 0 then 1 else n * main (n - 1)",
  "definitions": [
    {"id": "…", "name": "main", "ann": {…}, "ann_text": "Num -> Num",
     "doc": null, "current": true}
  ],
  "definition": "…",
  "definition_name": "main",
  "definition_doc": null,
  "definition_index": 0,
  "definition_count": 1,
  "definition_ann": {"ty": "Arrow", …},
  "render_with_cursor": "λn:Num. if n == 0 then 1 else n * main (n - »1«)",
  "cursor_path": [0, 2, 1],
  "focus_kind": "EmptyHole",
  "expected_ty": {"ty": "Num"},
  "expected_ty_text": "Num",
  "well_typed": true,
  "empty_holes": 1,
  "non_empty_holes": 0,
  "holes": [[0, 2, 1]],
  "document_empty_holes": 1,
  "document_non_empty_holes": 0,
  "complete": false,
  "log_len": 16,
  "can_undo": true,
  "can_redo": false,
  "author": 1,
  "exp": {"exp": "Lam", "id": "…", "name": "n", "ann": {…}, "body": {…}},
  "names": [{"id": "…", "name": "n"}],
  "docs": [{"id": "…", "doc": "two of them"}],
  "stdlib_count": 12
}
```

`cursor_path` is the child index of each frame from the root down, so it is the
same coordinate system `merge`'s `Path` uses. `complete` is true when the program
contains neither kind of hole. `well_typed` is reported rather than assumed; it
has never been observed false, because the calculus does not permit it.

`holes` lists the cursor path of every unfinished position — every empty and
non-empty hole — in the definition the cursor is in, in the order the cursor
would walk them, which is the order `move_to_hole` uses. `empty_holes` and
`non_empty_holes` count that same definition; `document_empty_holes` and
`document_non_empty_holes` count every definition in the document, so a client
can tell "this definition is finished" from "the program is finished".

`definition_doc` and each entry's `doc` are the documentation line for that
definition, or `null` when it has none. `names` and `docs` are the vocabulary
needed to read this document: the document's own names, plus the name and doc
line of every prelude definition the document actually references, and no
others — an untouched document borrows nothing. `stdlib_count` is how many
definitions the prelude holds in total; the catalogue itself comes from the
`stdlib` method, which is answered once rather than repeated on every response.

Since the document era (Phase B1) a program is an ordered list of named
top-level definitions. `render`, `render_with_cursor`, `exp` and `cursor_path`
all describe **the definition the cursor is in**; `render_document` and
`definitions` describe the whole document. `well_typed` is the *document's*
well-typedness, which is the only meaningful one — a body that calls another
definition by id is not well typed in isolation. `definition` is the id of the
definition the cursor is in, `definition_index` its position in
`definitions`, and `definition_ann` its type annotation. There is always at
least one definition; a document loaded from a version 1 file has exactly one,
named `main`.

---

## The hole-context query

`{"method": "hole_context"}` answers with everything a model needs to choose its
next edit, and nothing it would have to guess:

```json
{
  "definition": "…",
  "definition_name": "main",
  "definition_ann": {"ty": "Num"},
  "definition_ann_text": "Num",
  "cursor_path": [0, 1],
  "focus_kind": "EmptyHole",
  "focus_render": "⦇⦈",
  "at_empty_hole": true,
  "expected_ty": {"ty": "Num"},
  "expected_ty_text": "Num",
  "bindings": [
    {"id": "…", "name": "n", "ty": {"ty":"Num"}, "ty_text": "Num",
     "consistent_with_expected": true, "shadowed": false,
     "definition": false, "stdlib": false, "doc": null}
  ],
  "constructions": [
    {"step": "construct-num 0", "template": "construct-num <integer>",
     "action": {"action":"ConstructNum","value":0},
     "produces": "λn:Num. n * »0«", "cursor_after": [0, 1]}
  ],
  "movements": ["move-parent", "move-prev-sibling", "move-next-def"],
  "other_actions": ["delete", "create-definition", "set-def-ann Num", "rename-def <name>"]
}
```

`step` and `template` on a construction, and `doc` on a binding, are `null` when
they do not apply; the keys are always there.

**Bindings** are the definitions of the document and of the prelude, followed by
the binders in scope at the cursor, outermost first, each with its type from the
typing context, its *display name* from the name table, its documentation line
if it has one, and whether that type is consistent with the expected type. A
binding with `"definition": true` is a top-level definition rather than a path
binder — including the definition the cursor is in, which is how a recursive
call is written — and `"stdlib": true` marks the ones that come from the prelude
rather than from this document. `shadowed` marks a
binding whose display name is taken by a nearer binding — it is still reachable
through the structured `ConstructVar` form, and its `step` is `null`, because
the textual `construct-var NAME` would resolve to the other one.

**Constructions** are the constructions that are well typed here, in the precise
sense the spec asks for: *applying one of them does not produce a non-empty
hole*. This is not approximated from the expected type. Each candidate is
actually applied to a copy of the current state, and it is offered only if it
applies and the number of non-empty holes in the program did not go up. The
calculus is its own oracle; the query cannot drift away from it.

`template` is present for the two constructions that carry a payload the client
should vary — `construct-num <integer>` and `construct-bool <true|false>`. The
concrete `step` is a real, applicable action (`construct-num 0`); the template
says which part is free. Substituting a different literal cannot change the
outcome, because every integer literal synthesises `Num` and every boolean
literal synthesises `Bool`.

Consequences worth stating, all of them covered by tests in
`agentapi/src/holectx.rs`:

- at a hole expecting `Num`, no boolean and no `Bool`-typed variable is offered;
- at a hole expecting `Bool`, no number is offered;
- the offered set at an empty hole is never empty (`delete`, `construct-num`,
  `construct-bool` or a variable always survives for *some* expected type, and a
  hole expecting `?` admits everything);
- a property test over random well-typed programs and random cursors applies
  every offered construction and asserts no non-empty hole appeared.

Movements and the remaining actions (`delete`, `finish`, `set-ann`, `rename`,
`create-definition`, `delete-definition`, `set-def-ann`, `rename-def`)
are listed separately because they are not constructions: they do not fill the
hole, so the well-typedness question above does not apply to them. Only the ones
that actually apply at this cursor are listed.

---

## The standard library, and walking to the next hole

`{"method": "stdlib"}` answers with the whole prelude catalogue in one reply,
under `stdlib`:

```json
[{"id": "…", "name": "map", "ann_text": "(Num -> Num) -> [Num] -> [Num]",
  "doc": "apply a function to every element"}]
```

`doc` is `null` for a definition with no documentation line. The catalogue rides
on the reply that asked for it and never on `state`, because it does not change:
a client fetches it once per session. `state.stdlib_count` says how many entries
it has, so a client can tell whether it has them all.

`{"method": "move_to_hole"}` walks the cursor to the next unfinished position —
empty hole or quarantine — in the definition the cursor is in, wrapping round to
the first when there is none after the cursor. `{"params": {"forward": false}}`
walks backwards. It is not a jump: the reply's `actions` is the list of ordinary
movement actions it applied, every one of them written to the action log, so
undo walks back through them exactly as it would through hand-made moves.

```json
{"id":null,"ok":true,"applied":true,"actions":[{"action":"MoveChild","n":2},{"action":"MoveChild","n":1}],"state":{…}}
```

A definition with nothing unfinished left in it answers `ok:false` with an
`error` rather than moving.

---

## Provenance

Every log entry carries an author and a timestamp (Phase 2). The protocol turns
the log into a **per-node** projection.

`{"method": "provenance"}` answers with one record per node of the definition
the cursor is in:

```json
[{"path": [0, 1], "author": 2, "timestamp": 1756392012345, "entry": 11}]
```

`author` is `null` for nodes that were already there before the log started —
a program that was loaded rather than built.

The rule, applied after each replayed entry, walking every path of the new
program:

1. If the old program had a node at the same path with the same *shallow shape*
   — same constructor and same payload (the number, the boolean, the operator,
   the projection side, the binder id, the annotation, the hole id) — the node
   keeps the provenance it already had. Its children may have changed; the node
   itself did not.
2. Otherwise, if some node that was displaced by this edit has an identical
   *full structure* including binder and hole ids, the provenance moves with it.
   Matching is one-to-one and consuming: each displaced node is claimed at most
   once. This is what makes wrapping honest — after `construct-binop add` turns
   a human's `1` into `1 + ⦇⦈`, the `1` is still the human's, the `+` and the new
   hole belong to whoever wrapped it.
3. Otherwise this entry created the node, and it takes this entry's author and
   timestamp.

`Rename` creates no node, so it changes no node's provenance; it is recorded
against the binder's `Id` instead.

Provenance is kept **per definition**: the rule above is applied to every
definition's body after every entry, so an edit inside one definition cannot
reattribute another, and a definition created by an author is that author's
from its first empty hole onwards. Deleting a definition rewrites references to
it into empty holes, and those holes are attributed to whoever deleted it,
which is correct — they were not there before.

`{"method": "annotate", "params": {"agents": [2], "style": "brackets"}}` renders
the program with the spans written by those authors visually distinguished. The
reply carries `annotated` (the definition the cursor is in) and
`annotated_document` (every definition, one per line, headed by its name and
type). A
marker opens wherever the class changes, so the spans are maximal and a
human-authored node sitting inside a model-authored one is marked back out:

```
λn:Num. if n < 0 then ⟦0 - n⟧ else n
⟦1⟧ + 2                       model wrote the 1, a human wrote the rest
⟦⟨1⟩ + 2⟧                     a human wrote the 1, the model wrapped it
```

`style` is `brackets` (default: `⟦…⟧` model, `⟨…⟩` human), `ansi` (magenta and
cyan), or `plain`. `plain` reproduces `core::render::render` character for
character, which is the test that the annotator has not changed the projection.

The diff-side counterpart lives in `merge::provenance`: it attributes each typed
`Operation` in a structural diff to the log entry that produced it, and filters
the diff to only-human or only-agent operations. See `merge/src/provenance.rs`.

---

## Driving the editor with a model

```
cargo run -p nothing-agentapi --bin drive
```

`drive` starts the `protocol` binary as a real subprocess and talks JSON to it.
Each turn it asks for `state` and `hole_context`, renders them into a prompt
alongside the goal and the last few actions, shells out to the `claude` CLI in
headless mode for exactly one action, and applies whatever comes back — applied
or refused, both are recorded. It stops when the render matches the target or a
step cap is hit. The actions come from the model's replies; nothing about the
answer is in the harness.

Flags: `--goal`, `--target`, `--setup` (a `;`-separated action script applied
first as author 1, so the model's edits land on a human-authored base),
`--max-steps`, `--transcript`, `--editor`. `NOTHING_CLAUDE_BIN` and
`NOTHING_CLAUDE_MODEL` override the CLI path and the model id.

Transcripts, including the full prompt sent at every step, are under
`bench/agent-transcripts/`. The measured comparison against a text baseline is
in `bench/AGENT.md`.

---

## The measurement instrument

`agentapi/src/measure/text_parse.rs` is a parser for the *rendered* syntax. It
exists solely so the text-baseline arm of `bench/AGENT.md` can be scored: to ask
whether a program a model wrote as text parses at all, and whether it then
typechecks. It is used only by the `agentbench` binary and by its own tests.

It is not part of any editing path. No action, no protocol method and no
projection ever reads program text. Deleting this file would not change the
behaviour of the editor by one bit; it would only make the baseline unmeasurable.
That distinction is the whole reason the no-text-intermediate commitment survives
having a parser in the repository at all.
