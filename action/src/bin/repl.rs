
use std::io::{BufRead, Write};

use nothing_action::act::EditState;
use nothing_action::cursor_render::render_with_cursor;
use nothing_action::script::{Command, parse_command, HELP};

fn main() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let interactive = std::io::IsTerminal::is_terminal(&stdin);

    let mut state = EditState::empty();
    let mut rejected = 0usize;

    if interactive {
        println!("nothing REPL harness — `help` for commands, `quit` to stop.");
        println!("{}", render_with_cursor(&state.zipper));
    }

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                eprintln!("error: could not read stdin: {err}");
                std::process::exit(1);
            }
        };

        match parse_command(&line) {
            Err(err) => {
                eprintln!("error: {err}");
                rejected += 1;
            }
            Ok(None) => {}
            Ok(Some(Command::Quit)) => break,
            Ok(Some(Command::Help)) => println!("{HELP}"),
            Ok(Some(Command::Show)) => println!("{}", render_with_cursor(&state.zipper)),
            Ok(Some(Command::Reset)) => {
                state = EditState::empty();
                println!("{}", render_with_cursor(&state.zipper));
            }
            Ok(Some(Command::Act(action))) => {
                if state.apply_mut(action.clone()) {
                    println!("{}", render_with_cursor(&state.zipper));
                } else {
                    eprintln!(
                        "error: action does not apply here: {}",
                        nothing_action::script::action_name(&action)
                    );
                    rejected += 1;
                }
            }
        }
        let _ = stdout.flush();
    }

    if rejected > 0 {
        eprintln!("{rejected} line(s) rejected");
        std::process::exit(1);
    }
}