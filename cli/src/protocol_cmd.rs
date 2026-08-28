use std::io;

use nothing_agentapi::protocol::{author_from_args, run_stdio};
use nothing_agentapi::session::AgentSession;

pub const HELP: &str = "\
nothing protocol [--author N]

Speak the nothing agent protocol over stdio: one JSON request per line in,
one JSON response per line out. Send {\"method\":\"help\"} for the method
list.

Options:
  --author N   attribute applied actions to author id N (default 1)
  -h, --help   print this help and exit";

pub fn run(args: &[String]) -> i32 {
    let mut session = AgentSession::new(author_from_args(args));
    let stdin = io::stdin();
    match run_stdio(&mut session, stdin.lock(), io::stdout()) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    }
}
