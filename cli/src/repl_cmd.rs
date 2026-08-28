use std::io::{self, IsTerminal};

pub const HELP: &str = "\
nothing repl

Start the action-name REPL: type one action name per line (`help` lists
them), `show` re-renders the program, `reset` clears it, `quit` stops.

Options:
  -h, --help   print this help and exit";

pub fn run() -> i32 {
    let stdin = io::stdin();
    let interactive = stdin.is_terminal();
    nothing_action::repl::run(stdin.lock(), io::stdout(), io::stderr(), interactive)
}
