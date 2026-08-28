use std::io::{BufRead, Write};

use nothing_action::log::AuthorId;
use nothing_agentapi::protocol::handle_line;
use nothing_agentapi::session::AgentSession;

fn author_from_args() -> AuthorId {
    let args: Vec<String> = std::env::args().collect();
    let mut author = 1u64;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--author" {
            if let Some(value) = args.get(i + 1).and_then(|v| v.parse::<u64>().ok()) {
                author = value;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    AuthorId::new(author)
}

fn main() {
    let mut session = AgentSession::new(author_from_args());
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                eprintln!("error: could not read stdin: {err}");
                std::process::exit(1);
            }
        };
        let Some(outcome) = handle_line(&mut session, &line) else {
            continue;
        };
        if writeln!(stdout, "{}", outcome.value).is_err() {
            return;
        }
        if stdout.flush().is_err() {
            return;
        }
        if outcome.quit {
            return;
        }
    }
}
