use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use nothing_agentapi::json::{Json, parse};

const TOOL_NAMES: &[&str] = &[
    "get_state",
    "get_projection",
    "hole_context",
    "apply_action",
    "apply_actions",
    "save_document",
    "load_document",
    "typecheck",
    "run",
    "stdlib",
    "action_grammar",
    "undo",
    "redo",
    "reset",
    "move_to_hole",
];

const GREETING_PROGRAM: &[&str] = &[
    "rename-def greet",
    "set-def-ann Str -> Str",
    "construct-lam",
    "move-parent",
    "rename who",
    "set-ann Str",
    "move-child 0",
    "construct-str \"hello, \"",
    "construct-binop concat",
    "construct-var who",
    "create-definition",
    "rename-def names",
    "set-def-ann List Str",
    "construct-cons",
    "construct-str \"world\"",
    "move-parent",
    "move-child 1",
    "construct-cons",
    "construct-str \"again\"",
    "move-parent",
    "move-child 1",
    "construct-nil",
    "create-definition",
    "rename-def main",
    "set-def-ann Str",
    "construct-ap",
    "construct-var greet",
    "move-parent",
    "move-child 1",
    "construct-ap",
    "construct-ap",
    "construct-var join",
    "move-parent",
    "move-child 1",
    "construct-str \", \"",
    "move-parent",
    "move-parent",
    "move-child 1",
    "construct-var names",
];

const COUNTDOWN_PROGRAM: &[&str] = &[
    "set-def-ann Cmd ?",
    "construct-bind",
    "move-parent",
    "rename who",
    "move-child 0",
    "construct-readline",
    "move-parent",
    "move-child 1",
    "construct-print",
    "construct-str \"hello, \"",
    "construct-binop concat",
    "construct-var who",
];

struct McpServer {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    transcript: Vec<String>,
    next_id: i64,
}

impl McpServer {
    fn start() -> McpServer {
        let mut child = Command::new(env!("CARGO_BIN_EXE_nothing"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the nothing binary starts");
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
        McpServer {
            child,
            stdin: Some(stdin),
            stdout,
            transcript: Vec::new(),
            next_id: 1,
        }
    }

    fn send_raw(&mut self, line: &str) {
        let stdin = self.stdin.as_mut().expect("stdin is still open");
        writeln!(stdin, "{line}").expect("the message is written");
        stdin.flush().expect("the message is flushed");
    }

    fn read_reply(&mut self) -> Json {
        let mut line = String::new();
        let read = self
            .stdout
            .read_line(&mut line)
            .expect("the server's stdout is readable");
        assert!(read > 0, "the mcp server closed stdout without replying");
        self.transcript.push(line.trim_end().to_string());
        parse(line.trim()).unwrap_or_else(|error| panic!("reply `{line}` is not JSON: {error}"))
    }

    fn request(&mut self, method: &str, params: Json) -> Json {
        let id = self.next_id;
        self.next_id += 1;
        let message = Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("id", Json::Int(id)),
            ("method", Json::str(method)),
            ("params", params),
        ]);
        self.send_raw(&message.to_string());
        let reply = self.read_reply();
        assert_eq!(
            reply.get("jsonrpc").and_then(Json::as_str),
            Some("2.0"),
            "every response carries the JSON-RPC version: {reply}"
        );
        assert_eq!(
            reply.get("id").and_then(Json::as_i64),
            Some(id),
            "a response must echo the request id verbatim: {reply}"
        );
        reply
    }

    fn notify(&mut self, method: &str) {
        let message = Json::obj(vec![
            ("jsonrpc", Json::str("2.0")),
            ("method", Json::str(method)),
        ]);
        self.send_raw(&message.to_string());
    }

    fn initialize(&mut self) -> Json {
        let result = self.request(
            "initialize",
            Json::obj(vec![
                ("protocolVersion", Json::str("2025-06-18")),
                ("capabilities", Json::Obj(Vec::new())),
                (
                    "clientInfo",
                    Json::obj(vec![
                        ("name", Json::str("nothing-integration-test")),
                        ("version", Json::str("0")),
                    ]),
                ),
            ]),
        );
        self.notify("notifications/initialized");
        result
    }

    fn call(&mut self, name: &str, arguments: Json) -> Json {
        let reply = self.request(
            "tools/call",
            Json::obj(vec![("name", Json::str(name)), ("arguments", arguments)]),
        );
        reply
            .get("result")
            .cloned()
            .unwrap_or_else(|| panic!("`{name}` answered with a JSON-RPC error: {reply}"))
    }

    fn assert_every_line_was_a_jsonrpc_message(&self) {
        for line in &self.transcript {
            let message = parse(line)
                .unwrap_or_else(|error| panic!("stdout line `{line}` is not JSON: {error}"));
            assert!(
                matches!(message, Json::Obj(_)),
                "stdout line `{line}` is not a JSON object"
            );
            assert_eq!(
                message.get("jsonrpc").and_then(Json::as_str),
                Some("2.0"),
                "stdout line `{line}` is not a JSON-RPC message"
            );
            assert!(
                message.get("id").is_some(),
                "stdout line `{line}` carries no id"
            );
            assert!(
                message.get("result").is_some() || message.get("error").is_some(),
                "stdout line `{line}` is neither a result nor an error"
            );
        }
    }

    fn finish(&mut self) {
        self.assert_every_line_was_a_jsonrpc_message();
        self.stdin.take();
        let mut trailing = String::new();
        self.stdout
            .read_to_string(&mut trailing)
            .expect("stdout drains at end of input");
        assert!(
            trailing.trim().is_empty(),
            "the server wrote unsolicited output after the last reply: {trailing}"
        );
        let status = self.child.wait().expect("the mcp server exits");
        assert_eq!(
            status.code(),
            Some(0),
            "a clean end of input should exit 0, got {status}"
        );
    }
}

impl Drop for McpServer {
    fn drop(&mut self) {
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn result_of(reply: &Json) -> &Json {
    reply
        .get("result")
        .unwrap_or_else(|| panic!("expected a result, got {reply}"))
}

fn error_code(reply: &Json) -> i64 {
    reply
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(Json::as_i64)
        .unwrap_or_else(|| panic!("expected a JSON-RPC error, got {reply}"))
}

fn tool_text(result: &Json) -> String {
    let content = result
        .get("content")
        .and_then(Json::as_arr)
        .unwrap_or_else(|| panic!("a tool result carries content: {result}"));
    content
        .iter()
        .filter_map(|block| block.get("text").and_then(Json::as_str))
        .collect::<Vec<&str>>()
        .join("\n")
}

fn is_error(result: &Json) -> bool {
    result
        .get("isError")
        .and_then(Json::as_bool)
        .unwrap_or_else(|| panic!("every tool result says whether it is an error: {result}"))
}

fn structured(result: &Json) -> &Json {
    result
        .get("structuredContent")
        .unwrap_or_else(|| panic!("expected structured content: {result}"))
}

fn steps(script: &[&str]) -> Json {
    Json::obj(vec![(
        "steps",
        Json::arr(script.iter().map(|step| Json::str(*step)).collect()),
    )])
}

fn scratch_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("nothing-mcp-tests");
    std::fs::create_dir_all(&dir).expect("the scratch directory is creatable");
    dir.join(format!("{}-{name}", std::process::id()))
}

#[test]
fn the_initialize_handshake_names_the_server_and_its_tool_capability() {
    let mut server = McpServer::start();
    let reply = server.initialize();
    let result = result_of(&reply);
    assert_eq!(
        result.get("protocolVersion").and_then(Json::as_str),
        Some("2025-06-18")
    );
    let info = result.get("serverInfo").expect("serverInfo is present");
    assert_eq!(info.get("name").and_then(Json::as_str), Some("nothing"));
    assert_eq!(
        info.get("version").and_then(Json::as_str),
        Some(env!("CARGO_PKG_VERSION"))
    );
    let tools = result
        .get("capabilities")
        .and_then(|capabilities| capabilities.get("tools"))
        .expect("the server declares a tools capability");
    assert_eq!(
        tools.get("listChanged").and_then(Json::as_bool),
        Some(false)
    );
    server.finish();
}

#[test]
fn the_protocol_version_is_echoed_when_supported_and_falls_back_when_it_is_not() {
    for (requested, expected) in [
        ("2025-06-18", "2025-06-18"),
        ("2025-03-26", "2025-03-26"),
        ("2024-11-05", "2024-11-05"),
        ("1999-01-01", "2025-06-18"),
    ] {
        let mut server = McpServer::start();
        let reply = server.request(
            "initialize",
            Json::obj(vec![("protocolVersion", Json::str(requested))]),
        );
        assert_eq!(
            result_of(&reply)
                .get("protocolVersion")
                .and_then(Json::as_str),
            Some(expected),
            "asked for {requested}"
        );
        server.finish();
    }

    let mut server = McpServer::start();
    let reply = server.request("initialize", Json::Obj(Vec::new()));
    assert_eq!(
        result_of(&reply)
            .get("protocolVersion")
            .and_then(Json::as_str),
        Some("2025-06-18"),
        "a client that names no version gets the latest supported one"
    );
    server.finish();
}

#[test]
fn the_initialized_notification_is_never_answered() {
    let mut server = McpServer::start();
    server.request(
        "initialize",
        Json::obj(vec![("protocolVersion", Json::str("2025-06-18"))]),
    );
    server.notify("notifications/initialized");
    server.notify("notifications/cancelled");
    let reply = server.request("ping", Json::Obj(Vec::new()));
    assert_eq!(
        result_of(&reply),
        &Json::Obj(Vec::new()),
        "the next reply must belong to the ping, not to a notification"
    );
    server.finish();
}

#[test]
fn a_request_before_initialize_is_refused_but_ping_is_not() {
    let mut server = McpServer::start();
    let reply = server.request("tools/list", Json::Obj(Vec::new()));
    assert_eq!(error_code(&reply), -32600);
    let reply = server.request("ping", Json::Obj(Vec::new()));
    assert_eq!(result_of(&reply), &Json::Obj(Vec::new()));
    server.initialize();
    let reply = server.request("tools/list", Json::Obj(Vec::new()));
    assert!(result_of(&reply).get("tools").is_some());
    server.finish();
}

#[test]
fn tools_list_describes_every_tool_with_a_well_formed_input_schema() {
    let mut server = McpServer::start();
    server.initialize();
    let reply = server.request("tools/list", Json::obj(vec![("cursor", Json::str("0"))]));
    let tools = result_of(&reply)
        .get("tools")
        .and_then(Json::as_arr)
        .expect("tools/list answers with a tools array")
        .to_vec();

    let names: Vec<&str> = tools
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Json::as_str))
        .collect();
    assert_eq!(names, TOOL_NAMES);

    for tool in &tools {
        let name = tool.get("name").and_then(Json::as_str).unwrap_or("?");
        let description = tool
            .get("description")
            .and_then(Json::as_str)
            .unwrap_or_else(|| panic!("`{name}` has no description"));
        assert!(
            description.len() > 40,
            "`{name}` needs a description an agent can act on"
        );
        let schema = tool
            .get("inputSchema")
            .unwrap_or_else(|| panic!("`{name}` has no inputSchema"));
        assert_eq!(
            schema.get("type").and_then(Json::as_str),
            Some("object"),
            "`{name}` must take an object"
        );
        let properties = schema
            .get("properties")
            .unwrap_or_else(|| panic!("`{name}` has no properties"));
        assert!(
            matches!(properties, Json::Obj(_)),
            "`{name}` must describe its properties as an object"
        );
        let required = schema
            .get("required")
            .and_then(Json::as_arr)
            .unwrap_or_else(|| panic!("`{name}` has no required list"));
        for argument in required {
            let argument = argument.as_str().expect("a required name is a string");
            assert!(
                properties.get(argument).is_some(),
                "`{name}` requires `{argument}` without describing it"
            );
        }
    }
    server.finish();
}

#[test]
fn an_agent_builds_a_program_saves_it_and_a_fresh_server_reloads_it_well_typed() {
    let path = scratch_path("greeting.nothing");
    std::fs::remove_file(&path).ok();
    let path_text = path
        .to_str()
        .expect("the scratch path is UTF-8")
        .to_string();

    let mut author = McpServer::start();
    author.initialize();

    let built = author.call("apply_actions", steps(GREETING_PROGRAM));
    assert!(!is_error(&built), "the build script was refused: {built}");
    let rendered = tool_text(&built);
    assert!(
        rendered.contains("greet : Str -> Str = λwho:Str. \"hello, \" ++ who"),
        "{rendered}"
    );
    assert!(
        rendered.contains("names : List Str = \"world\" :: \"again\" :: nil"),
        "{rendered}"
    );
    assert!(
        rendered.contains("main : Str = greet (join \", \" names)"),
        "{rendered}"
    );

    let checked = author.call("typecheck", Json::Obj(Vec::new()));
    assert_eq!(
        structured(&checked)
            .get("well_typed")
            .and_then(Json::as_bool),
        Some(true)
    );
    assert_eq!(
        structured(&checked).get("complete").and_then(Json::as_bool),
        Some(true)
    );

    let saved = author.call(
        "save_document",
        Json::obj(vec![("path", Json::str(path_text.clone()))]),
    );
    assert!(!is_error(&saved), "{saved}");
    author.finish();
    assert!(path.exists(), "save_document wrote no file");

    let mut reader = McpServer::start();
    reader.initialize();
    let loaded = reader.call(
        "load_document",
        Json::obj(vec![("path", Json::str(path_text))]),
    );
    assert!(!is_error(&loaded), "{loaded}");

    let checked = reader.call("typecheck", Json::Obj(Vec::new()));
    let report = structured(&checked);
    assert_eq!(report.get("well_typed").and_then(Json::as_bool), Some(true));
    assert_eq!(report.get("complete").and_then(Json::as_bool), Some(true));
    assert_eq!(report.get("empty_holes").and_then(Json::as_i64), Some(0));
    assert_eq!(
        report.get("non_empty_holes").and_then(Json::as_i64),
        Some(0)
    );
    let definitions: Vec<&str> = report
        .get("definitions")
        .and_then(Json::as_arr)
        .expect("the report names every definition")
        .iter()
        .filter_map(|definition| definition.get("name").and_then(Json::as_str))
        .collect();
    assert_eq!(definitions, vec!["greet", "names", "main"]);

    let projection = reader.call(
        "get_projection",
        Json::obj(vec![("projection", Json::str("document"))]),
    );
    assert!(
        tool_text(&projection).contains("main : Str = greet (join \", \" names)"),
        "{}",
        tool_text(&projection)
    );

    let ran = reader.call("run", Json::Obj(Vec::new()));
    assert_eq!(
        structured(&ran).get("value").and_then(Json::as_str),
        Some("\"hello, world, again\"")
    );
    reader.finish();
    std::fs::remove_file(&path).ok();
}

#[test]
fn running_a_command_captures_what_it_printed_instead_of_writing_it_to_stdout() {
    let mut server = McpServer::start();
    server.initialize();
    let built = server.call("apply_actions", steps(COUNTDOWN_PROGRAM));
    assert!(!is_error(&built), "{built}");

    let ran = server.call(
        "run",
        Json::obj(vec![("stdin_lines", Json::arr(vec![Json::str("ada")]))]),
    );
    assert!(!is_error(&ran), "{ran}");
    let report = structured(&ran);
    assert_eq!(report.get("performed").and_then(Json::as_bool), Some(true));
    assert_eq!(report.get("status").and_then(Json::as_i64), Some(0));
    assert_eq!(
        report.get("printed").and_then(Json::as_arr),
        Some([Json::str("hello, ada")].as_slice()),
        "what the program printed belongs in the tool result, not on stdout"
    );
    assert!(tool_text(&ran).contains("hello, ada"));

    let state = server.call("get_state", Json::Obj(Vec::new()));
    assert!(!is_error(&state));
    server.finish();
}

#[test]
fn a_malformed_line_is_a_parse_error_and_the_server_keeps_running() {
    let mut server = McpServer::start();
    server.initialize();
    for bad in ["{ not json", "[1,2,3", "\"unterminated", "{\"a\"}"] {
        server.send_raw(bad);
        let reply = server.read_reply();
        assert_eq!(
            error_code(&reply),
            -32700,
            "`{bad}` should be a parse error"
        );
        assert_eq!(reply.get("id"), Some(&Json::Null));
    }
    server.send_raw("[1,2,3]");
    let reply = server.read_reply();
    assert_eq!(error_code(&reply), -32600);

    server.send_raw("");
    server.send_raw("   ");
    let alive = server.call("get_state", Json::Obj(Vec::new()));
    assert!(!is_error(&alive), "{alive}");
    server.finish();
}

#[test]
fn an_unknown_tool_is_a_tool_error_and_an_unknown_method_is_method_not_found() {
    let mut server = McpServer::start();
    server.initialize();

    let missing = server.call("no_such_tool", Json::Obj(Vec::new()));
    assert!(
        is_error(&missing),
        "an unknown tool is reported through isError, not a JSON-RPC error"
    );
    assert!(tool_text(&missing).contains("no_such_tool"));
    assert!(tool_text(&missing).contains("hole_context"));

    let reply = server.request("resources/list", Json::Obj(Vec::new()));
    assert_eq!(error_code(&reply), -32601);

    let reply = server.request("tools/call", Json::obj(vec![("nome", Json::str("run"))]));
    assert_eq!(error_code(&reply), -32602);

    let alive = server.call("get_state", Json::Obj(Vec::new()));
    assert!(!is_error(&alive));
    server.finish();
}

#[test]
fn an_action_that_does_not_apply_is_refused_without_changing_the_program() {
    let mut server = McpServer::start();
    server.initialize();
    server.call(
        "apply_action",
        Json::obj(vec![("step", Json::str("construct-num 7"))]),
    );
    let before = tool_text(&server.call(
        "get_projection",
        Json::obj(vec![("projection", Json::str("document"))]),
    ));

    let refused = server.call(
        "apply_action",
        Json::obj(vec![("step", Json::str("move-parent"))]),
    );
    assert!(is_error(&refused), "{refused}");
    assert_eq!(
        structured(&refused).get("applied").and_then(Json::as_bool),
        Some(false)
    );

    let unknown = server.call(
        "apply_action",
        Json::obj(vec![("step", Json::str("construct-teleport"))]),
    );
    assert!(is_error(&unknown), "{unknown}");

    let after = tool_text(&server.call(
        "get_projection",
        Json::obj(vec![("projection", Json::str("document"))]),
    ));
    assert_eq!(before, after, "a refused action must change nothing");
    server.finish();
}

#[test]
fn the_hole_context_tool_offers_only_constructions_that_are_well_typed_here() {
    let mut server = McpServer::start();
    server.initialize();
    server.call(
        "apply_actions",
        steps(&["construct-num 1", "construct-binop add"]),
    );

    let context = server.call("hole_context", Json::Obj(Vec::new()));
    assert!(!is_error(&context), "{context}");
    let offered: Vec<String> = structured(&context)
        .get("hole_context")
        .and_then(|value| value.get("constructions"))
        .and_then(Json::as_arr)
        .expect("the hole context lists constructions")
        .iter()
        .filter_map(|construction| construction.get("step").and_then(Json::as_str))
        .map(str::to_string)
        .collect();
    assert!(
        offered.iter().any(|step| step.starts_with("construct-num")),
        "{offered:?}"
    );
    assert!(
        !offered
            .iter()
            .any(|step| step.starts_with("construct-bool")),
        "a Num hole must not offer a boolean: {offered:?}"
    );
    assert!(tool_text(&context).contains("expected type at cursor: Num"));

    for step in &offered {
        let applied = server.call(
            "apply_action",
            Json::obj(vec![("step", Json::str(step.clone()))]),
        );
        assert!(!is_error(&applied), "`{step}` was offered but refused");
        assert_eq!(
            structured(&applied)
                .get("state")
                .and_then(|state| state.get("non_empty_holes"))
                .and_then(Json::as_i64),
            Some(0),
            "`{step}` was offered but produced a non-empty hole"
        );
        server.call("undo", Json::Obj(Vec::new()));
    }
    server.finish();
}

#[test]
fn the_grammar_undo_reset_and_stdlib_tools_answer() {
    let mut server = McpServer::start();
    server.initialize();

    let grammar = server.call("action_grammar", Json::Obj(Vec::new()));
    assert!(tool_text(&grammar).contains("construct-lam"));
    assert!(tool_text(&grammar).contains("set-def-ann"));

    let stdlib = server.call("stdlib", Json::obj(vec![("filter", Json::str("join"))]));
    assert!(
        tool_text(&stdlib).contains("join : Str -> List Str -> Str"),
        "{}",
        tool_text(&stdlib)
    );

    server.call(
        "apply_action",
        Json::obj(vec![("step", Json::str("construct-num 3"))]),
    );
    let undone = server.call("undo", Json::Obj(Vec::new()));
    assert!(!is_error(&undone), "{undone}");
    let redone = server.call("redo", Json::Obj(Vec::new()));
    assert!(tool_text(&redone).contains('3'), "{}", tool_text(&redone));

    let reset = server.call("reset", Json::Obj(Vec::new()));
    assert!(!is_error(&reset), "{reset}");
    assert_eq!(
        structured(&reset)
            .get("state")
            .and_then(|state| state.get("can_undo"))
            .and_then(Json::as_bool),
        Some(false)
    );

    let moved = server.call(
        "move_to_hole",
        Json::obj(vec![("forward", Json::Bool(true))]),
    );
    assert!(!is_error(&moved), "{moved}");
    server.finish();
}
