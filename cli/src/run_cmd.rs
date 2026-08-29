use std::io::{BufRead, Write};
use std::path::Path;

use nothing_core::doc::MAIN_NAME;
use nothing_core::prelude::Prelude;
use nothing_eval::{
    Io, Outcome, eval_doc_with_fuel, main_type, perform_doc, render, runs_as_a_command,
};
use nothing_store::Document;

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

pub struct RunReport {
    pub performed: bool,
    pub value: Option<String>,
    pub lines: Vec<String>,
    pub status: i32,
}

pub fn missing_main_message(doc: &Document) -> String {
    let mut message = format!("this document has no definition named `{MAIN_NAME}`\nit defines:\n");
    for def in doc.doc.defs() {
        message.push_str(&format!("  {} : {}\n", doc.names.display(def.id), def.ann));
    }
    message.push_str(&format!(
        "rename one of them to `{MAIN_NAME}` to say where to start"
    ));
    message
}

pub fn perform_or_evaluate(
    doc: &Document,
    prelude: &Prelude,
    fuel: usize,
    io: &mut (dyn Io + Send),
) -> Result<RunReport, String> {
    let Some(main) = doc.main_id() else {
        return Err(missing_main_message(doc));
    };

    let program = prelude.extend(&doc.doc);
    let performed = runs_as_a_command(&program, main);
    let outcome = if performed {
        perform_doc(&program, main, fuel, io).outcome
    } else {
        eval_doc_with_fuel(&program, main, fuel)
    };
    let names = prelude.names_for(&doc.names);

    let mut lines = Vec::new();
    let mut value = None;
    let status = match outcome {
        Outcome::Value(result) => {
            let rendered = render(&result, &names);
            if !performed {
                lines.push(rendered.clone());
            }
            value = Some(rendered);
            0
        }
        Outcome::Indeterminate { result, blocked } => {
            lines.push(format!("indeterminate: {}", render(&result, &names)));
            for hole in &blocked {
                lines.push(format!(
                    "  blocked on hole {:?} ({:?})",
                    hole.hole, hole.kind
                ));
                for (id, known) in hole.known() {
                    lines.push(format!(
                        "    {} = {}",
                        names.display(id),
                        render(&known, &names)
                    ));
                }
            }
            if performed && blocked.is_empty() {
                lines.push(format!(
                    "  this command cannot go any further: {} is stuck",
                    main_type(&program, main)
                ));
            }
            2
        }
        Outcome::OutOfFuel { partial, steps } => {
            lines.push(format!(
                "out of fuel after {steps} steps: {}",
                render(&partial, &names)
            ));
            if performed {
                lines.push(format!(
                    "  the run stopped at its budget of {fuel} steps; \
                     raise it with `--fuel N` if the program really is this long"
                ));
            }
            3
        }
    };

    Ok(RunReport {
        performed,
        value,
        lines,
        status,
    })
}

pub fn run_with_fuel(path: &Path, fuel: usize) -> i32 {
    let doc = match read_document(path) {
        Ok(doc) => doc,
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    };

    let prelude = nothing_stdlib::prelude();
    match perform_or_evaluate(&doc, &prelude, fuel, &mut StdIo) {
        Err(message) => {
            eprintln!("error: {message}");
            1
        }
        Ok(report) => {
            for line in &report.lines {
                println!("{line}");
            }
            report.status
        }
    }
}
