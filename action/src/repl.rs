use std::io::{BufRead, Write};

use crate::act::EditState;
use crate::cursor_render::render_with_cursor;
use crate::script::{Command, HELP, parse_command, step_name};

pub fn run<R: BufRead, W: Write, X: Write>(
    input: R,
    mut output: W,
    mut errput: X,
    interactive: bool,
) -> i32 {
    let mut state = EditState::empty();
    let mut rejected = 0usize;

    if interactive {
        let _ = writeln!(
            output,
            "nothing REPL harness — `help` for commands, `quit` to stop."
        );
        let _ = writeln!(
            output,
            "{}",
            render_with_cursor(&state.zipper, &state.names)
        );
    }

    for line in input.lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                let _ = writeln!(errput, "error: could not read stdin: {err}");
                return 1;
            }
        };

        match parse_command(&line) {
            Err(err) => {
                let _ = writeln!(errput, "error: {err}");
                rejected += 1;
            }
            Ok(None) => {}
            Ok(Some(Command::Quit)) => break,
            Ok(Some(Command::Help)) => {
                let _ = writeln!(output, "{HELP}");
            }
            Ok(Some(Command::Show)) => {
                let _ = writeln!(
                    output,
                    "{}",
                    render_with_cursor(&state.zipper, &state.names)
                );
            }
            Ok(Some(Command::Reset)) => {
                state = EditState::empty();
                let _ = writeln!(
                    output,
                    "{}",
                    render_with_cursor(&state.zipper, &state.names)
                );
            }
            Ok(Some(Command::Act(step))) => match step.resolve(&state) {
                Ok(action) if state.apply_mut(action.clone()) => {
                    let _ = writeln!(
                        output,
                        "{}",
                        render_with_cursor(&state.zipper, &state.names)
                    );
                }
                Ok(_) => {
                    let _ = writeln!(
                        errput,
                        "error: action does not apply here: {}",
                        step_name(&step)
                    );
                    rejected += 1;
                }
                Err(err) => {
                    let _ = writeln!(errput, "error: {err}");
                    rejected += 1;
                }
            },
        }
        let _ = output.flush();
    }

    if rejected > 0 {
        let _ = writeln!(errput, "{rejected} line(s) rejected");
        return 1;
    }
    0
}
