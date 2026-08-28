use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use nothing_agentapi::json::{Json, parse};

struct Driver {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Driver {
    fn start() -> Driver {
        let mut child = Command::new(env!("CARGO_BIN_EXE_protocol"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("the protocol binary starts");
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
        Driver {
            child,
            stdin,
            stdout,
        }
    }

    fn request(&mut self, line: &str) -> Json {
        writeln!(self.stdin, "{line}").expect("the request is written");
        self.stdin.flush().expect("the request is flushed");
        let mut reply = String::new();
        let read = self.stdout.read_line(&mut reply).expect("a reply arrives");
        assert!(read > 0, "the protocol binary closed stdout on `{line}`");
        parse(reply.trim()).unwrap_or_else(|e| panic!("bad reply `{reply}`: {e}"))
    }

    fn step(&mut self, step: &str) -> Json {
        let request = Json::obj(vec![
            ("method", Json::str("apply")),
            ("params", Json::obj(vec![("step", Json::str(step))])),
        ]);
        self.request(&request.to_string())
    }

    fn finish(mut self) {
        self.request(r#"{"method":"quit"}"#);
        drop(self.stdin);
        let status = self.child.wait().expect("the protocol binary exits");
        assert!(status.success(), "the protocol binary exited with {status}");
    }
}

fn repo_file(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

fn fixture_steps(name: &str) -> Vec<String> {
    let path = repo_file(&format!("bench/fixtures/{name}.actions"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn fixture_expected(name: &str) -> String {
    let path = repo_file(&format!("bench/fixtures/{name}.expected"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
        .trim()
        .to_string()
}

fn render_of(reply: &Json) -> String {
    reply
        .get("state")
        .and_then(|s| s.get("render"))
        .and_then(Json::as_str)
        .expect("every response carries the re-rendered program")
        .to_string()
}

fn drive_fixture(name: &str) -> String {
    let mut driver = Driver::start();
    let steps = fixture_steps(name);
    assert!(!steps.is_empty(), "{name} has no actions");
    let mut render = String::new();
    for step in &steps {
        let reply = driver.step(step);
        assert_eq!(
            reply.get("ok").and_then(Json::as_bool),
            Some(true),
            "`{step}` was rejected: {reply}"
        );
        assert_eq!(
            reply.get("applied").and_then(Json::as_bool),
            Some(true),
            "`{step}` did not apply: {reply}"
        );
        render = render_of(&reply);
    }
    let state = driver.request(r#"{"method":"state"}"#);
    assert_eq!(render_of(&state), render);
    assert_eq!(
        state
            .get("state")
            .and_then(|s| s.get("well_typed"))
            .and_then(Json::as_bool),
        Some(true)
    );
    assert_eq!(
        state
            .get("state")
            .and_then(|s| s.get("log_len"))
            .and_then(Json::as_i64),
        Some(steps.len() as i64)
    );
    driver.finish();
    render
}

#[test]
fn an_external_process_drives_the_editor_through_the_factorial_reference_program() {
    assert_eq!(drive_fixture("factorial"), fixture_expected("factorial"));
}

#[test]
fn an_external_process_drives_the_editor_through_every_reference_fixture() {
    for name in [
        "factorial",
        "list_map",
        "record",
        "state_machine",
        "nested_conditional",
    ] {
        assert_eq!(
            drive_fixture(name),
            fixture_expected(name),
            "{name} did not replay to its expected render"
        );
    }
}

#[test]
fn a_whole_fixture_can_be_sent_as_one_script_request() {
    let mut driver = Driver::start();
    let steps: Vec<Json> = fixture_steps("factorial")
        .into_iter()
        .map(Json::Str)
        .collect();
    let request = Json::obj(vec![
        ("id", Json::Int(1)),
        ("method", Json::str("script")),
        ("params", Json::obj(vec![("steps", Json::arr(steps))])),
    ]);
    let reply = driver.request(&request.to_string());
    assert_eq!(reply.get("id").and_then(Json::as_i64), Some(1));
    assert_eq!(reply.get("applied").and_then(Json::as_bool), Some(true));
    assert_eq!(render_of(&reply), fixture_expected("factorial"));
    driver.finish();
}

#[test]
fn the_hole_context_query_answers_over_the_wire() {
    let mut driver = Driver::start();
    for step in [
        "construct-lam",
        "move-parent",
        "rename n",
        "set-ann Num",
        "move-child 0",
        "construct-var n",
        "construct-binop mul",
    ] {
        driver.step(step);
    }
    let reply = driver.request(r#"{"method":"hole_context"}"#);
    let context = reply
        .get("hole_context")
        .expect("a hole context comes back");
    assert_eq!(
        context.get("expected_ty_text").and_then(Json::as_str),
        Some("Num")
    );
    assert_eq!(
        context.get("at_empty_hole").and_then(Json::as_bool),
        Some(true)
    );

    let offered: Vec<String> = context
        .get("constructions")
        .and_then(Json::as_arr)
        .expect("constructions come back")
        .iter()
        .filter_map(|c| c.get("step").and_then(Json::as_str))
        .map(str::to_string)
        .collect();
    assert!(
        offered.iter().any(|s| s == "construct-var n"),
        "{offered:?}"
    );
    assert!(
        !offered.iter().any(|s| s.starts_with("construct-bool")),
        "a Num hole offered a boolean: {offered:?}"
    );

    for step in &offered {
        let before = driver.request(r#"{"method":"state"}"#);
        let before_holes = before
            .get("state")
            .and_then(|s| s.get("non_empty_holes"))
            .and_then(Json::as_i64)
            .unwrap();
        let reply = driver.step(step);
        assert_eq!(
            reply.get("applied").and_then(Json::as_bool),
            Some(true),
            "{step}"
        );
        let after_holes = reply
            .get("state")
            .and_then(|s| s.get("non_empty_holes"))
            .and_then(Json::as_i64)
            .unwrap();
        assert!(
            after_holes <= before_holes,
            "`{step}` produced a non-empty hole"
        );
        driver.request(r#"{"method":"undo"}"#);
    }
    driver.finish();
}

#[test]
fn undo_and_reset_work_over_the_wire() {
    let mut driver = Driver::start();
    driver.step("construct-num 1");
    driver.step("construct-binop add");
    let reply = driver.step("construct-num 2");
    assert_eq!(render_of(&reply), "1 + 2");
    let reply = driver.request(r#"{"method":"undo"}"#);
    assert_eq!(render_of(&reply), "1 + ⦇⦈");
    let reply = driver.request(r#"{"method":"redo"}"#);
    assert_eq!(render_of(&reply), "1 + 2");
    let reply = driver.request(r#"{"method":"reset"}"#);
    assert_eq!(render_of(&reply), "⦇⦈");
    driver.finish();
}

#[test]
fn save_and_load_work_over_the_wire() {
    let dir = std::env::temp_dir().join("nothing-agentapi-subprocess");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("factorial.nothing");
    let path = path.to_str().unwrap().to_string();

    let mut writer = Driver::start();
    for step in fixture_steps("factorial") {
        writer.step(&step);
    }
    let reply = writer.request(&format!(
        r#"{{"method":"save","params":{{"path":"{path}"}}}}"#
    ));
    assert_eq!(
        reply.get("ok").and_then(Json::as_bool),
        Some(true),
        "{reply}"
    );
    writer.finish();

    let mut reader = Driver::start();
    let reply = reader.request(&format!(
        r#"{{"method":"load","params":{{"path":"{path}"}}}}"#
    ));
    assert_eq!(
        reply.get("ok").and_then(Json::as_bool),
        Some(true),
        "{reply}"
    );
    assert_eq!(render_of(&reply), fixture_expected("factorial"));
    reader.finish();
    std::fs::remove_file(&path).ok();
}

#[test]
fn a_malformed_line_gets_an_error_response_and_the_process_stays_up() {
    let mut driver = Driver::start();
    let reply = driver.request("{ not json");
    assert_eq!(reply.get("ok").and_then(Json::as_bool), Some(false));
    let reply = driver.request(r#"{"method":"nope"}"#);
    assert_eq!(reply.get("ok").and_then(Json::as_bool), Some(false));
    let reply = driver.step("construct-num 5");
    assert_eq!(render_of(&reply), "5");
    driver.finish();
}
