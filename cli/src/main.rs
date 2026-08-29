mod check;
mod doc_cmd;
mod edit;
mod fileio;
mod git_cmd;
mod holes;
mod mcp;
mod merge_cmd;
mod protocol_cmd;
mod repl_cmd;
mod run_cmd;
mod tutorial;

use std::path::PathBuf;
use std::process::ExitCode;

const TOP_HELP: &str = "\
nothing — a projectional structural editor and language

Usage: nothing <command> [args]

Commands:
  tutorial [<file>]      a guided first session in the editor (default tutorial.n)
  edit <file>            open <file> in the TUI editor
  run <file>             evaluate <file>, or perform it if it is a command
  check <file>           check <file> is well-typed
  doc [<file>]           render a reference for <file>, or for the stdlib
  repl                   the action-name REPL
  protocol               speak the JSON agent protocol over stdio
  mcp                    serve the editor to MCP agent hosts over stdio
  merge <base> <a> <b>   three-way structural merge
  merge-driver <files>   git merge driver — see GIT.md
  textconv <file>        structural rendering, for git diff
  diff-driver <args>     git external diff of typed operations

  --version, -V          print the version and exit
  --help, -h             print this help and exit

Run `nothing <command> --help` for command-specific help.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let status = nothing_core::stack::on_deep_stack(|| dispatch(&args));
    ExitCode::from(status as u8)
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
        "tutorial" => tutorial_command(rest),
        "edit" => run_file_command(rest, edit::HELP, edit::run),
        "run" => run_command(rest),
        "check" => run_file_command(rest, check::HELP, check::run),
        "doc" => doc_command(rest),
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
        "mcp" => {
            if wants_help(rest) {
                println!("{}", mcp::HELP);
                return 0;
            }
            mcp::run(rest) as u8
        }
        "merge" => merge_command(rest),
        "merge-driver" => merge_driver_command(rest),
        "textconv" => run_file_command(rest, git_cmd::TEXTCONV_HELP, git_cmd::run_textconv),
        "diff-driver" => diff_driver_command(rest),
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

fn tutorial_command(args: &[String]) -> u8 {
    if wants_help(args) {
        println!("{}", tutorial::HELP);
        return 0;
    }
    match args {
        [] => tutorial::run(&PathBuf::from(tutorial::DEFAULT_FILE)) as u8,
        [path] => tutorial::run(&PathBuf::from(path)) as u8,
        _ => {
            eprintln!("error: `tutorial` takes one file, and was given more than one");
            eprintln!("{}", tutorial::HELP);
            1
        }
    }
}

fn run_command(args: &[String]) -> u8 {
    if wants_help(args) {
        println!("{}", run_cmd::HELP);
        return 0;
    }

    let mut fuel = nothing_eval::DEFAULT_FUEL;
    let mut path: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--fuel" {
            let Some(text) = args.get(i + 1) else {
                eprintln!("error: `--fuel` needs a number of steps");
                eprintln!("{}", run_cmd::HELP);
                return 1;
            };
            match text.parse::<usize>() {
                Ok(0) => {
                    eprintln!("error: `--fuel 0` would not let the program take a single step");
                    return 1;
                }
                Ok(n) => fuel = n,
                Err(_) => {
                    eprintln!("error: `--fuel {text}` is not a number of steps");
                    return 1;
                }
            }
            i += 2;
        } else if path.is_none() {
            path = Some(PathBuf::from(&args[i]));
            i += 1;
        } else {
            eprintln!("error: `run` takes one file, and was given more than one");
            eprintln!("{}", run_cmd::HELP);
            return 1;
        }
    }

    match path {
        Some(path) => run_cmd::run_with_fuel(&path, fuel) as u8,
        None => {
            eprintln!("error: missing <file>");
            eprintln!("{}", run_cmd::HELP);
            1
        }
    }
}

fn doc_command(args: &[String]) -> u8 {
    if wants_help(args) {
        println!("{}", doc_cmd::HELP);
        return 0;
    }

    let mut path: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "-o" {
            let Some(target) = args.get(i + 1) else {
                eprintln!("error: `-o` needs a path");
                eprintln!("{}", doc_cmd::HELP);
                return 1;
            };
            out = Some(PathBuf::from(target));
            i += 2;
        } else if path.is_none() {
            path = Some(PathBuf::from(&args[i]));
            i += 1;
        } else {
            eprintln!("error: `doc` renders one document, and was given more than one");
            eprintln!("{}", doc_cmd::HELP);
            return 1;
        }
    }

    doc_cmd::run(path.as_deref(), out.as_deref()) as u8
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

fn merge_driver_command(args: &[String]) -> u8 {
    if wants_help(args) {
        println!("{}", git_cmd::MERGE_DRIVER_HELP);
        return 0;
    }

    if args.len() < 3 {
        eprintln!("error: expected <base> <ours> <theirs>");
        eprintln!("{}", git_cmd::MERGE_DRIVER_HELP);
        return 1;
    }

    git_cmd::run_merge_driver(
        std::path::Path::new(&args[0]),
        std::path::Path::new(&args[1]),
        std::path::Path::new(&args[2]),
        args.get(4).map(String::as_str),
    ) as u8
}

fn diff_driver_command(args: &[String]) -> u8 {
    if wants_help(args) {
        println!("{}", git_cmd::DIFF_DRIVER_HELP);
        return 0;
    }
    git_cmd::run_diff_driver(args) as u8
}
