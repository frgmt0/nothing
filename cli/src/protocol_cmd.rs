use std::io;

use nothing_agentapi::protocol::{author_from_args, run_stdio};
use nothing_agentapi::session::AgentSession;

pub const HELP: &str = "\
nothing protocol [--author N]

Speak the nothing agent protocol over stdio: one JSON request per line in,
one JSON response per line out. Send {\"method\":\"help\"} for the method
list.

The standard library is in scope by default. Send {\"method\":\"stdlib\"} once
for the catalogue; `state` then carries only the document's own vocabulary
plus the stdlib names it actually references, and `state.stdlib_count` says
how many more there are. `hole_context` bindings are marked
`\"stdlib\": true`. Stdlib definitions are never written into a saved
document.

Options:
  --author N     attribute applied actions to author id N (default 1)
  --no-stdlib    start with an empty prelude, as the stdlib itself was built
  -h, --help     print this help and exit";

pub fn run(args: &[String]) -> i32 {
    let mut session = if args.iter().any(|a| a == "--no-stdlib") {
        AgentSession::new(author_from_args(args))
    } else {
        AgentSession::from_base(
            nothing_action::act::EditState::empty().under(nothing_stdlib::prelude()),
            nothing_action::log::ActionLog::new(),
            author_from_args(args),
        )
    };
    let stdin = io::stdin();
    match run_stdio(&mut session, stdin.lock(), io::stdout()) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    }
}
