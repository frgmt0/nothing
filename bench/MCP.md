# `nothing mcp` — the editor as an MCP server

`nothing mcp` puts the whole structural editor behind the Model Context Protocol, so any
MCP-speaking agent host — Claude Code, or anything else that speaks the same wire format —
can build, check, run and save a `nothing` program without ever handling program text.

This is the same commitment `agentapi/PROTOCOL.md` makes, wearing a different hat. The MCP
layer is a translation of tool calls into agent-protocol requests: every tool goes through
`nothing_agentapi::protocol::handle`, and the editor semantics live in exactly one place.
There is no parser on this path either.

```
nothing mcp [--author N] [--no-stdlib]
```

* `--author N` — the author id recorded in the action log for every applied action (default `1`).
  Give the model a different id from the human and `get_projection` with
  `"projection": "annotated"` will show you who wrote what.
* `--no-stdlib` — start with an empty prelude, the way the standard library itself was built.

## Transport

Newline-delimited JSON-RPC 2.0 over stdio: one message per line on stdin, one message per
line on stdout. **Nothing but protocol messages is ever written to stdout.** Diagnostics go
to stderr. A program run through the `run` tool has its `print` output captured into the tool
result rather than written to the process's stdout, which is what keeps the stream intact —
there is an integration test that asserts every line the server emits parses as a JSON-RPC
message.

A clean end of input exits 0. A malformed line produces a parse-error response and the server
keeps running.

## The handshake

Protocol versions supported, newest first: **`2025-06-18`**, `2025-03-26`, `2024-11-05`.
Negotiation is honest — a version the server supports is echoed back, anything else gets the
newest supported version, and a client that names no version gets `2025-06-18`.

`notifications/initialized` is a notification, and gets no reply. Any message without an `id`
is a notification and is never answered. A message with an `id` but a `result` or `error`
instead of a `method` is a response to a request the server never sent, and is ignored.

Requests other than `initialize` and `ping` before the handshake are refused with `-32600`.

| method | answer |
| --- | --- |
| `initialize` | `protocolVersion`, `capabilities.tools.listChanged: false`, `serverInfo`, `instructions` |
| `notifications/initialized` | nothing at all |
| `ping` | `{}` |
| `tools/list` | `{"tools": [...]}`; a `cursor` param is accepted and ignored, and there is no `nextCursor` — one page |
| `tools/call` | `{"content": [{"type":"text","text":"…"}], "isError": …, "structuredContent": {…}}` |
| anything else | JSON-RPC error `-32601` |

Error codes: `-32700` parse error, `-32600` invalid request, `-32601` method not found,
`-32602` invalid params, `-32603` internal error. Every response carries `"jsonrpc": "2.0"`
and echoes the request `id` verbatim.

**A tool that fails is not a JSON-RPC error.** An action the calculus refuses, an unknown tool
name, a file that will not open — all of these come back as a normal result with
`"isError": true` and the reason in the text content, so the model reads the refusal and tries
something else. JSON-RPC errors are reserved for protocol-level failures.

## The tools

Fifteen tools. Every one returns human-readable text a model can act on directly, and most
also return a compact `structuredContent` object carrying the same facts in machine-readable
form. The state digest inside `structuredContent` has `render_document`, `render`,
`render_with_cursor`, `definition_name`, `definition_index`, `definition_count`,
`cursor_path`, `focus_kind`, `expected_ty_text`, `well_typed`, `empty_holes`,
`non_empty_holes`, `document_empty_holes`, `document_non_empty_holes`, `complete`, `can_undo`,
`can_redo` and `log_len`.

| tool | arguments | what comes back |
| --- | --- | --- |
| `get_state` | — | the rendered document, the cursor's definition and expected type, well-typedness, hole counts, and every definition with its annotation |
| `get_projection` | `projection` (`document` \| `definition` \| `cursor` \| `annotated`), `agents` | the requested rendering as text, plus `rendered` in the structured result |
| `hole_context` | — | the expected type at the cursor, every binding in scope with its type and whether it fits, and the constructions that are well typed here — each checked by actually applying it |
| `apply_action` | `step`, `action`, `author` | whether the one action applied, and the program as it now stands |
| `apply_actions` | `steps` (required), `author` | per-step applied/refused, stopping at the first refusal, and the program as it now stands |
| `save_document` | `path` (required) | the byte count written, and the state |
| `load_document` | `path` (required) | confirmation that the document was adopted, and the state |
| `typecheck` | — | well-typedness, completeness, and per-definition empty and non-empty hole counts |
| `run` | `fuel`, `stdin_lines` | whether `main` was performed or evaluated, what it printed, the value or the holes it is blocked on, and an exit status |
| `stdlib` | `filter` | the standard-library catalogue: name, type, doc line |
| `action_grammar` | — | every action name `apply_action` and `apply_actions` accept |
| `undo` | — | the program one action earlier |
| `redo` | — | the action `undo` removed, re-applied |
| `reset` | — | a single empty definition and an empty log; the stdlib stays in scope |
| `move_to_hole` | `forward` | the cursor moved to the next (or previous) hole in this definition |

`run` has no interactive stdin — stdin is the JSON-RPC channel. `readline` reads from the
`stdin_lines` argument and returns nothing once those run out.

### The real `tools/list` reply

Produced by the run described under *A worked example* below, re-indented for reading. Nothing
else about it is edited.

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "tools": [
      {
        "name": "get_state",
        "description": "Report the whole editor state: the rendered document, the definition and cursor position you are editing at, every definition with its type annotation, whether the document is well typed, and how many empty and non-empty holes are left. Start here, and call it again whenever you have lost track of where the cursor is. There is no parser and no source text: you edit by naming actions, and every action either applies — leaving a program that is still well typed — or is refused, changing nothing. Call `hole_context` before choosing an action; it lists exactly the constructions that are well typed at the cursor.",
        "inputSchema": {
          "type": "object",
          "properties": {},
          "required": []
        }
      },
      {
        "name": "get_projection",
        "description": "Render the program for reading. `document` (the default) prints every definition, one per line, with its name and type. `definition` prints only the definition the cursor is in. `cursor` prints that definition with the focus marked »like this«. `annotated` prints the document with the spans written by the given author ids bracketed, which is how you see what you wrote versus what a human wrote.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "projection": {
              "type": "string",
              "description": "which rendering to return; defaults to `document`",
              "enum": [
                "document",
                "definition",
                "cursor",
                "annotated"
              ]
            },
            "agents": {
              "type": "array",
              "description": "for the `annotated` projection: the author ids to mark as agent-written",
              "items": {
                "type": "integer",
                "description": "an author id"
              }
            }
          },
          "required": []
        }
      },
      {
        "name": "hole_context",
        "description": "The single most useful query: what is expected at the cursor, and what can be written there. Returns the expected type, every binding in scope with its type and whether that type fits, and the constructions that are well typed here — each one checked by actually applying it, so an offered construction is guaranteed to apply and to leave no non-empty hole. Also lists the movements and the other actions that apply here. There is no parser and no source text: you edit by naming actions, and every action either applies — leaving a program that is still well typed — or is refused, changing nothing. Call `hole_context` before choosing an action; it lists exactly the constructions that are well typed at the cursor.",
        "inputSchema": {
          "type": "object",
          "properties": {},
          "required": []
        }
      },
      {
        "name": "apply_action",
        "description": "Apply exactly one action at the cursor. Name it textually with `step` — `construct-lam`, `construct-num 42`, `construct-var xs`, `set-ann Num -> Bool`, `move-child 0`, `rename-def helper` — or structurally with `action`, which is the only way to name a binder shadowed by a nearer one of the same display name. Call `action_grammar` for every spelling. The action either applies, and the reply shows the new program, or it is refused and nothing changes. There is no parser and no source text: you edit by naming actions, and every action either applies — leaving a program that is still well typed — or is refused, changing nothing. Call `hole_context` before choosing an action; it lists exactly the constructions that are well typed at the cursor.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "step": {
              "type": "string",
              "description": "the action as a step string, for example `construct-binop mul`"
            },
            "action": {
              "type": "object",
              "description": "the action in structured form, for example {\"action\":\"ConstructVar\",\"id\":\"…uuid…\"}"
            },
            "author": {
              "type": "integer",
              "description": "attribute this action to this author id instead of the session default"
            }
          },
          "required": []
        }
      },
      {
        "name": "apply_actions",
        "description": "Apply a sequence of actions in order, stopping at the first one that does not apply. This is the fast way to build a definition: send the whole script and read back the finished program. The reply says, per step, whether it applied, so a script that stops early tells you exactly which step the calculus refused and what the program looked like when it did. Steps already applied are kept; they are not rolled back.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "steps": {
              "type": "array",
              "description": "the actions to apply, in order",
              "items": {
                "description": "either a step string such as `construct-binop mul`, or a structured action object such as {\"action\":\"ConstructNum\",\"value\":42}",
                "anyOf": [
                  {
                    "type": "string"
                  },
                  {
                    "type": "object"
                  }
                ]
              }
            },
            "author": {
              "type": "integer",
              "description": "attribute these actions to this author id instead of the session default"
            }
          },
          "required": [
            "steps"
          ]
        }
      },
      {
        "name": "save_document",
        "description": "Write the current document to a file in the binary `NTHG` format that `nothing edit`, `nothing run` and `nothing check` read. The action log is written with it, so provenance and undo survive the round trip. There is no text format to write instead.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "path": {
              "type": "string",
              "description": "where to write the document"
            }
          },
          "required": [
            "path"
          ]
        }
      },
      {
        "name": "load_document",
        "description": "Read a document from a file and adopt it as the session's program, replacing whatever was being edited. The cursor lands in the first definition. Use this to continue work on a document saved earlier, or one built in the TUI editor.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "path": {
              "type": "string",
              "description": "the document to read"
            }
          },
          "required": [
            "path"
          ]
        }
      },
      {
        "name": "typecheck",
        "description": "Report whether the document is well typed, whether it is complete (no holes left), and the empty and non-empty hole counts for each definition by name. A `nothing` program is well typed at every instant by construction, so the interesting answer here is usually the hole count: it tells you how much of the program is still unwritten and which definition to go and fill in.",
        "inputSchema": {
          "type": "object",
          "properties": {},
          "required": []
        }
      },
      {
        "name": "run",
        "description": "Evaluate the definition named `main` and report the outcome. If `main` has a command type it is performed instead: `print` writes a line, `readline` reads one from the `stdin_lines` argument, `bind` sequences. What the program printed comes back inside this tool result, so a run never disturbs the protocol stream. A program with a hole on the path to its answer reports an indeterminate result and the holes it is blocked on rather than failing.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "fuel": {
              "type": "integer",
              "description": "the execution budget in steps; defaults to 200000"
            },
            "stdin_lines": {
              "type": "array",
              "description": "the lines `readline` should return, in order; it returns nothing once they run out",
              "items": {
                "type": "string",
                "description": "one line of input"
              }
            }
          },
          "required": []
        }
      },
      {
        "name": "stdlib",
        "description": "List the standard library: every name in scope that the document did not define, with its type and its doc line. These are callable with `construct-var NAME` exactly like the document's own definitions, and they are never written into a saved document.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "filter": {
              "type": "string",
              "description": "only list entries whose name or doc line contains this text"
            }
          },
          "required": []
        }
      },
      {
        "name": "action_grammar",
        "description": "The complete grammar of action names accepted by `apply_action` and `apply_actions`: every movement, every construction, every definition-level edit, and the type syntax `set-ann` and `set-def-ann` take. Read this once before your first edit.",
        "inputSchema": {
          "type": "object",
          "properties": {},
          "required": []
        }
      },
      {
        "name": "undo",
        "description": "Undo the last applied action by truncating the action log and replaying it. The program returns to exactly the state before that action; there is no partial undo, because there was no partial edit.",
        "inputSchema": {
          "type": "object",
          "properties": {},
          "required": []
        }
      },
      {
        "name": "redo",
        "description": "Re-apply the action that `undo` removed. Applying any new action after an undo discards what could have been redone.",
        "inputSchema": {
          "type": "object",
          "properties": {},
          "required": []
        }
      },
      {
        "name": "reset",
        "description": "Throw the document away and start again from a single empty definition with an empty action log. The standard library stays in scope.",
        "inputSchema": {
          "type": "object",
          "properties": {},
          "required": []
        }
      },
      {
        "name": "move_to_hole",
        "description": "Move the cursor to the next hole in this definition, wrapping around at the end, so you can fill a program in without counting `move-parent` and `move-child` steps yourself. Set `forward` to false to walk backwards.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "forward": {
              "type": "boolean",
              "description": "walk forwards to the next hole; defaults to true"
            }
          },
          "required": []
        }
      }
    ]
  }
}
```

## Configuring a host

### Claude Code

Build the binary, then register it. `claude mcp add` takes the server name, then `--`, then
the command and its arguments (verified against `claude mcp add --help`):

```sh
cargo build --release -p nothing-cli
claude mcp add nothing -- /absolute/path/to/nothing/target/release/nothing mcp
```

Add `-s user` to register it for every project rather than only the current one, or
`-s project` to write it into the repository's `.mcp.json` for everyone working on it. Check
it came up with `claude mcp list`, and remove it again with `claude mcp remove nothing`.

To attribute the model's edits separately from a human's, pass the flag through:

```sh
claude mcp add nothing -- /absolute/path/to/nothing mcp --author 2
```

### Any other MCP host

The same server as a raw stdio configuration block. This is the shape Claude Code writes into
`.mcp.json`, and the shape most other hosts accept:

```json
{
  "mcpServers": {
    "nothing": {
      "command": "/absolute/path/to/nothing",
      "args": ["mcp"],
      "env": {}
    }
  }
}
```

Use an absolute path. The server inherits no working directory assumptions, so `save_document`
and `load_document` paths should be absolute too.

## A worked example

**This is an automated run, not a Claude Code session.** The lines below were produced by
piping the request lines into `nothing mcp` and capturing stdout. They are pasted verbatim
except where a line is explicitly marked as elided for length. What a real Claude Code session
looks like is the HUMAN-REQUIRED item at the bottom of this file.

The goal: build a three-definition program that uses a string, a list and a standard-library
call, check it, run it, and save it.

```
greet : Str -> Str = λwho:Str. "hello, " ++ who
names : List Str = "world" :: "again" :: nil
main : Str = greet (join ", " names)
```

**1. The handshake.**

```json
{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "worked-example", "version": "1"}}}
```

```json
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"nothing","version":"0.1.0"},"instructions":"`nothing` is a projectional structural editor. There is no parser and no source text: you build a program by naming actions, and every action either applies — leaving a program that is still well typed — or is refused, changing nothing. Call `action_grammar` once to learn the action names, then loop: `hole_context` to see the expected type and the constructions that are well typed at the cursor, `apply_action` or `apply_actions` to edit, `get_projection` to read the program back. `typecheck` reports what is left to fill in, `run` evaluates `main`, and `save_document` writes a `.nothing` file that the editor and `nothing run` read."}}
```

**2. The initialized notification, which is answered with silence.**

```json
{"jsonrpc": "2.0", "method": "notifications/initialized"}
```

No response line. The next line on stdout belongs to the next request.

**3. `tools/list`** — request below; the reply is the block pasted in full above.

```json
{"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}
```

**4. `hole_context` at the empty program.** The reply's `structuredContent` is elided here for
length; this is its `content[0].text`, which is what a model reads, with the middle of the
in-scope list cut where marked.

```json
{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "hole_context", "arguments": {}}}
```

```
definition: main : ?
cursor path: []
focus: ⦇⦈ (EmptyHole)
expected type at cursor: ?
in scope:
  main : ?   (definition)   (fits the expected type)
  not : Bool -> Bool   (definition)   (fits the expected type)
  and : Bool -> Bool -> Bool   (definition)   (fits the expected type)
  ... 35 more standard-library bindings elided ...
well-typed constructions here:
  ... the constructions that fill a hole expecting ? — all of them ...
  construct-fold   ->   fold »⦇⦈« ⦇⦈ ⦇⦈
  construct-record   ->   {f0 = »⦇⦈«}
  construct-inj   ->   `C0 »⦇⦈«
  construct-match   ->   match »⦇⦈« {}
  construct-print   ->   print »⦇⦈«
  construct-readline   ->   »readline«
  construct-pure   ->   pure »⦇⦈«
  construct-bind   ->   bind x0 <- »⦇⦈« in ⦇⦈
other: delete, create-definition, set-def-ann <type>, rename-def <name>

Every construction listed above applies at this cursor and leaves no non-empty hole; anything not listed either does not apply or would not be well typed here. The bindings marked `(definition)` include the whole standard library — call `stdlib` with a `filter` to search it.
```

**5. Build the whole program with one `apply_actions` call.**

```json
{"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "apply_actions", "arguments": {"steps": ["rename-def greet", "set-def-ann Str -> Str", "construct-lam", "move-parent", "rename who", "set-ann Str", "move-child 0", "construct-str \"hello, \"", "construct-binop concat", "construct-var who", "create-definition", "rename-def names", "set-def-ann List Str", "construct-cons", "construct-str \"world\"", "move-parent", "move-child 1", "construct-cons", "construct-str \"again\"", "move-parent", "move-child 1", "construct-nil", "create-definition", "rename-def main", "set-def-ann Str", "construct-ap", "construct-var greet", "move-parent", "move-child 1", "construct-ap", "construct-ap", "construct-var join", "move-parent", "move-child 1", "construct-str \", \"", "move-parent", "move-parent", "move-child 1", "construct-var names"]}}}
```

```json
{"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"39 of 39 action(s) applied.\n  0: rename-def greet — applied\n  1: set-def-ann Str -> Str — applied\n  2: construct-lam — applied\n  3: move-parent — applied\n  4: rename who — applied\n  5: set-ann Str — applied\n  6: move-child 0 — applied\n  7: construct-str \"hello, \" — applied\n  8: construct-binop concat — applied\n  9: construct-var who — applied\n  10: create-definition — applied\n  11: rename-def names — applied\n  12: set-def-ann List Str — applied\n  13: construct-cons — applied\n  14: construct-str \"world\" — applied\n  15: move-parent — applied\n  16: move-child 1 — applied\n  17: construct-cons — applied\n  18: construct-str \"again\" — applied\n  19: move-parent — applied\n  20: move-child 1 — applied\n  21: construct-nil — applied\n  22: create-definition — applied\n  23: rename-def main — applied\n  24: set-def-ann Str — applied\n  25: construct-ap — applied\n  26: construct-var greet — applied\n  27: move-parent — applied\n  28: move-child 1 — applied\n  29: construct-ap — applied\n  30: construct-ap — applied\n  31: construct-var join — applied\n  32: move-parent — applied\n  33: move-child 1 — applied\n  34: construct-str \", \" — applied\n  35: move-parent — applied\n  36: move-parent — applied\n  37: move-child 1 — applied\n  38: construct-var names — applied\n\nthe document now reads:\n  greet : Str -> Str = λwho:Str. \"hello, \" ++ who\n  names : List Str = \"world\" :: \"again\" :: nil\n  main : Str = greet (join \", \" names)\n\nthe cursor sits in `main`, marked »…«:\n  greet (join \", \" »names«)\n\nexpected type at the cursor: List Str\nthe kind of node under the cursor: Var\nwell-typed: true; 3 definition(s) holding 0 empty and 0 non-empty hole(s)\n"}],"isError":false,"structuredContent":{"ok":true,"applied":true,"state":{"render_document":"greet : Str -> Str = λwho:Str. \"hello, \" ++ who\nnames : List Str = \"world\" :: \"again\" :: nil\nmain : Str = greet (join \", \" names)","render":"greet (join \", \" names)","render_with_cursor":"greet (join \", \" »names«)","definition_name":"main","definition_index":2,"definition_count":3,"cursor_path":[1,1],"focus_kind":"Var","expected_ty_text":"List Str","well_typed":true,"empty_holes":0,"non_empty_holes":0,"document_empty_holes":0,"document_non_empty_holes":0,"complete":true,"can_undo":true,"can_redo":false,"log_len":39}}}}
```

**6. `typecheck`.**

```json
{"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "typecheck", "arguments": {}}}
```

```json
{"jsonrpc":"2.0","id":5,"result":{"content":[{"type":"text","text":"well-typed: true\ncomplete: yes — there is no hole left anywhere in the document\ndefinitions (3):\n  greet : Str -> Str   0 empty hole(s), 0 non-empty hole(s)\n  names : List Str   0 empty hole(s), 0 non-empty hole(s)\n  main : Str   0 empty hole(s), 0 non-empty hole(s)\nstdlib definitions in scope: 37\n"}],"isError":false,"structuredContent":{"well_typed":true,"complete":true,"empty_holes":0,"non_empty_holes":0,"stdlib_definitions":37,"definitions":[{"name":"greet","ann":"Str -> Str","empty_holes":0,"non_empty_holes":0},{"name":"names","ann":"List Str","empty_holes":0,"non_empty_holes":0},{"name":"main","ann":"Str","empty_holes":0,"non_empty_holes":0}],"state":{"render_document":"greet : Str -> Str = λwho:Str. \"hello, \" ++ who\nnames : List Str = \"world\" :: \"again\" :: nil\nmain : Str = greet (join \", \" names)","render":"greet (join \", \" names)","render_with_cursor":"greet (join \", \" »names«)","definition_name":"main","definition_index":2,"definition_count":3,"cursor_path":[1,1],"focus_kind":"Var","expected_ty_text":"List Str","well_typed":true,"empty_holes":0,"non_empty_holes":0,"document_empty_holes":0,"document_non_empty_holes":0,"complete":true,"can_undo":true,"can_redo":false,"log_len":39}}}}
```

**7. `run`.**

```json
{"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "run", "arguments": {}}}
```

```json
{"jsonrpc":"2.0","id":6,"result":{"content":[{"type":"text","text":"`main` was evaluated.\n\nthe program printed nothing.\n\n\"hello, world, again\"\n\nexit status 0: the run produced a value\n"}],"isError":false,"structuredContent":{"performed":false,"status":0,"value":"\"hello, world, again\"","printed":[],"report":["\"hello, world, again\""],"fuel":200000}}}
```

**8. `save_document`.**

```json
{"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "save_document", "arguments": {"path": "/tmp/greeting.nothing"}}}
```

```json
{"jsonrpc":"2.0","id":7,"result":{"content":[{"type":"text","text":"wrote 1728 bytes to /tmp/greeting.nothing.\n\nthe document now reads:\n  greet : Str -> Str = λwho:Str. \"hello, \" ++ who\n  names : List Str = \"world\" :: \"again\" :: nil\n  main : Str = greet (join \", \" names)\n\nthe cursor sits in `main`, marked »…«:\n  greet (join \", \" »names«)\n\nexpected type at the cursor: List Str\nthe kind of node under the cursor: Var\nwell-typed: true; 3 definition(s) holding 0 empty and 0 non-empty hole(s)\n"}],"isError":false,"structuredContent":{"ok":true,"applied":true,"state":{"render_document":"greet : Str -> Str = λwho:Str. \"hello, \" ++ who\nnames : List Str = \"world\" :: \"again\" :: nil\nmain : Str = greet (join \", \" names)","render":"greet (join \", \" names)","render_with_cursor":"greet (join \", \" »names«)","definition_name":"main","definition_index":2,"definition_count":3,"cursor_path":[1,1],"focus_kind":"Var","expected_ty_text":"List Str","well_typed":true,"empty_holes":0,"non_empty_holes":0,"document_empty_holes":0,"document_non_empty_holes":0,"complete":true,"can_undo":true,"can_redo":false,"log_len":39}}}}
```

**9. The file the ordinary tools read.** The document the model saved needs nothing else done
to it:

```
$ nothing check /tmp/greeting.nothing
well-typed: true
definitions: 3
stdlib definitions in scope: 37
empty holes: 0
non-empty holes: 0

$ nothing run /tmp/greeting.nothing
"hello, world, again"
```

## What the automated tests cover

`cli/tests/mcp.rs` spawns the real binary and speaks MCP to it over pipes. It asserts the
handshake shape and the version negotiation, that `notifications/initialized` produces no
output line, that every tool in `tools/list` has a description and a well-formed
`inputSchema`, that a program built through tool calls saves and reloads into a *fresh server
process* still well typed and complete, that a command's `print` output arrives in the tool
result rather than on stdout and that every stdout line is a JSON-RPC message, that a
malformed line is a `-32700` and the server survives it, that an unknown tool is `isError`
rather than a crash, and that an unknown method is `-32601`.

## Session transcript

**HUMAN-REQUIRED.**

The Phase B5 done-when for this item reads:

> **Done when** a Claude Code session with the server configured builds and saves a working
> program in one conversation, transcript committed under bench/.

That last clause cannot be closed by an automated agent, and nothing in this file pretends
otherwise. An agent running inside Claude Code cannot start a second Claude Code session
against its own host and record it, and a transcript that was not produced by a real
conversation would be a fabrication — which the honesty guard in `spec-build.md` rules out
more firmly than any missing feature would. The worked example above is labelled as what it
is: a piped, scripted run.

So this is the one step a maintainer has to take by hand.

**1. Register the server.**

```sh
cargo build --release -p nothing-cli
claude mcp add nothing -- "$(pwd)/target/release/nothing" mcp
claude mcp list
```

**2. Start a fresh Claude Code session** in this repository — a fresh one matters, because
half the point is whether a model that has not been coached can work out the editing model
from `tools/list` alone.

**3. Give it a goal and nothing else.** A suggested prompt, deliberately not naming any
action:

> Using only the `nothing` MCP server's tools, build a small program in the `nothing`
> language: a definition `celsius` holding a list of numbers, a definition `to_fahrenheit`
> that converts one Celsius number to Fahrenheit, and a `main` that maps it over the list and
> joins the results into one string. Check it typechecks and has no holes left, run it, and
> save it to `bench/fixtures/mcp-session.nothing`. Do not write any program text — the server
> has no parser.

Anything of comparable size works. What is being tested is one conversation, ending in a
saved, well-typed, hole-free program.

**4. Export the transcript.** Claude Code keeps every session as JSONL at
`~/.claude/projects/<repo-path-with-slashes-as-dashes>/<session-id>.jsonl` — verified to exist
on this machine — so the surest route is to copy that file, or to render the tool calls out of
it. If your build of Claude Code has an `/export` slash command, that writes the conversation
straight out and is easier to read.

**5. Commit it** over `bench/agent-transcripts/mcp-session.md`, which is currently a
placeholder saying exactly this, alongside the existing `factorial.jsonl` and friends. Then
replace this section's HUMAN-REQUIRED marker with a link to it plus two or three sentences on
what actually happened: how many tool calls, which ones the model reached for, what it got
wrong first, and whether the program it saved runs. If the model could not do it in one
conversation, say that instead — the tool descriptions are the thing to fix, and the honest
failed run is more useful than a second attempt with a coached prompt.

**6. Then tick the Phase B5 box in `spec-build.md`,** and not before.
