//! `nothing-bench`: the keystroke benchmark harness.
//!
//! It takes a named reference program and a recorded sequence of editor
//! actions, and reports the count. It does not need an editor yet — it
//! counts actions applied programmatically, once Phase 2 supplies the action
//! calculus and Phase 3 supplies recorded action sequences for the five
//! reference programs in `bench/references.md`.

/// A reference program benchmarked by keystroke/action count.
///
/// `actions` is the recorded sequence of editor action names (see Phase 3)
/// that builds this program from an empty hole. It is empty until later
/// phases record real sequences.
struct ReferenceProgram {
    name: &'static str,
    actions: Vec<&'static str>,
}

/// The set of reference programs known to the benchmark. Empty until Phase 3
/// records action sequences for the five programs chosen in Phase 0
/// (`bench/references.md`).
fn reference_programs() -> Vec<ReferenceProgram> {
    Vec::new()
}

fn print_usage() {
    println!("nothing-bench: keystroke benchmark harness");
    println!();
    println!("USAGE:");
    println!("    nothing-bench list        list known reference programs");
    println!("    nothing-bench count NAME  print the action count for NAME");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(String::as_str);

    match command {
        Some("list") => {
            let programs = reference_programs();
            if programs.is_empty() {
                println!("(no reference programs recorded yet)");
            } else {
                for program in &programs {
                    println!("{} ({} actions)", program.name, program.actions.len());
                }
            }
        }
        Some("count") => {
            let Some(name) = args.get(2) else {
                eprintln!("error: `count` requires a reference program name");
                print_usage();
                std::process::exit(2);
            };
            match reference_programs().into_iter().find(|p| p.name == name) {
                Some(program) => println!("{}", program.actions.len()),
                None => {
                    eprintln!("error: no reference program named `{name}`");
                    std::process::exit(1);
                }
            }
        }
        _ => print_usage(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_programs_start_empty() {
        assert!(reference_programs().is_empty());
    }
}
