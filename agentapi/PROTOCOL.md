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
| `undo` | — | truncate one entry and replay |
| `redo` | — | re-apply the entry undo removed |
| `reset` | — | back to the empty program with an empty log |
| `save` | `path` | write a `store` document (binary, `NTHG`) |
| `load` | `path` | read a `store` document and adopt it |
| `log` | — | the action log up to the undo cursor |
| `provenance` | — | per-node author and time, from the replayed log |
| `annotate` | `agents` (array of author ids), `style` | an author-annotated render |
| `help` | — | the protocol version, the method list, the step grammar |
| `quit` | — | answer, then exit |

### Naming an action

Two spellings are accepted and mean exactly the same thing.

**Textual**, reusing the REPL step grammar from `action/src/script.rs` verbatim —
this is the same text that `bench/fixtures/*.actions` is written in:

```json
{"method": "apply", "params": {"step": "construct-num 42"}}
{"method": "apply", "params": {"step": "construct-var xs"}}
{"method": "apply", "params": {"step": "set-ann Num -> Bool"}}
```

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
  "render": "λn:Num. if n == 0 then 1 else n * ⦇⦈",
  "render_with_cursor": "λn:Num. if n == 0 then 1 else n * »⦇⦈«",
  "cursor_path": [0, 2, 1],
  "focus_kind": "EmptyHole",
  "expected_ty": {"ty": "Num"},
  "expected_ty_text": "Num",
  "well_typed": true,
  "empty_holes": 1,
  "non_empty_holes": 0,
  "complete": false,
  "log_len": 16,
  "can_undo": true,
  "can_redo": false,
  "author": 1,
  "exp": {"exp": "Lam", "id": "…", "name": "n", "ann": {…}, "body": {…}},
  "names": [{"id": "…", "name": "n"}]
}
```

`cursor_path` is the child index of each frame from the root down, so it is the
same coordinate system `merge`'s `Path` uses. `complete` is true when the program
contains neither kind of hole. `well_typed` is reported rather than assumed; it
has never been observed false, because the calculus does not permit it.

---

## The hole-context query

`{"method": "hole_context"}` answers with everything a model needs to choose its
next edit, and nothing it would have to guess:

```json
{
  "cursor_path": [0, 1],
  "focus_kind": "EmptyHole",
  "focus_render": "⦇⦈",
  "at_empty_hole": true,
  "expected_ty": {"ty": "Num"},
  "expected_ty_text": "Num",
  "bindings": [
    {"id": "…", "name": "n", "ty": {"ty":"Num"}, "ty_text": "Num",
     "consistent_with_expected": true, "shadowed": false}
  ],
  "constructions": [
    {"step": "construct-num 0", "template": "construct-num <integer>",
     "action": {"action":"ConstructNum","value":0},
     "produces": "λn:Num. n * »0«", "cursor_after": [0, 1]}
  ],
  "movements": ["move-parent", "move-prev-sibling"],
  "other_actions": ["delete"]
}
```

**Bindings** are the binders in scope at the cursor, outermost first, each with
its type from the typing context, its *display name* from the name table, and
whether that type is consistent with the expected type. `shadowed` marks a
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

Movements and the remaining actions (`delete`, `finish`, `set-ann`, `rename`)
are listed separately because they are not constructions: they do not fill the
hole, so the well-typedness question above does not apply to them. Only the ones
that actually apply at this cursor are listed.

---

## Provenance

Every log entry carries an author and a timestamp (Phase 2). The protocol turns
the log into a **per-node** projection.

`{"method": "provenance"}` answers with one record per node of the current
program:

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

`{"method": "annotate", "params": {"agents": [2], "style": "brackets"}}` renders
the program with the spans written by those authors visually distinguished. A
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
