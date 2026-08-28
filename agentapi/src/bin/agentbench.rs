use std::path::PathBuf;

use nothing_action::log::AuthorId;
use nothing_agentapi::holectx::hole_context;
use nothing_agentapi::json::Json;
use nothing_agentapi::measure::claude::{Claude, action_lines};
use nothing_agentapi::measure::tasks::{Task, tasks};
use nothing_agentapi::measure::text_parse::{parse_program, strip_fences};
use nothing_agentapi::session::AgentSession;
use nothing_core::render::render;
use nothing_core::typing::is_well_typed;

const HUMAN_AUTHOR: u64 = 1;
const MODEL_AUTHOR: u64 = 2;

const SYNTAX: &str = "\
The program is written in this syntax:
  numbers        0   1   -3
  booleans       true   false
  variable       a name bound by an enclosing λ or let
  function       λx:T. body        T is a type: Num, Bool, ?, T -> T, T * T
  application    f a               left associative, each argument is an atom
  operators      + - * < ==        * binds tightest, then + and -, then < and ==
  conditional    if c then a else b
  binding        let x = e in body
  pair           (a, b)
  projection     fst e     snd e
  empty hole     ⦇⦈
  grouping       ( e )
Every name you use must be bound by an enclosing λ or let. There are no other
built-in names, no strings, no lists and no recursion.";

fn start(task: &Task) -> Result<AgentSession, String> {
    let mut session = AgentSession::new(AuthorId::new(HUMAN_AUTHOR));
    for line in task.setup.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match session.apply_text(line) {
            Ok(true) => {}
            Ok(false) => return Err(format!("`{line}` did not apply")),
            Err(e) => return Err(format!("`{line}` did not parse: {e}")),
        }
    }
    session.set_author(AuthorId::new(MODEL_AUTHOR));
    Ok(session)
}

fn action_prompt(task: &Task, session: &AgentSession) -> String {
    let context = hole_context(session.state());
    let mut out = String::new();
    out.push_str("You are editing a program in a structural editor. There is no text and no\n");
    out.push_str("parser: you cannot type a program. You change it only by naming actions, and\n");
    out.push_str("each action either applies and leaves a well-typed program, or is refused.\n\n");
    out.push_str(&format!("Task: {}\n\n", task.goal));
    out.push_str(&format!(
        "Current program (the cursor is between » and «):\n  {}\n\n",
        nothing_action::cursor_render::render_with_cursor(&session.state().zipper, session.names())
    ));
    out.push_str(&context.to_prompt_block());
    out.push_str(
        "\nThe action grammar:\n\
         \x20 construct-num N            write a number\n\
         \x20 construct-bool true|false  write a boolean\n\
         \x20 construct-var NAME         refer to an in-scope name\n\
         \x20 construct-lam              e becomes λx:?. e\n\
         \x20 construct-ap               e becomes e ⦇⦈\n\
         \x20 construct-binop OP         e becomes e OP ⦇⦈   (add sub mul lt eq)\n\
         \x20 construct-if               e becomes if e then ⦇⦈ else ⦇⦈\n\
         \x20 construct-let              e becomes let x = e in ⦇⦈\n\
         \x20 construct-pair             e becomes (e, ⦇⦈)\n\
         \x20 construct-proj l|r         e becomes fst e / snd e\n\
         \x20 delete                     the focus becomes an empty hole\n\
         \x20 finish                     unwrap a quarantine ⦇e⦈ whose contents now fit\n\
         \x20 set-ann TYPE               set the λ parameter's type (cursor on the λ)\n\
         \x20 rename NAME                rename the binder (cursor on the λ or the let)\n\
         \x20 move-child N               descend to child N, 0-based, source order\n\
         \x20 move-parent                ascend\n\
         \x20 move-next-sibling          next child of the same parent\n\
         \x20 move-prev-sibling          previous child of the same parent\n\n\
         At an empty hole a construction fills the hole. On anything else the\n\
         construction wraps it, as shown above. After a construction the cursor lands\n\
         on the first new empty hole if the form has one.\n\n\
         Answer with the whole sequence of actions that finishes the task, one action\n\
         per line, in order, and nothing else: no prose, no numbering, no backticks.\n",
    );
    out
}

fn text_prompt(task: &Task, session: &AgentSession) -> String {
    let mut out = String::new();
    out.push_str("You are editing a program by rewriting it as text.\n\n");
    out.push_str(&format!("Task: {}\n\n", task.goal));
    out.push_str(&format!(
        "Current program:\n  {}\n\n",
        session.state().render()
    ));
    out.push_str(SYNTAX);
    out.push_str(
        "\n\nAnswer with the complete edited program on one line and nothing else:\n\
         no prose, no code fence, no explanation.\n",
    );
    out
}

struct Condition {
    edits: usize,
    invalid: usize,
    reached: usize,
    failed_calls: usize,
    retries: usize,
}

impl Condition {
    fn new() -> Condition {
        Condition {
            edits: 0,
            invalid: 0,
            reached: 0,
            failed_calls: 0,
            retries: 0,
        }
    }

    fn rate(&self) -> f64 {
        if self.edits == 0 {
            0.0
        } else {
            self.invalid as f64 / self.edits as f64
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

struct Options {
    out: PathBuf,
    only: Option<String>,
    limit: usize,
}

fn options() -> Options {
    let args: Vec<String> = std::env::args().collect();
    let mut options = Options {
        out: repo_root().join("bench/agent-transcripts/invalid-edit-rate.jsonl"),
        only: None,
        limit: usize::MAX,
    };
    let mut i = 1;
    while i < args.len() {
        let next = args.get(i + 1).cloned();
        match args[i].as_str() {
            "--out" => {
                if let Some(v) = next {
                    options.out = PathBuf::from(v);
                }
                i += 2;
            }
            "--only" => {
                options.only = next;
                i += 2;
            }
            "--limit" => {
                if let Some(v) = next.and_then(|v| v.parse().ok()) {
                    options.limit = v;
                }
                i += 2;
            }
            _ => i += 1,
        }
    }
    options
}

fn main() {
    let options = options();
    let claude = Claude::new();
    let all: Vec<Task> = tasks()
        .into_iter()
        .filter(|t| options.only.as_deref().is_none_or(|name| t.name == name))
        .take(options.limit)
        .collect();

    if let Some(parent) = options.out.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("error: cannot create {}: {e}", parent.display());
        std::process::exit(1);
    }

    let mut lines: Vec<String> = vec![
        Json::obj(vec![
            ("record", Json::str("run")),
            ("model", Json::str(claude.model.clone())),
            ("tasks", Json::Int(all.len() as i64)),
            (
                "conditions",
                Json::str("A=action protocol, B=text baseline"),
            ),
        ])
        .to_string(),
    ];

    let mut a = Condition::new();
    let mut b = Condition::new();

    for task in &all {
        let session = match start(task) {
            Ok(session) => session,
            Err(message) => {
                eprintln!("error: {}: {message}", task.name);
                std::process::exit(1);
            }
        };
        let start_render = session.state().render();

        let prompt = action_prompt(task, &session);
        let (a_record, a_reached) = match claude.ask(&prompt) {
            Err(message) => {
                a.failed_calls += 1;
                (
                    Json::obj(vec![
                        ("record", Json::str("task")),
                        ("task", Json::str(task.name)),
                        ("condition", Json::str("A")),
                        ("call_failed", Json::Bool(true)),
                        ("error", Json::str(message)),
                    ]),
                    false,
                )
            }
            Ok(reply) => {
                a.retries += reply.attempts - 1;
                let mut session = session.clone();
                let mut results = Vec::new();
                for line in action_lines(&reply.text) {
                    a.edits += 1;
                    let outcome = match session.apply_text(&line) {
                        Err(e) => {
                            a.invalid += 1;
                            ("parse_error", e.to_string())
                        }
                        Ok(false) => {
                            a.invalid += 1;
                            ("did_not_apply", String::new())
                        }
                        Ok(true) => ("applied", String::new()),
                    };
                    results.push(Json::obj(vec![
                        ("step", Json::str(line)),
                        ("outcome", Json::str(outcome.0)),
                        ("error", Json::str(outcome.1)),
                        ("render", Json::str(session.state().render())),
                        ("well_typed", Json::Bool(is_well_typed(&session.exp()))),
                    ]));
                }
                let render = session.state().render();
                let reached = render == task.target;
                if reached {
                    a.reached += 1;
                }
                (
                    Json::obj(vec![
                        ("record", Json::str("task")),
                        ("task", Json::str(task.name)),
                        ("family", Json::str(task.family.label())),
                        ("condition", Json::str("A")),
                        ("goal", Json::str(task.goal)),
                        ("start_render", Json::str(start_render.clone())),
                        ("target", Json::str(task.target)),
                        ("prompt", Json::str(prompt.clone())),
                        ("reply", Json::str(reply.text)),
                        ("attempts", Json::Int(reply.attempts as i64)),
                        ("edits", Json::Int(results.len() as i64)),
                        ("steps", Json::arr(results)),
                        ("final_render", Json::str(render)),
                        ("reached_target", Json::Bool(reached)),
                    ]),
                    reached,
                )
            }
        };
        lines.push(a_record.to_string());

        let prompt = text_prompt(task, &session);
        let (b_record, b_reached) = match claude.ask(&prompt) {
            Err(message) => {
                b.failed_calls += 1;
                (
                    Json::obj(vec![
                        ("record", Json::str("task")),
                        ("task", Json::str(task.name)),
                        ("condition", Json::str("B")),
                        ("call_failed", Json::Bool(true)),
                        ("error", Json::str(message)),
                    ]),
                    false,
                )
            }
            Ok(reply) => {
                b.retries += reply.attempts - 1;
                b.edits += 1;
                let body = strip_fences(&reply.text);
                let (outcome, error, render, reached) = match parse_program(&body) {
                    Err(e) => {
                        b.invalid += 1;
                        ("parse_error", e.to_string(), String::new(), false)
                    }
                    Ok(parsed) => {
                        let render = render(&parsed.exp, &parsed.names);
                        if is_well_typed(&parsed.exp) {
                            let reached = render == task.target;
                            if reached {
                                b.reached += 1;
                            }
                            ("well_typed", String::new(), render, reached)
                        } else {
                            b.invalid += 1;
                            (
                                "not_well_typed",
                                "synthesis failed in the empty context".to_string(),
                                render,
                                false,
                            )
                        }
                    }
                };
                (
                    Json::obj(vec![
                        ("record", Json::str("task")),
                        ("task", Json::str(task.name)),
                        ("family", Json::str(task.family.label())),
                        ("condition", Json::str("B")),
                        ("goal", Json::str(task.goal)),
                        ("start_render", Json::str(start_render)),
                        ("target", Json::str(task.target)),
                        ("prompt", Json::str(prompt)),
                        ("reply", Json::str(reply.text)),
                        ("attempts", Json::Int(reply.attempts as i64)),
                        ("edits", Json::Int(1)),
                        ("emitted", Json::str(body)),
                        ("outcome", Json::str(outcome)),
                        ("error", Json::str(error)),
                        ("final_render", Json::str(render)),
                        ("reached_target", Json::Bool(reached)),
                    ]),
                    reached,
                )
            }
        };
        lines.push(b_record.to_string());

        println!(
            "{:<28}  A: {:>2} edits, {:>2} invalid, target {}   B: {} target {}",
            task.name,
            a_record.get("edits").and_then(Json::as_i64).unwrap_or(0),
            a_record
                .get("steps")
                .and_then(Json::as_arr)
                .map(|s| s
                    .iter()
                    .filter(|r| r.get("outcome").and_then(Json::as_str) != Some("applied"))
                    .count())
                .unwrap_or(0),
            if a_reached { "yes" } else { "no " },
            b_record
                .get("outcome")
                .and_then(Json::as_str)
                .unwrap_or("call failed"),
            if b_reached { "yes" } else { "no " },
        );
    }

    let summary = Json::obj(vec![
        ("record", Json::str("summary")),
        ("model", Json::str(claude.model.clone())),
        ("tasks", Json::Int(all.len() as i64)),
        (
            "condition_a",
            Json::obj(vec![
                ("edits", Json::Int(a.edits as i64)),
                ("invalid", Json::Int(a.invalid as i64)),
                ("invalid_rate", Json::Float(a.rate())),
                ("reached_target", Json::Int(a.reached as i64)),
                ("failed_calls", Json::Int(a.failed_calls as i64)),
                ("retries", Json::Int(a.retries as i64)),
            ]),
        ),
        (
            "condition_b",
            Json::obj(vec![
                ("edits", Json::Int(b.edits as i64)),
                ("invalid", Json::Int(b.invalid as i64)),
                ("invalid_rate", Json::Float(b.rate())),
                ("reached_target", Json::Int(b.reached as i64)),
                ("failed_calls", Json::Int(b.failed_calls as i64)),
                ("retries", Json::Int(b.retries as i64)),
            ]),
        ),
    ]);
    lines.push(summary.to_string());

    if let Err(e) = std::fs::write(&options.out, lines.join("\n") + "\n") {
        eprintln!("error: cannot write {}: {e}", options.out.display());
        std::process::exit(1);
    }

    println!();
    println!("model                     {}", claude.model);
    println!("tasks                     {}", all.len());
    println!(
        "A  action protocol        {} edits, {} invalid  ({:.1}%), {} reached target, {} failed calls, {} retries",
        a.edits,
        a.invalid,
        a.rate() * 100.0,
        a.reached,
        a.failed_calls,
        a.retries
    );
    println!(
        "B  text baseline          {} edits, {} invalid  ({:.1}%), {} reached target, {} failed calls, {} retries",
        b.edits,
        b.invalid,
        b.rate() * 100.0,
        b.reached,
        b.failed_calls,
        b.retries
    );
    println!("transcript                {}", options.out.display());
}
