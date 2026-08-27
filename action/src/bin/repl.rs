//! The throwaway REPL harness (Phase 3).
//!
//! Reads one action name per line from stdin, applies it, and prints the
//! program with the cursor after each. No TUI, no keybindings, no
//! completion — this exists so that programs can be built and reference
//! action sequences recorded *before* Phase 4 designs the keyboard grammar,
//! and it is expected to be thrown away once the editor exists.
//!
//! ```text
//! $ printf 'construct-num 1\nconstruct-binop add\nconstruct-num 2\n' \
//!     | cargo run -p nothing-action --bin repl
//! ```
//!
//! Every line is one of the commands listed by `help`. Unknown or malformed
//! input prints an error on stderr and the session continues — a harness
//! that dies on a typo is useless for recording fixtures. An action that
//! parses but does not apply at the cursor (`move-parent` at the root,
//! `finish` off a non-empty hole) is likewise reported and skipped: that is
//! the action judgment's clean-failure outcome, not a harness error.
//!
//! Exit status is 0 on clean EOF or `quit`, and 1 if anything was rejected,
//! so that piping a fixture through the harness is a usable check.

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
