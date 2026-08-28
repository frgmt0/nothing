
use std::path::PathBuf;

use nothing_action::script::{parse_script, replay_script};
use nothing_core::typing::is_well_typed;

struct ReferenceProgram {
    name: &'static str,
    reference: usize,
    fixture: &'static str,
    keys_fixture: &'static str,
    neovim_keystrokes: usize,
    approximate: bool,
}

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

fn fixture_dir() -> PathBuf {
    match std::env::var_os("NOTHING_BENCH_FIXTURES") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures"),
    }
}

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

    #[cfg(test)]
    fn expected_path(&self) -> PathBuf {
        fixture_dir().join(self.fixture.replace(".actions", ".expected"))
    }

    fn read(&self) -> Result<String, String> {
        let path = self.path();
        std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))
    }

    fn action_count(&self) -> Result<usize, String> {
        let text = self.read()?;
        parse_script(&text)
            .map(|actions| actions.len())
            .map_err(|e| e.to_string())
    }

    fn replay(&self) -> Result<String, String> {
        let text = self.read()?;
        let state = replay_script(&text).map_err(|e| e.to_string())?;
        let exp = state.exp();
        if !is_well_typed(&exp) {
            return Err(format!("{}: replayed to an ill-typed program", self.name));
        }
        Ok(state.render())
    }

    fn ratio(&self, actions: usize) -> f64 {
        actions as f64 / self.neovim_keystrokes as f64
    }

    fn keys_path(&self) -> PathBuf {
        keys_dir().join(self.keys_fixture)
    }

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