use std::path::PathBuf;

use nothing_merge::bench::{git_available, markdown, plain_table, run_all};
use nothing_merge::merge3::merge;
use nothing_merge::scenarios::all;
use nothing_merge::text::to_text;

const DATE: &str = "2026-08-29";

fn default_output() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.push("bench");
    path.push("MERGE.md");
    path
}

fn dump(index: usize) {
    let scenarios = all();
    let scenario = match scenarios.get(index) {
        Some(scenario) => scenario,
        None => {
            eprintln!("no scenario {index}; there are {}", scenarios.len());
            std::process::exit(2);
        }
    };
    println!("scenario {index}: {}", scenario.name);
    println!("class: {}", scenario.category.label());
    println!("note: {}\n", scenario.note);
    for (which, version, style) in [
        ("base", &scenario.base, scenario.base_style),
        ("ours", &scenario.ours, scenario.ours_style),
        ("theirs", &scenario.theirs, scenario.theirs_style),
    ] {
        println!("--- {which} ---");
        print!("{}", to_text(version, style));
        println!();
    }
    let outcome = merge(&scenario.base, &scenario.ours, &scenario.theirs);
    println!("--- structural operations ---");
    println!("ours:   {:#?}", outcome.ours_ops);
    println!("theirs: {:#?}", outcome.theirs_ops);
    println!("--- structural merge ---");
    println!("{}", outcome.report());
    print!("{}", to_text(&outcome.merged, scenario.base_style));
}

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.first().map(String::as_str) == Some("--dump") {
        let index = raw
            .get(1)
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(0);
        dump(index);
        return;
    }

    if !git_available() {
        eprintln!("git is not on PATH; the line-based half of this benchmark cannot run");
        std::process::exit(1);
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out: Option<PathBuf> = Some(default_output());
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out = args.get(i).map(PathBuf::from);
            }
            "--print" => out = None,
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let rows = run_all();
    print!("{}", plain_table(&rows));

    if let Some(path) = out {
        let body = markdown(&rows, DATE);
        std::fs::write(&path, body).expect("write the benchmark table");
        println!("\nwrote {}", path.display());
    }
}
