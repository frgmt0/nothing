use std::io::{BufRead, Write};
use std::path::Path;

use nothing_core::doc::MAIN_NAME;
use nothing_eval::{
    Io, Outcome, eval_doc_with_fuel, main_type, perform_doc, render, runs_as_a_command,
};

use crate::fileio::read_document;

pub const HELP: &str = "\
nothing run [--fuel N] <file>

Evaluate the definition named `main` in <file> and print the outcome:
  - a value, if evaluation finished
  - an indeterminate result and the holes it is blocked on
  - a partial result, if evaluation ran out of fuel

If `main` has a command type, it is performed instead of printed: `print`
writes a line to standard output, `readline` reads one from standard input,
`bind` sequences and `pure` yields. What a command run writes is what it
printed; the value it finally yields is not printed. A command that reaches
a hole performs everything up to the hole and then reports it.

The standard library is in scope, so a program may call `min`, `map`, `join`
and the rest without defining them; `nothing doc` lists what there is.

Exit status: 0 on a value, 2 on an indeterminate result, 3 on out-of-fuel,
1 on a file error.

Options:
  --fuel N     the execution budget, in steps (default 200000); every
               evaluation step and every command performed spends one
  -h, --help   print this help and exit";

struct StdIo;

impl Io for StdIo {
    fn write_line(&mut self, text: &str) {
        let mut out = std::io::stdout();
        let _ = writeln!(out, "{text}");
        let _ = out.flush();
    }

    fn read_line(&mut self) -> Option<String> {
        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim_end_matches(['\n', '\r']).to_string()),
        }
    }
}

pub fn run_with_fuel(path: &Path, fuel: usize) -> i32 {
    let doc = match read_document(path) {
        Ok(doc) => doc,
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    };

    let Some(main) = doc.main_id() else {
        eprintln!("error: this document has no definition named `{MAIN_NAME}`");
        eprintln!("it defines:");
        for def in doc.doc.defs() {
            eprintln!("  {} : {}", doc.names.display(def.id), def.ann);
        }
        eprintln!("rename one of them to `{MAIN_NAME}` to say where to start");
        return 1;
    };

    let program = nothing_stdlib::prelude().extend(&doc.doc);
    let performing = runs_as_a_command(&program, main);
    let outcome = if performing {
        perform_doc(&program, main, fuel, &mut StdIo).outcome
    } else {
        eval_doc_with_fuel(&program, main, fuel)
    };
    let names = nothing_stdlib::prelude().names_for(&doc.names);

    match outcome {
        Outcome::Value(value) => {
            if !performing {
                println!("{}", render(&value, &names));
            }
            0
        }
        Outcome::Indeterminate { result, blocked } => {
            println!("indeterminate: {}", render(&result, &names));
            for hole in &blocked {
                println!("  blocked on hole {:?} ({:?})", hole.hole, hole.kind);
                for (id, value) in hole.known() {
                    println!("    {} = {}", names.display(id), render(&value, &names));
                }
            }
            if performing && blocked.is_empty() {
                println!(
                    "  this command cannot go any further: {} is stuck",
                    main_type(&program, main)
                );
            }
            2
        }
        Outcome::OutOfFuel { partial, steps } => {
            println!(
                "out of fuel after {steps} steps: {}",
                render(&partial, &names)
            );
            if performing {
                println!(
                    "  the run stopped at its budget of {fuel} steps; \
                     raise it with `--fuel N` if the program really is this long"
                );
            }
            3
        }
    }
}
