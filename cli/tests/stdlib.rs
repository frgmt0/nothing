use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use nothing_action::log::ActionLog;
use nothing_agentapi::json::parse;
use nothing_core::doc::MAIN_ID;
use nothing_core::exp::Exp;
use nothing_core::names::NameTable;
use nothing_store::{Document, encode_document};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_nothing")
}

fn scratch_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("nothing-cli-stdlib-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn reference_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("stdlib")
        .join("REFERENCE.md")
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

fn stdlib_var(name: &str) -> Exp {
    Exp::Var(nothing_stdlib::id_of(name).unwrap_or_else(|| panic!("the stdlib defines `{name}`")))
}

fn write_main(file: &str, body: Exp) -> PathBuf {
    let path = scratch_dir().join(file);
    let mut names = NameTable::new();
    names.set(MAIN_ID, "main");
    let document = Document::new(body, names, ActionLog::new());
    std::fs::write(&path, encode_document(&document)).unwrap();
    path
}

fn ap2(f: Exp, a: Exp, b: Exp) -> Exp {
    Exp::Ap(Box::new(Exp::Ap(Box::new(f), Box::new(a))), Box::new(b))
}

#[test]
fn a_program_that_calls_a_stdlib_function_evaluates_to_its_answer() {
    let path = write_main(
        "calls-min.nothing",
        ap2(stdlib_var("min"), Exp::Num(3), Exp::Num(5)),
    );
    let (code, stdout, stderr) = run(&["run", path.to_str().unwrap()]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "3");
}

#[test]
fn a_program_that_folds_a_list_through_the_stdlib_evaluates() {
    let list = Exp::Cons(
        Box::new(Exp::Num(4)),
        Box::new(Exp::Cons(
            Box::new(Exp::Num(9)),
            Box::new(Exp::Cons(Box::new(Exp::Num(2)), Box::new(Exp::Nil))),
        )),
    );
    let path = write_main(
        "sums-a-list.nothing",
        Exp::Ap(Box::new(stdlib_var("sum")), Box::new(list)),
    );
    let (code, stdout, stderr) = run(&["run", path.to_str().unwrap()]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "15");
}

#[test]
fn a_command_from_the_stdlib_is_performed_and_writes_its_line() {
    let path = write_main(
        "prints-a-label.nothing",
        ap2(
            stdlib_var("print_labelled"),
            Exp::Str("total".to_string()),
            Exp::Str("7".to_string()),
        ),
    );
    let (code, stdout, stderr) = run(&["run", path.to_str().unwrap()]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert_eq!(stdout.trim(), "total: 7");
}

#[test]
fn check_reports_a_stdlib_call_well_typed_and_says_how_much_is_in_scope() {
    let path = write_main(
        "check-stdlib.nothing",
        ap2(stdlib_var("max"), Exp::Num(1), Exp::Num(2)),
    );
    let (code, stdout, stderr) = run(&["check", path.to_str().unwrap()]);
    assert_eq!(code, 0, "stdout: {stdout}\nstderr: {stderr}");
    assert!(stdout.contains("well-typed: true"), "stdout: {stdout}");
    assert!(
        stdout.contains(&format!(
            "stdlib definitions in scope: {}",
            nothing_stdlib::doc().len()
        )),
        "stdout: {stdout}"
    );
}

#[test]
fn a_saved_document_does_not_carry_the_stdlib_it_borrowed() {
    let path = write_main(
        "borrows-min.nothing",
        ap2(stdlib_var("min"), Exp::Num(3), Exp::Num(5)),
    );
    let bytes = std::fs::read(&path).unwrap();
    let document = nothing_store::decode_document(&bytes).expect("it decodes");
    assert_eq!(document.doc.len(), 1);
    assert!(document.docs.is_empty());
    assert!(bytes.len() < nothing_stdlib::STDLIB_BYTES.len() / 10);
}

#[test]
fn doc_renders_the_stdlib_reference_that_is_committed() {
    let (code, stdout, stderr) = run(&["doc"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let committed = std::fs::read_to_string(reference_path())
        .expect("stdlib/REFERENCE.md is committed alongside the stdlib");
    assert_eq!(
        stdout, committed,
        "stdlib/REFERENCE.md is out of date; regenerate it with `nothing doc -o stdlib/REFERENCE.md`"
    );
}

#[test]
fn the_reference_carries_a_section_a_type_and_a_doc_line_for_every_definition() {
    let reference = std::fs::read_to_string(reference_path()).expect("the reference is committed");
    for name in nothing_stdlib::names_in_order() {
        assert!(
            reference.contains(&format!("### `{name}`\n")),
            "the reference has no section for `{name}`"
        );
    }
    let id = nothing_stdlib::id_of("clamp").expect("clamp is in the stdlib");
    let line = nothing_stdlib::docs().get(id).expect("clamp is documented");
    assert!(reference.contains(line), "the reference drops clamp's doc");
    assert!(reference.contains("Num -> Num -> Num -> Num"));
    assert!(reference.contains("In words: "));
}

#[test]
fn doc_names_the_stdlib_functions_a_document_calls_rather_than_their_ids() {
    let path = write_main(
        "doc-calls-stdlib.nothing",
        ap2(stdlib_var("min"), Exp::Num(3), Exp::Num(5)),
    );
    let (code, stdout, stderr) = run(&["doc", path.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stdout.contains("min 3 5"),
        "a borrowed name must render as a name: {stdout}"
    );
    assert!(
        !stdout.contains('_'),
        "no definition should render as a raw id: {stdout}"
    );
}

#[test]
fn doc_renders_a_document_of_ones_own() {
    let path = write_main("doc-of-my-own.nothing", Exp::Num(12));
    let (code, stdout, stderr) = run(&["doc", path.to_str().unwrap()]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("### `main`"), "stdout: {stdout}");
    assert!(stdout.contains("1 definitions"), "stdout: {stdout}");
    assert!(!stdout.contains("### `min`"), "stdout: {stdout}");
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
fn the_protocol_offers_the_stdlib_by_name_and_carries_its_doc_lines() {
    let mut proto = Lines::start(&["protocol"]);
    let reply = proto.send(r#"{"method":"apply","params":{"step":"construct-var min"}}"#);
    let json = parse(&reply).unwrap_or_else(|e| panic!("bad reply `{reply}`: {e}"));
    assert_eq!(json.get("ok").and_then(|v| v.as_bool()), Some(true));
    assert!(
        reply.contains("the smaller of two numbers"),
        "the doc line for min did not reach the protocol: {reply}"
    );
    proto.send(r#"{"method":"quit"}"#);
    proto.finish();
}

#[test]
fn the_protocol_can_be_asked_for_an_empty_prelude_as_the_stdlib_itself_was_built() {
    let mut proto = Lines::start(&["protocol", "--no-stdlib"]);
    let reply = proto.send(r#"{"method":"apply","params":{"step":"construct-var min"}}"#);
    let json = parse(&reply).unwrap_or_else(|e| panic!("bad reply `{reply}`: {e}"));
    assert_eq!(json.get("ok").and_then(|v| v.as_bool()), Some(false));
    proto.send(r#"{"method":"quit"}"#);
    proto.finish();
}
