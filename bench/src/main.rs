//! `nothing-bench`: the keystroke benchmark harness.
//!
//! It takes a named reference program and a recorded sequence of editor
//! actions and reports the count, then divides by the Neovim keystroke
//! baseline fixed in `bench/references.md` to get the ratio that Phase 0's
//! failure-mode guard is stated in terms of.
//!
//! The recorded sequences live in `bench/fixtures/<name>.actions`, one
//! action per line in the syntax accepted by the Phase 3 REPL harness
//! (`cargo run -p nothing-action --bin repl`). They are replayed through
//! the real action calculus — this harness never contains a hand-written
//! count, it always replays and counts what actually applied.
//!
//! Phase 4 adds a second, more honest denominator on the `nothing` side:
//! the `.keys` fixtures in `tui/tests/keys/<name>.keys` record what a
//! *person at the keyboard* presses — one keystroke per line, exactly the
//! format `nothing-tui`'s own `keyscript` module reads and replays through
//! the pure key handler (see `tui/tests/references.rs`, which is the test
//! that keeps these fixtures honest against the actual TUI). The old
//! action-count mode (`count`/`table`, against `bench/fixtures/*.actions`)
//! stays exactly as it was — it measures primitive actions, which is a
//! different and still-useful number — and the new `keystrokes`/`keytable`
//! commands measure keystrokes, which is the number Phase 0's 3× guard is
//! actually stated in terms of.

use std::path::PathBuf;

use nothing_action::script::{parse_script, replay_script};
use nothing_core::render::render;
use nothing_core::typing::is_well_typed;

/// A reference program benchmarked by keystroke/action count.
///
/// `fixture` names the file in `bench/fixtures` holding the recorded
/// sequence of editor actions that builds this program from an empty hole;
/// `neovim_keystrokes` is the permanent baseline from `bench/references.md`.
struct ReferenceProgram {
    name: &'static str,
    /// The reference-program number in `bench/references.md`.
    reference: usize,
    fixture: &'static str,
    /// The `.keys` fixture in `tui/tests/keys` that builds this program one
    /// keystroke at a time through the real TUI key handler. `None` would
    /// mean no keyboard fixture exists yet; all five references have one.
    keys_fixture: &'static str,
    neovim_keystrokes: usize,
    /// Whether the fixture is an exact rendering of the reference program
    /// or an approximation forced by the Phase-1 surface. See
    /// `bench/references.md` for the mapping.
    approximate: bool,
}

/// The five reference programs chosen in Phase 0 (`bench/references.md`),
/// each with the Phase 3 action fixture that builds its nearest Phase-1
/// equivalent.
fn reference_programs() -> Vec<ReferenceProgram> {
    vec![
        ReferenceProgram {
            name: "factorial",
            reference: 1,
            fixture: "factorial.actions",
            keys_fixture: "factorial.keys",
            neovim_keystrokes: 84,
            approximate: true,
        },
        ReferenceProgram {
            name: "list_map",
            reference: 2,
            fixture: "list_map.actions",
            keys_fixture: "list_map.keys",
            neovim_keystrokes: 114,
            approximate: true,
        },
        ReferenceProgram {
            name: "record",
            reference: 3,
            fixture: "record.actions",
            keys_fixture: "record.keys",
            neovim_keystrokes: 65,
            approximate: true,
        },
        ReferenceProgram {
            name: "state_machine",
            reference: 4,
            fixture: "state_machine.actions",
            keys_fixture: "state_machine.keys",
            neovim_keystrokes: 151,
            approximate: true,
        },
        ReferenceProgram {
            name: "nested_conditional",
            reference: 5,
            fixture: "nested_conditional.actions",
            keys_fixture: "nested_conditional.keys",
            neovim_keystrokes: 146,
            approximate: false,
        },
    ]
}

/// The directory holding the recorded action fixtures.
///
/// Defaults to `bench/fixtures` next to this crate's manifest; override with
/// `NOTHING_BENCH_FIXTURES` when running the binary from somewhere else.
fn fixture_dir() -> PathBuf {
    match std::env::var_os("NOTHING_BENCH_FIXTURES") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures"),
    }
}

/// The directory holding the recorded `.keys` keyboard fixtures.
///
/// These belong to `nothing-tui` (they are also its own acceptance test,
/// `tui/tests/references.rs`), not to `bench`; `bench` only reads them.
/// Defaults to `../tui/tests/keys` next to this crate's manifest; override
/// with `NOTHING_BENCH_KEYS` when running the binary from somewhere else.
fn keys_dir() -> PathBuf {
    match std::env::var_os("NOTHING_BENCH_KEYS") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tui/tests/keys"),
    }
}

impl ReferenceProgram {
    fn path(&self) -> PathBuf {
        fixture_dir().join(self.fixture)
    }

    /// The committed expected rendering, next to the fixture. Read by the
    /// replay test, which is the thing that keeps the fixtures honest.
    #[cfg(test)]
    fn expected_path(&self) -> PathBuf {
        fixture_dir().join(self.fixture.replace(".actions", ".expected"))
    }

    fn read(&self) -> Result<String, String> {
        let path = self.path();
        std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))
    }

    /// The number of actions in the fixture. Comments and blank lines do
    /// not count — only edits do, or the ratio would be a measure of how
    /// chatty the fixture's comments are.
    fn action_count(&self) -> Result<usize, String> {
        let text = self.read()?;
        parse_script(&text)
            .map(|actions| actions.len())
            .map_err(|e| e.to_string())
    }

    /// Replay the fixture through the action calculus and render the result.
    fn replay(&self) -> Result<String, String> {
        let text = self.read()?;
        let state = replay_script(&text).map_err(|e| e.to_string())?;
        let exp = state.exp();
        if !is_well_typed(&exp) {
            return Err(format!("{}: replayed to an ill-typed program", self.name));
        }
        Ok(render(&exp))
    }

    fn ratio(&self, actions: usize) -> f64 {
        actions as f64 / self.neovim_keystrokes as f64
    }

    fn keys_path(&self) -> PathBuf {
        keys_dir().join(self.keys_fixture)
    }

    /// The number of keystrokes in the `.keys` fixture: one non-blank,
    /// non-comment line is one keystroke, by construction of the format
    /// (`tui/src/keyscript.rs`). This is deliberately a plain line count,
    /// not a call into `nothing-tui`'s parser: the number the 3× guard is
    /// stated in terms of has to be legible by inspection of the fixture
    /// file, the same way the Neovim baselines in `references.md` are.
    /// `nothing-tui`'s own `tui/tests/references.rs` is what proves this
    /// count agrees with `keyscript::parse_keys` and with the pure key
    /// handler actually accepting the fixture.
    fn keystroke_count(&self) -> Result<usize, String> {
        let path = self.keys_path();
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Ok(text
            .lines()
            .filter(|line| {
                let line = line.trim();
                !line.is_empty() && !line.starts_with('#')
            })
            .count())
    }

    fn keystroke_ratio(&self, keystrokes: usize) -> f64 {
        keystrokes as f64 / self.neovim_keystrokes as f64
    }
}

fn print_usage() {
    println!("nothing-bench: keystroke benchmark harness");
    println!();
    println!("USAGE:");
    println!("    nothing-bench list             list the reference programs and their fixtures");
    println!("    nothing-bench count NAME       print the action count for NAME");
    println!("    nothing-bench run NAME         replay NAME's fixture and print the program");
    println!("    nothing-bench table            print the action-count ratio table (markdown)");
    println!("    nothing-bench keystrokes NAME  print the keystroke count for NAME");
    println!("    nothing-bench keytable         print the keystroke ratio table (markdown)");
}

fn find(name: &str) -> ReferenceProgram {
    match reference_programs().into_iter().find(|p| p.name == name) {
        Some(program) => program,
        None => {
            eprintln!("error: no reference program named `{name}`");
            eprintln!(
                "known: {}",
                reference_programs()
                    .iter()
                    .map(|p| p.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            std::process::exit(1);
        }
    }
}

fn unwrap_or_exit<T>(result: Result<T, String>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(String::as_str);

    match command {
        Some("list") => {
            for program in reference_programs() {
                let count = match program.action_count() {
                    Ok(count) => format!("{count} actions"),
                    Err(err) => format!("unavailable: {err}"),
                };
                println!(
                    "{:<20} references.md §{}  {:<14} {}  (Neovim {})",
                    program.name,
                    program.reference,
                    count,
                    program.path().display(),
                    program.neovim_keystrokes,
                );
            }
        }
        Some("count") => {
            let Some(name) = args.get(2) else {
                eprintln!("error: `count` requires a reference program name");
                print_usage();
                std::process::exit(2);
            };
            println!("{}", unwrap_or_exit(find(name).action_count()));
        }
        Some("run") => {
            let Some(name) = args.get(2) else {
                eprintln!("error: `run` requires a reference program name");
                print_usage();
                std::process::exit(2);
            };
            let program = find(name);
            let rendered = unwrap_or_exit(program.replay());
            let actions = unwrap_or_exit(program.action_count());
            println!("{rendered}");
            println!(
                "{actions} actions / {} Neovim keystrokes = {:.2}x",
                program.neovim_keystrokes,
                program.ratio(actions),
            );
        }
        Some("table") => {
            println!("| # | Program | Neovim keystrokes | `nothing` actions | Ratio |");
            println!("|---|---------|------------------:|------------------:|------:|");
            for program in reference_programs() {
                let actions = unwrap_or_exit(program.action_count());
                println!(
                    "| {} | {}{} | {} | {} | {:.2}x |",
                    program.reference,
                    program.name,
                    if program.approximate { " *" } else { "" },
                    program.neovim_keystrokes,
                    actions,
                    program.ratio(actions),
                );
            }
        }
        Some("keystrokes") => {
            let Some(name) = args.get(2) else {
                eprintln!("error: `keystrokes` requires a reference program name");
                print_usage();
                std::process::exit(2);
            };
            println!("{}", unwrap_or_exit(find(name).keystroke_count()));
        }
        Some("keytable") => {
            println!("| # | Program | Neovim keystrokes | `nothing` keystrokes | Ratio |");
            println!("|---|---------|------------------:|----------------------:|------:|");
            for program in reference_programs() {
                let keystrokes = unwrap_or_exit(program.keystroke_count());
                println!(
                    "| {} | {}{} | {} | {} | {:.2}x |",
                    program.reference,
                    program.name,
                    if program.approximate { " *" } else { "" },
                    program.neovim_keystrokes,
                    keystrokes,
                    program.keystroke_ratio(keystrokes),
                );
            }
        }
        _ => print_usage(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_five_reference_programs() {
        assert_eq!(reference_programs().len(), 5);
    }

    /// Phase 3's "all five fixtures exist and replay cleanly": every
    /// recorded action applies, the result is well-typed, and it renders
    /// exactly as the committed `.expected` file says.
    #[test]
    fn every_fixture_replays_to_its_committed_rendering() {
        for program in reference_programs() {
            let rendered = program
                .replay()
                .unwrap_or_else(|e| panic!("{}: {e}", program.name));
            let expected_path = program.expected_path();
            let expected = std::fs::read_to_string(&expected_path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", expected_path.display()));
            assert_eq!(
                rendered,
                expected.trim_end_matches('\n'),
                "{} replayed to a different program than {} records",
                program.name,
                expected_path.display(),
            );
        }
    }

    /// The guard against a fixture quietly turning into a no-op: the
    /// benchmark would still "pass" with an empty fixture, and the ratio
    /// would look wonderful.
    #[test]
    fn no_fixture_is_trivial() {
        for program in reference_programs() {
            let count = program
                .action_count()
                .unwrap_or_else(|e| panic!("{}: {e}", program.name));
            assert!(
                count >= 10,
                "{} is only {count} actions — that is not a reference program",
                program.name
            );
            let rendered = program.replay().unwrap();
            assert!(
                rendered.len() > 20,
                "{} renders as `{rendered}`, which is not the reference program",
                program.name
            );
        }
    }

    /// Only factorial is allowed to still contain a hole: it is the one
    /// reference program whose missing piece (the recursive call) cannot be
    /// written before Phase 6. Everything else must be complete.
    #[test]
    fn only_factorial_is_left_unfinished() {
        for program in reference_programs() {
            let rendered = program.replay().unwrap();
            let has_hole = rendered.contains('⦇');
            assert_eq!(
                has_hole,
                program.name == "factorial",
                "{} renders as `{rendered}`",
                program.name
            );
        }
    }

    #[test]
    fn every_ratio_is_computable_and_recorded_in_results_md() {
        let results = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("RESULTS.md");
        let text = std::fs::read_to_string(&results)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", results.display()));
        for program in reference_programs() {
            let actions = program.action_count().unwrap();
            let ratio = format!("{:.2}x", program.ratio(actions));
            assert!(
                text.contains(&ratio),
                "RESULTS.md does not record {}'s ratio of {ratio}",
                program.name
            );
            assert!(
                text.contains(program.name),
                "RESULTS.md does not mention {}",
                program.name
            );
        }
        assert!(
            text.contains("2026-08-26"),
            "RESULTS.md is not dated with the run date"
        );
        assert!(
            text.to_lowercase().contains("pre-keybinding"),
            "RESULTS.md is missing the note that these are pre-keybinding counts"
        );
    }

    /// Every reference program has a `.keys` fixture and it is not a stub.
    #[test]
    fn every_keys_fixture_exists_and_is_nontrivial() {
        for program in reference_programs() {
            let count = program
                .keystroke_count()
                .unwrap_or_else(|e| panic!("{}: {e}", program.name));
            assert!(
                count >= 10,
                "{} is only {count} keystrokes — that is not a reference program",
                program.name
            );
        }
    }

    /// Phase 0's guard, stated in the unit it was actually stated in:
    /// keystrokes, not actions. This is the tripwire `nothing-bench` itself
    /// carries; `tui/tests/references.rs` carries the same tripwire against
    /// the live key handler, so a regression trips in two places.
    #[test]
    fn no_keystroke_ratio_exceeds_the_three_times_guard() {
        for program in reference_programs() {
            let keystrokes = program.keystroke_count().unwrap();
            let ratio = program.keystroke_ratio(keystrokes);
            assert!(
                ratio <= 3.0,
                "{}: {keystrokes} keystrokes against a baseline of {} is {ratio:.2}x — Phase 0's \
                 guard is breached, stop and fix the grammar",
                program.name,
                program.neovim_keystrokes
            );
        }
    }

    /// Phase 4's re-run of the benchmark: the dated keystroke ratios have to
    /// actually be written into `RESULTS.md`, in the units the guard is
    /// stated in.
    #[test]
    fn every_keystroke_ratio_is_recorded_in_results_md() {
        let results = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("RESULTS.md");
        let text = std::fs::read_to_string(&results)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", results.display()));
        for program in reference_programs() {
            let keystrokes = program.keystroke_count().unwrap();
            let ratio = format!("{:.2}x", program.keystroke_ratio(keystrokes));
            assert!(
                text.contains(&ratio),
                "RESULTS.md does not record {}'s keystroke ratio of {ratio}",
                program.name
            );
        }
        assert!(
            text.contains("2026-08-27"),
            "RESULTS.md is not dated with the Phase 4 keystroke run date"
        );
    }
}
