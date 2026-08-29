use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use nothing_action::log::ActionLog;
use nothing_agentapi::json::parse;
use nothing_core::doc::{Def, Doc};
use nothing_core::examples;
use nothing_core::exp::{Exp, Id};
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;
use nothing_merge::{DocVersion, merge_documents};
use nothing_store::{Document, encode_document};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_nothing")
}

fn scratch_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("nothing-cli-subcommand-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_fixture(name: &str, exp: Exp) -> std::path::PathBuf {
    let path = scratch_dir().join(name);
    let doc = Document::new(exp, examples::names(), ActionLog::new());
    std::fs::write(&path, encode_document(&doc)).unwrap();
    path
}

fn write_document(name: &str, defs: Vec<Def>, names: NameTable) -> std::path::PathBuf {
    let path = scratch_dir().join(name);
    let doc = Document::from_doc(
        Doc::new(defs).expect("distinct ids"),
        names,
        ActionLog::new(),
    );
    std::fs::write(&path, encode_document(&doc)).unwrap();
    path
}

fn factorial_definitions() -> (Vec<Def>, NameTable) {
    use nothing_eval::definitions::{FactorialIds, recursive_factorial};
    let ids = FactorialIds::fresh();
    let mut names = NameTable::new();
    names.set(ids.fact, "factorial");
    names.set(ids.n, "n");
    let mut fact = recursive_factorial(ids);
    fact.id = ids.fact;
    (vec![fact], names)
}

fn run(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(bin())
        .args(args)
        .output()
        .expect("the nothing binary runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn help_fits_one_screen(help: &str) {
    let lines: Vec<&str> = help.lines().collect();
    assert!(
        lines.len() <= 24,
        "help text is {} lines, expected <= 24:\n{help}",
        lines.len()
    );
    for line in &lines {
        assert!(
            line.chars().count() <= 100,
            "help line exceeds 100 columns: {line}"
        );
    }
}

#[test]
fn version_prints_the_workspace_version() {
    let (code, stdout, _) = run(&["--version"]);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "nothing 0.1.0");
}

#[test]
fn top_level_help_fits_one_screen() {
    let (code, stdout, _) = run(&["--help"]);
    assert_eq!(code, 0);
    help_fits_one_screen(&stdout);
}

#[test]
fn every_subcommand_help_fits_one_screen() {
    for command in ["edit", "run", "check", "repl", "protocol", "merge"] {
        let (code, stdout, _) = run(&[command, "--help"]);
        assert_eq!(code, 0, "`{command} --help` did not exit 0");
        help_fits_one_screen(&stdout);
    }
}

#[test]
fn check_accepts_a_well_typed_file_written_via_store() {
    let path = write_fixture("check-well-typed.nothing", examples::add_with_empty_hole());
    let (code, stdout, _) = run(&["check", path.to_str().unwrap()]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("well-typed: true"));
    assert!(stdout.contains("empty holes: 1"));
}

#[test]
fn check_rejects_an_ill_typed_tree() {
    let ill_typed = Exp::ap(Exp::num(1), Exp::num(2));
    let path = write_fixture("check-ill-typed.nothing", ill_typed);
    let (code, stdout, _) = run(&["check", path.to_str().unwrap()]);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("well-typed: false"));
}

#[test]
fn run_prints_a_value_for_a_program_that_finishes() {
    let path = write_fixture("run-value.nothing", examples::let_identity());
    let (code, stdout, _) = run(&["run", path.to_str().unwrap()]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert_eq!(stdout.trim(), "1");
}

#[test]
fn run_reports_an_indeterminate_result_and_its_hole() {
    let path = write_fixture("run-indeterminate.nothing", examples::add_with_empty_hole());
    let (code, stdout, _) = run(&["run", path.to_str().unwrap()]);
    assert_eq!(code, 2, "stdout: {stdout}");
    assert!(stdout.contains("indeterminate"));
    assert!(stdout.contains("blocked on hole"));
}

#[test]
fn run_evaluates_the_definition_named_main_across_definitions() {
    let (mut defs, mut names) = factorial_definitions();
    let fact = defs[0].id;
    let main = Id::fresh();
    names.set(main, "main");
    defs.push(Def::new(
        main,
        Ty::Num,
        Exp::ap(Exp::var(fact), Exp::num(5)),
    ));
    let path = write_document("run-main-across-definitions.nothing", defs, names);

    let (code, stdout, _) = run(&["run", path.to_str().unwrap()]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert_eq!(
        stdout.trim(),
        "120",
        "a self-referencing definition, no combinator"
    );
}

#[test]
fn run_lists_the_definitions_when_there_is_no_main() {
    let (defs, names) = factorial_definitions();
    let path = write_document("run-without-main.nothing", defs, names);

    let (code, stdout, stderr) = run(&["run", path.to_str().unwrap()]);
    assert_eq!(code, 1, "stdout: {stdout} stderr: {stderr}");
    let said = format!("{stdout}{stderr}");
    assert!(said.contains("no definition named `main`"), "{said}");
    assert!(said.contains("factorial"), "{said}");
    assert!(said.contains("Num -> Num"), "{said}");
}

#[test]
fn merge_writes_a_clean_result_and_matches_the_library() {
    let base_exp = Exp::pair(Exp::num(1), Exp::num(2));
    let ours_exp = Exp::pair(Exp::num(9), Exp::num(2));
    let theirs_exp = Exp::pair(Exp::num(1), Exp::num(8));

    let mut names = examples::names();
    names.set(nothing_core::doc::MAIN_ID, nothing_core::doc::MAIN_NAME);
    let expected = merge_documents(
        &DocVersion::single(base_exp.clone(), names.clone()),
        &DocVersion::single(ours_exp.clone(), names.clone()),
        &DocVersion::single(theirs_exp.clone(), names.clone()),
    );
    assert!(expected.is_clean(), "fixture is expected to merge cleanly");

    let base = write_fixture("merge-base-clean.nothing", base_exp);
    let ours = write_fixture("merge-ours-clean.nothing", ours_exp);
    let theirs = write_fixture("merge-theirs-clean.nothing", theirs_exp);

    let (code, stdout, _) = run(&[
        "merge",
        base.to_str().unwrap(),
        ours.to_str().unwrap(),
        theirs.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert_eq!(stdout.trim(), expected.merged.render());
}

#[test]
fn merge_reports_conflicts_and_exits_nonzero() {
    let base_exp = Exp::num(1);
    let ours_exp = Exp::num(2);
    let theirs_exp = Exp::num(3);

    let base = write_fixture("merge-base-conflict.nothing", base_exp);
    let ours = write_fixture("merge-ours-conflict.nothing", ours_exp);
    let theirs = write_fixture("merge-theirs-conflict.nothing", theirs_exp);

    let (code, stdout, _) = run(&[
        "merge",
        base.to_str().unwrap(),
        ours.to_str().unwrap(),
        theirs.to_str().unwrap(),
    ]);
    assert_eq!(code, 1, "stdout: {stdout}");
    assert!(stdout.contains("conflict"));
}

#[test]
fn merge_o_flag_writes_a_document_the_cli_can_read_back() {
    let base_exp = Exp::pair(Exp::num(1), Exp::num(2));
    let ours_exp = Exp::pair(Exp::num(9), Exp::num(2));
    let theirs_exp = Exp::pair(Exp::num(1), Exp::num(8));

    let base = write_fixture("merge-base-o.nothing", base_exp);
    let ours = write_fixture("merge-ours-o.nothing", ours_exp);
    let theirs = write_fixture("merge-theirs-o.nothing", theirs_exp);
    let out = scratch_dir().join("merge-out.nothing");
    std::fs::remove_file(&out).ok();

    let (code, stdout, _) = run(&[
        "merge",
        base.to_str().unwrap(),
        ours.to_str().unwrap(),
        theirs.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(out.exists());

    let (code, stdout, _) = run(&["check", out.to_str().unwrap()]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("well-typed: true"));
}

struct Lines {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Lines {
    fn start(args: &[&str]) -> Lines {
        let mut child = Command::new(bin())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("the nothing binary starts");
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
        Lines {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, line: &str) -> String {
        writeln!(self.stdin, "{line}").expect("the line is written");
        self.stdin.flush().expect("the line is flushed");
        let mut reply = String::new();
        self.stdout.read_line(&mut reply).expect("a reply arrives");
        reply.trim_end().to_string()
    }

    fn finish(mut self) {
        drop(self.stdin);
        let status = self.child.wait().expect("the process exits");
        assert!(status.success(), "exited with {status}");
    }
}

#[test]
fn repl_applies_a_step_and_renders_the_result() {
    let mut repl = Lines::start(&["repl"]);
    let reply = repl.send("construct-num 5");
    assert!(reply.contains('5'), "reply: {reply}");
    repl.send("quit");
    repl.finish();
}

#[test]
fn protocol_speaks_json_over_stdio() {
    let mut proto = Lines::start(&["protocol"]);
    let reply = proto.send(r#"{"method":"apply","params":{"step":"construct-num 5"}}"#);
    let json = parse(&reply).unwrap_or_else(|e| panic!("bad reply `{reply}`: {e}"));
    assert_eq!(json.get("ok").and_then(|v| v.as_bool()), Some(true));
    proto.send(r#"{"method":"quit"}"#);
    proto.finish();
}
