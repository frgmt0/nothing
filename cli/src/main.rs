mod check;
mod edit;
mod fileio;
mod holes;
mod merge_cmd;
mod protocol_cmd;
mod repl_cmd;
mod run_cmd;

use std::path::PathBuf;
use std::process::ExitCode;

const TOP_HELP: &str = "\
nothing — a projectional structural editor and language

Usage: nothing <command> [args]

Commands:
  edit <file>            open <file> in the TUI editor
  run <file>             evaluate <file> and print the outcome
  check <file>           check <file> is well-typed
  repl                   the action-name REPL
  protocol               speak the JSON agent protocol over stdio
  merge <base> <a> <b>   three-way structural merge

  --version, -V          print the version and exit
  --help, -h             print this help and exit

Run `nothing <command> --help` for command-specific help.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(dispatch(&args) as u8)
}

fn wants_help(args: &[String]) -> bool {
    args.iter().any(|a| a == "-h" || a == "--help")
}

fn dispatch(args: &[String]) -> u8 {
    let Some(command) = args.first() else {
        println!("{TOP_HELP}");
        return 0;
    };
    let rest = &args[1..];

    match command.as_str() {
        "--version" | "-V" => {
            println!("nothing {}", env!("CARGO_PKG_VERSION"));
            0
        }
        "--help" | "-h" => {
            println!("{TOP_HELP}");
            0
        }
        "edit" => run_file_command(rest, edit::HELP, edit::run),
        "run" => run_file_command(rest, run_cmd::HELP, run_cmd::run),
        "check" => run_file_command(rest, check::HELP, check::run),
        "repl" => {
            if wants_help(rest) {
                println!("{}", repl_cmd::HELP);
                return 0;
            }
            repl_cmd::run() as u8
        }
        "protocol" => {
            if wants_help(rest) {
                println!("{}", protocol_cmd::HELP);
                return 0;
            }
            protocol_cmd::run(rest) as u8
        }
        "merge" => merge_command(rest),
        other => {
            eprintln!("error: unknown command `{other}`");
            eprintln!("{TOP_HELP}");
            1
        }
    }
}

fn run_file_command(args: &[String], help: &str, f: fn(&std::path::Path) -> i32) -> u8 {
    if wants_help(args) {
        println!("{help}");
        return 0;
    }
    match args.first() {
        Some(path) => f(&PathBuf::from(path)) as u8,
        None => {
            eprintln!("error: missing <file>");
            eprintln!("{help}");
            1
        }
    }
}

fn merge_command(args: &[String]) -> u8 {
    if wants_help(args) {
        println!("{}", merge_cmd::HELP);
        return 0;
    }

    let mut positional = Vec::new();
    let mut out: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "-o" {
            match args.get(i + 1) {
                Some(path) => {
                    out = Some(PathBuf::from(path));
                    i += 2;
                }
                None => {
                    eprintln!("error: `-o` needs a path");
                    eprintln!("{}", merge_cmd::HELP);
                    return 1;
                }
            }
        } else {
            positional.push(args[i].clone());
            i += 1;
        }
    }

    if positional.len() != 3 {
        eprintln!("error: expected <base> <a> <b>");
        eprintln!("{}", merge_cmd::HELP);
        return 1;
    }

    merge_cmd::run(
        std::path::Path::new(&positional[0]),
        std::path::Path::new(&positional[1]),
        std::path::Path::new(&positional[2]),
        out.as_deref(),
    ) as u8
}
