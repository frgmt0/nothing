use std::path::Path;

use nothing_eval::{Outcome, eval, render};

use crate::fileio::read_document;

pub const HELP: &str = "\
nothing run <file>

Evaluate the program in <file> and print the outcome:
  - a value, if evaluation finished
  - an indeterminate result and the holes it is blocked on
  - a partial result, if evaluation ran out of fuel

Exit status: 0 on a value, 2 on an indeterminate result, 3 on out-of-fuel,
1 on a file error.

Options:
  -h, --help   print this help and exit";

pub fn run(path: &Path) -> i32 {
    let doc = match read_document(path) {
        Ok(doc) => doc,
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    };

    match eval(&doc.exp) {
        Outcome::Value(value) => {
            println!("{}", render(&value, &doc.names));
            0
        }
        Outcome::Indeterminate { result, blocked } => {
            println!("indeterminate: {}", render(&result, &doc.names));
            for hole in &blocked {
                println!("  blocked on hole {:?} ({:?})", hole.hole, hole.kind);
                for (id, value) in hole.known() {
                    println!(
                        "    {} = {}",
                        doc.names.display(id),
                        render(&value, &doc.names)
                    );
                }
            }
            2
        }
        Outcome::OutOfFuel { partial, steps } => {
            println!(
                "out of fuel after {steps} steps: {}",
                render(&partial, &doc.names)
            );
            3
        }
    }
}
