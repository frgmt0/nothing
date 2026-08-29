# Claude Code session transcript — `nothing mcp`

**PLACEHOLDER. HUMAN-REQUIRED. There is no transcript here yet, and nothing below is a
record of anything that happened.**

Phase B5's done-when for the MCP server asks for a real Claude Code session, with the server
configured, that builds and saves a working program in one conversation. That is a human step:
an agent running inside Claude Code cannot start a second Claude Code session against its own
host and record it, and inventing a plausible-looking conversation would be a fabrication.

The server itself is finished and tested — see `bench/MCP.md` for the tool reference, the
configuration command, and a worked example from a scripted (not conversational) run, plus
`cli/tests/mcp.rs` for the integration tests that speak MCP to the real binary.

## What to do

The step-by-step instructions live in the `## Session transcript` section of `bench/MCP.md`.
In short:

1. `cargo build --release -p nothing-cli`
2. `claude mcp add nothing -- "$(pwd)/target/release/nothing" mcp`
3. Start a fresh Claude Code session and give it a build goal without naming any action.
4. Export the conversation and replace this whole file with it.
5. Add a short honest note above the transcript: how many tool calls it took, which tools the
   model reached for, what it got wrong first, and whether the saved program runs. If the
   model could not finish in one conversation, record that instead of retrying with a coached
   prompt — the tool descriptions are the thing to fix.
6. Then, and only then, tick the MCP server box in `spec-build.md`.
