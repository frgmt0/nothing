use std::path::Path;

use nothing_tui::tutorial;

use crate::edit;
use crate::run_cmd;

pub const DEFAULT_FILE: &str = tutorial::DEFAULT_FILE;

pub const HELP: &str = "\
nothing tutorial [<file>]

A guided first session inside the real editor. Nine steps build a small
program: write a function, fill a hole, rename a definition, cause a
quarantine and repair it, with an instruction pane beside the program.

Progress is read off the document itself: every step is a structural check
on the program you are building, never on what was printed. Quitting with
ctrl-q and opening the same file again resumes where you left off, and
nothing is written anywhere but <file>.

<file> defaults to `tutorial.n` in the current directory. It is an ordinary
document: `nothing edit <file>` and `nothing run <file>` work on it
afterwards. When every step is done, quitting saves <file> and then performs
it, so the last thing the tutorial teaches is what a run looks like.

Options:
  -h, --help   print this help and exit";

pub fn run(path: &Path) -> i32 {
    let (initial, base_log) = match edit::load(path) {
        Ok(pair) => pair,
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    };

    println!("tutorial: editing {}", path.display());
    let started = tutorial::begin(initial, path.display().to_string());

    let final_state = match nothing_tui::term::run(started) {
        Ok(state) => state,
        Err(err) => {
            eprintln!("error: terminal session failed: {err}");
            return 1;
        }
    };

    if let Err(err) = edit::save(path, &final_state, base_log) {
        eprintln!("error: {err}");
        return 1;
    }
    println!("tutorial: saved {}", path.display());

    if !tutorial::is_complete(&final_state) {
        let step = final_state
            .tutorial
            .as_ref()
            .map(|t| t.step + 1)
            .unwrap_or(1);
        println!(
            "tutorial: stopped on step {step} of {}. `nothing tutorial {}` resumes here",
            tutorial::STEPS.len(),
            path.display()
        );
        return 0;
    }

    println!("tutorial: running {}", path.display());
    run_cmd::run_with_fuel(path, nothing_eval::DEFAULT_FUEL)
}
