use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use nothing_action::act::EditState;
use nothing_action::log::{ActionLog, AuthorId};
use nothing_action::script;
use nothing_core::exp::Exp;
use nothing_core::ty::Ty;
use nothing_store::{Document, decode_document, encode_document};

const EXAMPLES: [&str; 5] = [
    "unit_converter",
    "grade_calculator",
    "state_machine",
    "text_game_turn",
    "decision_table",
];

const AUTHOR: AuthorId = AuthorId::new(1);

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_nothing")
}

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
}

fn script_path(name: &str) -> PathBuf {
    examples_dir().join(format!("{name}.actions"))
}

fn document_path(name: &str) -> PathBuf {
    examples_dir().join(format!("{name}.n"))
}

fn read_script(name: &str) -> String {
    std::fs::read_to_string(script_path(name))
        .unwrap_or_else(|e| panic!("examples/{name}.actions is readable: {e}"))
}

fn read_bytes(name: &str) -> Vec<u8> {
    std::fs::read(document_path(name))
        .unwrap_or_else(|e| panic!("examples/{name}.n is readable: {e}"))
}

fn replay(name: &str) -> (EditState, ActionLog) {
    let text = read_script(name);
    let mut state = EditState::empty().under(nothing_stdlib::prelude());
    let mut log = ActionLog::new();
    for (line, step) in script::parse_numbered_script(&text)
        .unwrap_or_else(|e| panic!("examples/{name}.actions parses: {e}"))
    {
        let action = step
            .resolve(&state)
            .unwrap_or_else(|e| panic!("examples/{name}.actions line {line}: {e}"));
        assert!(
            state.apply_mut(action.clone()),
            "examples/{name}.actions line {line}: `{}` did not apply to\n{}",
            script::step_name(&step),
            state.render_document()
        );
        log.append(action, 0, AUTHOR);
    }
    (state, log)
}

fn build(name: &str) -> Document {
    let (state, log) = replay(name);
    Document::documented(state.doc(), state.names.own(), state.docs.own(), log)
}

fn committed(name: &str) -> Document {
    decode_document(&read_bytes(name))
        .unwrap_or_else(|e| panic!("examples/{name}.n decodes: {e:?}"))
}

fn holes(exp: &Exp) -> usize {
    let here = usize::from(matches!(exp, Exp::EmptyHole(_) | Exp::NonEmptyHole(..)));
    here + match exp {
        Exp::Lam(_, _, b)
        | Exp::Proj(_, b)
        | Exp::Field(b, _)
        | Exp::Inj(_, b)
        | Exp::Print(b)
        | Exp::CmdPure(b)
        | Exp::NonEmptyHole(_, b) => holes(b),
        Exp::Ap(a, b)
        | Exp::BinOp(_, a, b)
        | Exp::Let(_, a, b)
        | Exp::Pair(a, b)
        | Exp::CmdBind(a, _, b)
        | Exp::Cons(a, b) => holes(a) + holes(b),
        Exp::If(a, b, c) | Exp::Fold(a, b, c) => holes(a) + holes(b) + holes(c),
        Exp::Record(fields) => fields.iter().map(|(_, e)| holes(e)).sum(),
        Exp::Match(s, arms) => holes(s) + arms.iter().map(|(_, _, b)| holes(b)).sum::<usize>(),
        _ => 0,
    }
}

fn run_cli(args: &[&str], stdin: &str) -> (i32, String, String) {
    let mut child = Command::new(bin())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the nothing binary starts");
    child
        .stdin
        .as_mut()
        .expect("stdin is piped")
        .write_all(stdin.as_bytes())
        .expect("the input is written");
    let output = child.wait_with_output().expect("the process exits");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn expected_runs() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("unit_converter", "", UNIT_CONVERTER_OUTPUT),
        ("grade_calculator", "", GRADE_CALCULATOR_OUTPUT),
        ("state_machine", "coin\nkick\nfix\n", STATE_MACHINE_OUTPUT),
        ("text_game_turn", "look\nnorth\n", TEXT_GAME_TURN_OUTPUT),
        ("decision_table", "", DECISION_TABLE_OUTPUT),
    ]
}

const UNIT_CONVERTER_OUTPUT: &str =
    "320 :: 680 :: 986 :: 2120 :: 42000 :: 4186 :: 10800 :: 5000 :: nil\n";

const GRADE_CALCULATOR_OUTPUT: &str = "\
class grade: B
grades: A, C, B, D, A
everyone passed
";

const STATE_MACHINE_OUTPUT: &str = "\
the turnstile is unlocked
the turnstile is jammed
the turnstile is locked
";

const TEXT_GAME_TURN_OUTPUT: &str = "\
The observatory. Try look, exits, north or inventory.
A cold glass dome. The telescope is pointed at nothing in particular.
Shelves of star charts, most of them wrong.
";

const DECISION_TABLE_OUTPUT: &str = "\
young saver
standard
referred
senior
declined
";

#[test]
fn replaying_each_action_script_reproduces_the_committed_document_byte_for_byte() {
    for name in EXAMPLES {
        let rebuilt = encode_document(&build(name));
        assert_eq!(
            rebuilt,
            read_bytes(name),
            "examples/{name}.n is not what replaying examples/{name}.actions writes"
        );
    }
}

#[test]
fn every_example_decodes_and_is_small_and_well_typed_and_free_of_holes() {
    let prelude = nothing_stdlib::prelude();
    for name in EXAMPLES {
        let doc = committed(name);
        assert!(
            doc.doc.len() < 30,
            "examples/{name}.n holds {} definitions; an example must stay under 30",
            doc.doc.len()
        );
        assert!(
            doc.doc.is_well_typed_in(prelude.ctx()),
            "examples/{name}.n is not well typed against the standard library"
        );
        for def in doc.doc.defs() {
            assert_eq!(
                holes(&def.body),
                0,
                "examples/{name}.n still has a hole in `{}`",
                doc.names.display(def.id)
            );
        }
    }
}

#[test]
fn every_definition_in_every_example_is_named_annotated_and_documented() {
    for name in EXAMPLES {
        let doc = committed(name);
        for def in doc.doc.defs() {
            let shown = doc
                .names
                .get(def.id)
                .unwrap_or_else(|| panic!("examples/{name}.n leaves {} unnamed", def.id));
            assert!(
                doc.docs.get(def.id).is_some_and(|line| !line.is_empty()),
                "examples/{name}.n leaves `{shown}` without a doc line"
            );
            assert_ne!(
                def.ann,
                Ty::Hole,
                "examples/{name}.n leaves `{shown}` without a type annotation"
            );
        }
    }
}

#[test]
fn check_accepts_every_example() {
    for name in EXAMPLES {
        let path = document_path(name);
        let (code, stdout, stderr) = run_cli(&["check", path.to_str().unwrap()], "");
        assert_eq!(
            code, 0,
            "`nothing check examples/{name}.n` failed\nstdout: {stdout}\nstderr: {stderr}"
        );
    }
}

#[test]
fn running_every_example_prints_exactly_what_the_documentation_promises() {
    for (name, stdin, expected) in expected_runs() {
        let path = document_path(name);
        let (code, stdout, stderr) = run_cli(&["run", path.to_str().unwrap()], stdin);
        assert_eq!(
            code, 0,
            "`nothing run examples/{name}.n` failed\nstdout: {stdout}\nstderr: {stderr}"
        );
        assert_eq!(
            stdout, expected,
            "`nothing run examples/{name}.n` printed something else"
        );
    }
}

#[test]
fn every_example_opens_in_the_editor() {
    for name in EXAMPLES {
        let doc = committed(name);
        let state = EditState::with_doc(&doc.doc, doc.names.clone(), 0)
            .unwrap_or_else(|| panic!("examples/{name}.n has a first definition to open on"))
            .with_docs(doc.docs.clone())
            .under(nothing_stdlib::prelude());
        assert_eq!(state.def_count(), doc.doc.len());
        assert_eq!(state.def_index(), 0);
        assert!(
            state.is_well_typed(),
            "examples/{name}.n does not type check once it is open in the editor"
        );
        assert!(
            !state.prelude_ids().is_empty(),
            "examples/{name}.n opens without the standard library in scope"
        );
    }
}

#[test]
#[ignore]
fn regenerate() {
    for name in EXAMPLES {
        let (state, log) = replay(name);
        let doc = Document::documented(state.doc(), state.names.own(), state.docs.own(), log);
        std::fs::write(document_path(name), encode_document(&doc))
            .unwrap_or_else(|e| panic!("examples/{name}.n is writable: {e}"));
        println!("--- {name} ---");
        println!("{}", state.render_document());
        println!("well typed: {}", state.is_well_typed());
    }
}
