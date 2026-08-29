mod rpc;
mod server;
mod tools;

pub use server::run;

pub const HELP: &str = "\
nothing mcp [--author N] [--no-stdlib]

Serve the editor to any MCP-speaking agent host over stdio: one JSON-RPC 2.0
message per line in, one per line out. Nothing but protocol messages is ever
written to stdout; diagnostics go to stderr. Protocol versions 2025-06-18,
2025-03-26 and 2024-11-05 are accepted.

There is no parser. An agent edits by naming actions — `construct-lam`,
`construct-var xs`, `move-child 0` — and each one either applies, leaving a
well-typed program, or is refused. The tools are get_state, get_projection,
hole_context, apply_action, apply_actions, save_document, load_document,
typecheck, run, stdlib, action_grammar, undo, redo, reset and move_to_hole.

Configure it in Claude Code with:
  claude mcp add nothing -- /path/to/nothing mcp
bench/MCP.md has the raw JSON config block and the full tool reference.

The standard library is in scope by default.

Options:
  --author N     attribute applied actions to author id N (default 1)
  --no-stdlib    start with an empty prelude, as the stdlib itself was built
  -h, --help     print this help and exit";
