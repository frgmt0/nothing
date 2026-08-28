use std::io::{self, IsTerminal};

fn main() {
    let stdin = io::stdin();
    let interactive = stdin.is_terminal();
    let code = nothing_action::repl::run(stdin.lock(), io::stdout(), io::stderr(), interactive);
    std::process::exit(code);
}
