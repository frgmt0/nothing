use std::io::{BufRead, Write};

use nothing_agentapi::json::Json;
use nothing_agentapi::protocol::author_from_args;
use nothing_agentapi::session::AgentSession;

use crate::mcp::rpc::{self, Incoming};
use crate::mcp::tools;

pub const LATEST_PROTOCOL_VERSION: &str = "2025-06-18";

pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "\
`nothing` is a projectional structural editor. There is no parser and no source text: you build a \
program by naming actions, and every action either applies — leaving a program that is still well \
typed — or is refused, changing nothing. Call `action_grammar` once to learn the action names, then \
loop: `hole_context` to see the expected type and the constructions that are well typed at the \
cursor, `apply_action` or `apply_actions` to edit, `get_projection` to read the program back. \
`typecheck` reports what is left to fill in, `run` evaluates `main`, and `save_document` writes a \
`.nothing` file that the editor and `nothing run` read.";

pub struct Server {
    session: AgentSession,
    handshaken: bool,
}

impl Server {
    pub fn new(args: &[String]) -> Server {
        let author = author_from_args(args);
        let session = if args.iter().any(|arg| arg == "--no-stdlib") {
            AgentSession::new(author)
        } else {
            AgentSession::from_base(
                nothing_action::act::EditState::empty().under(nothing_stdlib::prelude()),
                nothing_action::log::ActionLog::new(),
                author,
            )
        };
        Server {
            session,
            handshaken: false,
        }
    }

    pub fn serve<R: BufRead, W: Write>(&mut self, input: R, mut output: W) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            let Some(message) = rpc::read_message(&line) else {
                continue;
            };
            let Some(response) = self.respond(message) else {
                continue;
            };
            writeln!(output, "{response}")?;
            output.flush()?;
        }
        Ok(())
    }

    fn respond(&mut self, message: Incoming) -> Option<Json> {
        match message {
            Incoming::Notification | Incoming::Reply => None,
            Incoming::Malformed(detail) => Some(rpc::failure(
                &Json::Null,
                rpc::PARSE_ERROR,
                format!("that line is not JSON: {detail}"),
            )),
            Incoming::Invalid { id, message } => {
                Some(rpc::failure(&id, rpc::INVALID_REQUEST, message))
            }
            Incoming::Request { id, method, params } => Some(self.answer(&id, &method, &params)),
        }
    }

    fn answer(&mut self, id: &Json, method: &str, params: &Json) -> Json {
        match method {
            "initialize" => {
                self.handshaken = true;
                rpc::success(id, initialize_result(params))
            }
            "ping" => rpc::success(id, Json::Obj(Vec::new())),
            _ if !self.handshaken => rpc::failure(
                id,
                rpc::INVALID_REQUEST,
                format!(
                    "`{method}` arrived before `initialize`; send an `initialize` request first"
                ),
            ),
            "tools/list" => rpc::success(id, tools::listing()),
            "tools/call" => self.call(id, params),
            other => rpc::failure(
                id,
                rpc::METHOD_NOT_FOUND,
                format!(
                    "this server implements `initialize`, `ping`, `tools/list` and \
                     `tools/call`; it does not implement `{other}`"
                ),
            ),
        }
    }

    fn call(&mut self, id: &Json, params: &Json) -> Json {
        let Some(name) = params.get("name").and_then(Json::as_str) else {
            return rpc::failure(
                id,
                rpc::INVALID_PARAMS,
                "`tools/call` needs a `name` string naming the tool",
            );
        };
        let empty = Json::Obj(Vec::new());
        let arguments = params.get("arguments").unwrap_or(&empty);
        if !matches!(arguments, Json::Obj(_)) {
            return rpc::failure(
                id,
                rpc::INVALID_PARAMS,
                "`tools/call` takes its `arguments` as a JSON object",
            );
        }
        let outcome = tools::call(&mut self.session, name, arguments);
        rpc::success(id, outcome.into_result())
    }
}

fn negotiated_version(params: &Json) -> &'static str {
    let requested = params.get("protocolVersion").and_then(Json::as_str);
    match requested {
        Some(version) => SUPPORTED_PROTOCOL_VERSIONS
            .iter()
            .copied()
            .find(|supported| *supported == version)
            .unwrap_or(LATEST_PROTOCOL_VERSION),
        None => LATEST_PROTOCOL_VERSION,
    }
}

fn initialize_result(params: &Json) -> Json {
    Json::obj(vec![
        ("protocolVersion", Json::str(negotiated_version(params))),
        (
            "capabilities",
            Json::obj(vec![(
                "tools",
                Json::obj(vec![("listChanged", Json::Bool(false))]),
            )]),
        ),
        (
            "serverInfo",
            Json::obj(vec![
                ("name", Json::str("nothing")),
                ("version", Json::str(env!("CARGO_PKG_VERSION"))),
            ]),
        ),
        ("instructions", Json::str(INSTRUCTIONS)),
    ])
}

pub fn run(args: &[String]) -> i32 {
    let mut server = Server::new(args);
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    match server.serve(stdin.lock(), stdout.lock()) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("error: {error}");
            1
        }
    }
}
