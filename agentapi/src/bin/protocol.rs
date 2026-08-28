use std::io;

use nothing_agentapi::protocol::{author_from_args, run_stdio};
use nothing_agentapi::session::AgentSession;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut session = AgentSession::new(author_from_args(&args));
    let stdin = io::stdin();
    run_stdio(&mut session, stdin.lock(), io::stdout())
}
