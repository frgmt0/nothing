use std::path::PathBuf;
use std::sync::Mutex;

use nothing_action::log::AuthorId;
use nothing_agentapi::holectx::hole_context;
use nothing_agentapi::json::Json;
use nothing_agentapi::measure::claude::{Claude, action_lines, first_meaningful_line};
use nothing_agentapi::measure::legend::{ORIGINAL_SYNTAX, post_b2_action_grammar, post_b2_syntax};
use nothing_agentapi::measure::tasks::{Family, Task, post_b2_tasks, tasks};
use nothing_agentapi::measure::text_parse::{parse_program, strip_fences};
use nothing_agentapi::session::AgentSession;
use nothing_core::render::render;
use nothing_core::typing::is_well_typed;

const HUMAN_AUTHOR: u64 = 1;
const MODEL_AUTHOR: u64 = 2;
const DONE_SENTINEL: &str = "done";
const HISTORY_SHOWN: usize = 6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TaskSet {
    Original,
    PostB2,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    OneShot,
    Interactive,
}

struct Options {
    out: PathBuf,
    only: Option<String>,
    limit: usize,
    task_set: TaskSet,
    mode: Mode,
    max_steps: usize,
    baseline_turns: usize,
    workers: usize,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn options() -> Options {
    let args: Vec<String> = std::env::args().collect();
    let mut out: Option<PathBuf> = None;
    let mut only: Option<String> = None;
    let mut limit = usize::MAX;
    let mut task_set = TaskSet::Original;
    let mut mode: Option<Mode> = None;
    let mut max_steps = 30usize;
    let mut baseline_turns = 5usize;
    let mut workers = 4usize;
    let mut i = 1;
    while i < args.len() {
        let next = args.get(i + 1).cloned();
        match args[i].as_str() {
            "--out" => {
                out = next.map(PathBuf::from);
                i += 2;
            }
            "--only" => {
                only = next;
                i += 2;
            }
            "--limit" => {
                if let Some(v) = next.and_then(|v| v.parse().ok()) {
                    limit = v;
                }
                i += 2;
            }
            "--tasks" => {
                match next.as_deref() {
                    Some("post-b2") => task_set = TaskSet::PostB2,
                    Some("original") | None => task_set = TaskSet::Original,
                    Some(other) => {
                        eprintln!("error: unknown task set `{other}` (original|post-b2)");
                        std::process::exit(1);
                    }
                }
                i += 2;
            }
            "--mode" => {
                match next.as_deref() {
                    Some("one-shot") => mode = Some(Mode::OneShot),
                    Some("interactive") => mode = Some(Mode::Interactive),
                    other => {
                        eprintln!("error: unknown mode `{other:?}` (one-shot|interactive)");
                        std::process::exit(1);
                    }
                }
                i += 2;
            }
            "--max-steps" => {
                if let Some(v) = next.and_then(|v| v.parse().ok()) {
                    max_steps = v;
                }
                i += 2;
            }
            "--baseline-turns" => {
                if let Some(v) = next.and_then(|v| v.parse().ok()) {
                    baseline_turns = v;
                }
                i += 2;
            }
            "--workers" => {
                if let Some(v) = next.and_then(|v| v.parse().ok()).filter(|v| *v > 0) {
                    workers = v;
                }
                i += 2;
            }
            _ => i += 1,
        }
    }
    let mode = mode.unwrap_or(match task_set {
        TaskSet::Original => Mode::OneShot,
        TaskSet::PostB2 => Mode::Interactive,
    });
    let out = out.unwrap_or_else(|| match task_set {
        TaskSet::Original => repo_root().join("bench/agent-transcripts/invalid-edit-rate.jsonl"),
        TaskSet::PostB2 => {
            repo_root().join("bench/agent-transcripts/post-b2-invalid-edit-rate.jsonl")
        }
    });
    Options {
        out,
        only,
        limit,
        task_set,
        mode,
        max_steps,
        baseline_turns,
        workers,
    }
}

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

#[derive(Clone, Default)]
struct Arm {
    calls: usize,
    edits: usize,
    invalid: usize,
    parse_errors: usize,
    did_not_apply: usize,
    ill_typed_steps: usize,
    type_errors: usize,
    failed_calls: usize,
    retries: usize,
    reached: bool,
    final_render: String,
    record: Option<Json>,
}

#[derive(Clone, Default)]
struct Totals {
    calls: usize,
    edits: usize,
    invalid: usize,
    parse_errors: usize,
    did_not_apply: usize,
    ill_typed_steps: usize,
    type_errors: usize,
    failed_calls: usize,
    retries: usize,
    reached: usize,
    tasks_with_an_invalid_edit: usize,
}

impl Totals {
    fn absorb(&mut self, arm: &Arm) {
        self.calls += arm.calls;
        self.edits += arm.edits;
        self.invalid += arm.invalid;
        self.parse_errors += arm.parse_errors;
        self.did_not_apply += arm.did_not_apply;
        self.ill_typed_steps += arm.ill_typed_steps;
        self.type_errors += arm.type_errors;
        self.failed_calls += arm.failed_calls;
        self.retries += arm.retries;
        if arm.reached {
            self.reached += 1;
        }
        if arm.invalid > 0 {
            self.tasks_with_an_invalid_edit += 1;
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

fn one_shot_action_prompt(task: &Task, session: &AgentSession) -> String {
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
    out.push_str(ORIGINAL_ACTION_GRAMMAR);
    out
}

const ORIGINAL_ACTION_GRAMMAR: &str = "\
\nThe action grammar:\n\
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
         per line, in order, and nothing else: no prose, no numbering, no backticks.\n";

struct InteractiveTurn<'a> {
    task: &'a Task,
    session: &'a AgentSession,
    history: &'a [String],
    step: usize,
    cap: usize,
}

fn interactive_action_prompt(turn: &InteractiveTurn<'_>) -> String {
    let context = hole_context(turn.session.state());
    let mut out = String::new();
    out.push_str("You are editing a program in a structural editor. There is no text and no\n");
    out.push_str("parser: you cannot type a program. You change it only by naming actions, and\n");
    out.push_str("each action either applies and leaves a well-typed program, or is refused.\n\n");
    out.push_str(&format!("Task: {}\n\n", turn.task.goal));
    out.push_str(&format!(
        "Current program (the cursor is between » and «):\n  {}\n\n",
        nothing_action::cursor_render::render_with_cursor(
            &turn.session.state().zipper,
            turn.session.names()
        )
    ));
    out.push_str(&context.to_prompt_block());
    out.push('\n');
    out.push_str(&post_b2_action_grammar());
    out.push('\n');
    if !turn.history.is_empty() {
        out.push_str("\nWhat you have done so far, most recent last:\n");
        for line in turn.history.iter().rev().take(HISTORY_SHOWN).rev() {
            out.push_str(&format!("  {line}\n"));
        }
    }
    out.push_str(&format!(
        "\nThis is step {} of at most {}.\n",
        turn.step, turn.cap
    ));
    out.push_str(
        "Answer with exactly one action, on one line, and nothing else: no prose, no\n\
         numbering, no backticks. If the task is already finished, answer `done`.\n",
    );
    out
}

fn one_shot_text_prompt(task: &Task, start_render: &str, syntax: &str) -> String {
    let mut out = String::new();
    out.push_str("You are editing a program by rewriting it as text.\n\n");
    out.push_str(&format!("Task: {}\n\n", task.goal));
    out.push_str(&format!("Current program:\n  {start_render}\n\n"));
    out.push_str(syntax);
    out.push_str(
        "\n\nAnswer with the complete edited program on one line and nothing else:\n\
         no prose, no code fence, no explanation.\n",
    );
    out
}

fn interactive_text_prompt(
    task: &Task,
    current_render: &str,
    history: &[String],
    turn: usize,
    cap: usize,
) -> String {
    let mut out = String::new();
    out.push_str("You are editing a program by rewriting it as text.\n\n");
    out.push_str(&format!("Task: {}\n\n", task.goal));
    out.push_str(&format!("Current program:\n  {current_render}\n\n"));
    out.push_str(&post_b2_syntax());
    if !history.is_empty() {
        out.push_str("\n\nWhat you have tried so far, most recent last:\n");
        for line in history.iter().rev().take(HISTORY_SHOWN).rev() {
            out.push_str(&format!("  {line}\n"));
        }
    }
    out.push_str(&format!("\n\nThis is attempt {turn} of at most {cap}.\n"));
    out.push_str(
        "Answer with the complete edited program on one line and nothing else: no\n\
         prose, no code fence, no explanation. If the program above already finishes\n\
         the task, answer `done`.\n",
    );
    out
}

fn task_header(task: &Task, condition: &str, start_render: &str) -> Vec<(&'static str, Json)> {
    vec![
        ("record", Json::str("task")),
        ("task", Json::str(task.name)),
        ("family", Json::str(task.family.label())),
        ("condition", Json::str(condition)),
        ("goal", Json::str(task.goal)),
        ("start_render", Json::str(start_render.to_string())),
        ("target", Json::str(task.target)),
    ]
}

fn run_one_shot_actions(claude: &Claude, task: &Task, session: &AgentSession) -> Arm {
    let start_render = session.state().render();
    let mut arm = Arm {
        final_render: start_render.clone(),
        ..Arm::default()
    };
    let prompt = one_shot_action_prompt(task, session);
    arm.calls += 1;
    let reply = match claude.ask(&prompt) {
        Err(message) => {
            arm.failed_calls += 1;
            let mut fields = task_header(task, "A", &start_render);
            fields.push(("call_failed", Json::Bool(true)));
            fields.push(("error", Json::str(message)));
            arm.record = Some(Json::obj(fields));
            return arm;
        }
        Ok(reply) => reply,
    };
    arm.retries += reply.attempts - 1;
    let mut session = session.clone();
    let mut steps = Vec::new();
    for line in action_lines(&reply.text) {
        arm.edits += 1;
        let (outcome, error) = match session.apply_text(&line) {
            Err(e) => {
                arm.invalid += 1;
                arm.parse_errors += 1;
                ("parse_error", e.to_string())
            }
            Ok(false) => {
                arm.invalid += 1;
                arm.did_not_apply += 1;
                ("did_not_apply", String::new())
            }
            Ok(true) => ("applied", String::new()),
        };
        let well_typed = is_well_typed(&session.exp());
        if !well_typed {
            arm.ill_typed_steps += 1;
        }
        steps.push(Json::obj(vec![
            ("step", Json::str(line)),
            ("outcome", Json::str(outcome)),
            ("error", Json::str(error)),
            ("render", Json::str(session.state().render())),
            ("well_typed", Json::Bool(well_typed)),
        ]));
    }
    arm.final_render = session.state().render();
    arm.reached = arm.final_render == task.target;
    let mut fields = task_header(task, "A", &start_render);
    fields.push(("prompt", Json::str(prompt)));
    fields.push(("reply", Json::str(reply.text)));
    fields.push(("attempts", Json::Int(reply.attempts as i64)));
    fields.push(("edits", Json::Int(arm.edits as i64)));
    fields.push(("steps", Json::arr(steps)));
    fields.push(("final_render", Json::str(arm.final_render.clone())));
    fields.push(("reached_target", Json::Bool(arm.reached)));
    arm.record = Some(Json::obj(fields));
    arm
}

fn run_interactive_actions(
    claude: &Claude,
    task: &Task,
    session: &AgentSession,
    cap: usize,
) -> Arm {
    let start_render = session.state().render();
    let mut arm = Arm {
        final_render: start_render.clone(),
        ..Arm::default()
    };
    let mut session = session.clone();
    let mut history: Vec<String> = Vec::new();
    let mut steps = Vec::new();
    let mut stop_reason = "step_cap";

    for step in 1..=cap {
        if session.state().render() == task.target {
            stop_reason = "reached_target";
            break;
        }
        let prompt = interactive_action_prompt(&InteractiveTurn {
            task,
            session: &session,
            history: &history,
            step,
            cap,
        });
        arm.calls += 1;
        let reply = match claude.ask(&prompt) {
            Ok(reply) => reply,
            Err(message) => {
                arm.failed_calls += 1;
                stop_reason = "call_failed";
                steps.push(Json::obj(vec![
                    ("step", Json::Int(step as i64)),
                    ("prompt", Json::str(prompt)),
                    ("call_failed", Json::Bool(true)),
                    ("error", Json::str(message)),
                ]));
                break;
            }
        };
        arm.retries += reply.attempts - 1;
        let action = first_meaningful_line(&reply.text);
        if action.eq_ignore_ascii_case(DONE_SENTINEL) {
            stop_reason = "model_said_done";
            steps.push(Json::obj(vec![
                ("step", Json::Int(step as i64)),
                ("prompt", Json::str(prompt)),
                ("reply", Json::str(reply.text)),
                ("action", Json::str(DONE_SENTINEL)),
                ("outcome", Json::str("done")),
                ("counts_as_an_edit", Json::Bool(false)),
                ("render", Json::str(session.state().render())),
            ]));
            break;
        }
        arm.edits += 1;
        let (outcome, error) = match session.apply_text(&action) {
            Err(e) => {
                arm.invalid += 1;
                arm.parse_errors += 1;
                ("parse_error", e.to_string())
            }
            Ok(false) => {
                arm.invalid += 1;
                arm.did_not_apply += 1;
                ("did_not_apply", String::new())
            }
            Ok(true) => ("applied", String::new()),
        };
        let well_typed = is_well_typed(&session.exp());
        if !well_typed {
            arm.ill_typed_steps += 1;
        }
        let after = nothing_action::cursor_render::render_with_cursor(
            &session.state().zipper,
            session.names(),
        );
        history.push(if outcome == "applied" {
            format!("{action}   ->   {after}")
        } else {
            format!("{action}   ->   REFUSED ({outcome} {error})")
        });
        steps.push(Json::obj(vec![
            ("step", Json::Int(step as i64)),
            ("prompt", Json::str(prompt)),
            ("reply", Json::str(reply.text)),
            ("action", Json::str(action)),
            ("outcome", Json::str(outcome)),
            ("counts_as_an_edit", Json::Bool(true)),
            ("error", Json::str(error)),
            ("render", Json::str(session.state().render())),
            ("render_with_cursor", Json::str(after)),
            ("well_typed", Json::Bool(well_typed)),
        ]));
    }

    arm.final_render = session.state().render();
    arm.reached = arm.final_render == task.target;
    if arm.reached && stop_reason == "step_cap" {
        stop_reason = "reached_target";
    }
    let mut fields = task_header(task, "A", &start_render);
    fields.push(("loop", Json::str("interactive, one action per call")));
    fields.push(("step_cap", Json::Int(cap as i64)));
    fields.push(("calls", Json::Int(arm.calls as i64)));
    fields.push(("edits", Json::Int(arm.edits as i64)));
    fields.push(("invalid", Json::Int(arm.invalid as i64)));
    fields.push(("parse_errors", Json::Int(arm.parse_errors as i64)));
    fields.push(("did_not_apply", Json::Int(arm.did_not_apply as i64)));
    fields.push(("ill_typed_steps", Json::Int(arm.ill_typed_steps as i64)));
    fields.push(("stop_reason", Json::str(stop_reason)));
    fields.push(("steps", Json::arr(steps)));
    fields.push(("final_render", Json::str(arm.final_render.clone())));
    fields.push(("reached_target", Json::Bool(arm.reached)));
    arm.record = Some(Json::obj(fields));
    arm
}

fn score_text_reply(body: &str, target: &str) -> (&'static str, String, String, bool) {
    match parse_program(body) {
        Err(e) => ("parse_error", e.to_string(), String::new(), false),
        Ok(parsed) => {
            let rendered = render(&parsed.exp, &parsed.names);
            if is_well_typed(&parsed.exp) {
                let reached = rendered == target;
                ("well_typed", String::new(), rendered, reached)
            } else {
                (
                    "not_well_typed",
                    "synthesis failed in the empty context".to_string(),
                    rendered,
                    false,
                )
            }
        }
    }
}

fn run_one_shot_text(claude: &Claude, task: &Task, start_render: &str, syntax: &str) -> Arm {
    let mut arm = Arm {
        final_render: start_render.to_string(),
        ..Arm::default()
    };
    let prompt = one_shot_text_prompt(task, start_render, syntax);
    arm.calls += 1;
    let reply = match claude.ask(&prompt) {
        Err(message) => {
            arm.failed_calls += 1;
            let mut fields = task_header(task, "B", start_render);
            fields.push(("call_failed", Json::Bool(true)));
            fields.push(("error", Json::str(message)));
            arm.record = Some(Json::obj(fields));
            return arm;
        }
        Ok(reply) => reply,
    };
    arm.retries += reply.attempts - 1;
    arm.edits += 1;
    let body = strip_fences(&reply.text);
    let (outcome, error, rendered, reached) = score_text_reply(&body, task.target);
    if outcome != "well_typed" {
        arm.invalid += 1;
        if outcome == "parse_error" {
            arm.parse_errors += 1;
        } else {
            arm.type_errors += 1;
        }
    } else {
        arm.final_render = rendered.clone();
    }
    arm.reached = reached;
    let mut fields = task_header(task, "B", start_render);
    fields.push(("prompt", Json::str(prompt)));
    fields.push(("reply", Json::str(reply.text)));
    fields.push(("attempts", Json::Int(reply.attempts as i64)));
    fields.push(("edits", Json::Int(1)));
    fields.push(("emitted", Json::str(body)));
    fields.push(("outcome", Json::str(outcome)));
    fields.push(("error", Json::str(error)));
    fields.push(("final_render", Json::str(rendered)));
    fields.push(("reached_target", Json::Bool(reached)));
    arm.record = Some(Json::obj(fields));
    arm
}

fn run_interactive_text(claude: &Claude, task: &Task, start_render: &str, cap: usize) -> Arm {
    let mut arm = Arm {
        final_render: start_render.to_string(),
        ..Arm::default()
    };
    let mut current = start_render.to_string();
    let mut history: Vec<String> = Vec::new();
    let mut attempts = Vec::new();
    let mut stop_reason = "turn_cap";

    for turn in 1..=cap {
        if current == task.target {
            stop_reason = "reached_target";
            break;
        }
        let prompt = interactive_text_prompt(task, &current, &history, turn, cap);
        arm.calls += 1;
        let reply = match claude.ask(&prompt) {
            Ok(reply) => reply,
            Err(message) => {
                arm.failed_calls += 1;
                stop_reason = "call_failed";
                attempts.push(Json::obj(vec![
                    ("turn", Json::Int(turn as i64)),
                    ("prompt", Json::str(prompt)),
                    ("call_failed", Json::Bool(true)),
                    ("error", Json::str(message)),
                ]));
                break;
            }
        };
        arm.retries += reply.attempts - 1;
        let body = strip_fences(&reply.text);
        if body.trim().eq_ignore_ascii_case(DONE_SENTINEL) {
            stop_reason = "model_said_done";
            attempts.push(Json::obj(vec![
                ("turn", Json::Int(turn as i64)),
                ("prompt", Json::str(prompt)),
                ("reply", Json::str(reply.text)),
                ("outcome", Json::str("done")),
                ("counts_as_an_edit", Json::Bool(false)),
                ("render", Json::str(current.clone())),
            ]));
            break;
        }
        arm.edits += 1;
        let (outcome, error, rendered, reached) = score_text_reply(&body, task.target);
        if outcome == "well_typed" {
            current = rendered.clone();
            history.push(format!("attempt {turn} was accepted and gave   {rendered}"));
        } else {
            arm.invalid += 1;
            if outcome == "parse_error" {
                arm.parse_errors += 1;
            } else {
                arm.type_errors += 1;
            }
            history.push(format!(
                "attempt {turn} was rejected ({outcome}: {error}) and changed nothing"
            ));
        }
        attempts.push(Json::obj(vec![
            ("turn", Json::Int(turn as i64)),
            ("prompt", Json::str(prompt)),
            ("reply", Json::str(reply.text)),
            ("emitted", Json::str(body)),
            ("outcome", Json::str(outcome)),
            ("counts_as_an_edit", Json::Bool(true)),
            ("error", Json::str(error)),
            ("render", Json::str(rendered)),
            ("reached_target", Json::Bool(reached)),
        ]));
        if reached {
            stop_reason = "reached_target";
            break;
        }
    }

    arm.final_render = current.clone();
    arm.reached = current == task.target;
    let mut fields = task_header(task, "B2", start_render);
    fields.push(("loop", Json::str("interactive, whole program per call")));
    fields.push(("turn_cap", Json::Int(cap as i64)));
    fields.push(("calls", Json::Int(arm.calls as i64)));
    fields.push(("edits", Json::Int(arm.edits as i64)));
    fields.push(("invalid", Json::Int(arm.invalid as i64)));
    fields.push(("stop_reason", Json::str(stop_reason)));
    fields.push(("attempts", Json::arr(attempts)));
    fields.push(("final_render", Json::str(arm.final_render.clone())));
    fields.push(("reached_target", Json::Bool(arm.reached)));
    arm.record = Some(Json::obj(fields));
    arm
}

struct TaskRun {
    a: Arm,
    b: Arm,
    b2: Option<Arm>,
}

fn run_task(claude: &Claude, task: &Task, options: &Options) -> Result<TaskRun, String> {
    let session = start(task)?;
    let start_render = session.state().render();
    match options.mode {
        Mode::OneShot => Ok(TaskRun {
            a: run_one_shot_actions(claude, task, &session),
            b: run_one_shot_text(claude, task, &start_render, ORIGINAL_SYNTAX),
            b2: None,
        }),
        Mode::Interactive => {
            let syntax = post_b2_syntax();
            Ok(TaskRun {
                a: run_interactive_actions(claude, task, &session, options.max_steps),
                b: run_one_shot_text(claude, task, &start_render, &syntax),
                b2: Some(run_interactive_text(
                    claude,
                    task,
                    &start_render,
                    options.baseline_turns,
                )),
            })
        }
    }
}

fn percent(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 * 100.0 / whole as f64
    }
}

fn hit(reached: bool) -> &'static str {
    if reached { "yes" } else { "no" }
}

fn families_in_order(all: &[Task]) -> Vec<Family> {
    let mut families: Vec<Family> = Vec::new();
    for task in all {
        if !families.contains(&task.family) {
            families.push(task.family);
        }
    }
    families
}

fn arm_json(totals: &Totals) -> Json {
    Json::obj(vec![
        ("calls", Json::Int(totals.calls as i64)),
        ("edits", Json::Int(totals.edits as i64)),
        ("invalid", Json::Int(totals.invalid as i64)),
        ("invalid_rate", Json::Float(totals.rate())),
        ("parse_errors", Json::Int(totals.parse_errors as i64)),
        ("did_not_apply", Json::Int(totals.did_not_apply as i64)),
        ("ill_typed_steps", Json::Int(totals.ill_typed_steps as i64)),
        ("type_errors", Json::Int(totals.type_errors as i64)),
        ("reached_target", Json::Int(totals.reached as i64)),
        (
            "tasks_with_an_invalid_edit",
            Json::Int(totals.tasks_with_an_invalid_edit as i64),
        ),
        ("failed_calls", Json::Int(totals.failed_calls as i64)),
        ("retries", Json::Int(totals.retries as i64)),
    ])
}

fn run_header(claude: &Claude, options: &Options, task_count: usize) -> String {
    Json::obj(vec![
        ("record", Json::str("run")),
        ("model", Json::str(claude.model.clone())),
        (
            "task_set",
            Json::str(match options.task_set {
                TaskSet::Original => "original",
                TaskSet::PostB2 => "post-b2",
            }),
        ),
        (
            "mode",
            Json::str(match options.mode {
                Mode::OneShot => "one-shot",
                Mode::Interactive => "interactive",
            }),
        ),
        ("tasks", Json::Int(task_count as i64)),
        ("step_cap", Json::Int(options.max_steps as i64)),
        (
            "baseline_turn_cap",
            Json::Int(options.baseline_turns as i64),
        ),
        ("workers", Json::Int(options.workers as i64)),
        (
            "conditions",
            Json::str(match options.mode {
                Mode::OneShot => "A=action protocol (one shot), B=text baseline (one shot)",
                Mode::Interactive => {
                    "A=action protocol (interactive), B=text baseline (one shot), \
                     B2=text baseline (interactive)"
                }
            }),
        ),
    ])
    .to_string()
}

fn task_records(run: &TaskRun) -> Vec<String> {
    [Some(&run.a), Some(&run.b), run.b2.as_ref()]
        .into_iter()
        .flatten()
        .filter_map(|arm| arm.record.as_ref().map(Json::to_string))
        .collect()
}

fn write_partial_transcript(
    path: &std::path::Path,
    header: &str,
    finished: &[Option<TaskRun>],
) -> std::io::Result<()> {
    let mut lines = vec![header.to_string()];
    for run in finished.iter().flatten() {
        lines.extend(task_records(run));
    }
    std::fs::write(path, lines.join("\n") + "\n")
}

fn main() {
    let options = options();
    let claude = Claude::new();
    let started = std::time::Instant::now();
    let all: Vec<Task> = match options.task_set {
        TaskSet::Original => tasks(),
        TaskSet::PostB2 => post_b2_tasks(),
    }
    .into_iter()
    .filter(|t| options.only.as_deref().is_none_or(|name| t.name == name))
    .take(options.limit)
    .collect();

    if all.is_empty() {
        eprintln!("error: no tasks selected");
        std::process::exit(1);
    }

    if let Some(parent) = options.out.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("error: cannot create {}: {e}", parent.display());
        std::process::exit(1);
    }

    let header = run_header(&claude, &options, all.len());
    let next_task = Mutex::new(0usize);
    let runs: Mutex<Vec<Option<TaskRun>>> = Mutex::new((0..all.len()).map(|_| None).collect());
    let console = Mutex::new(());

    std::thread::scope(|scope| {
        for _ in 0..options.workers.min(all.len()) {
            scope.spawn(|| {
                loop {
                    let index = {
                        let mut cursor = next_task.lock().expect("the task cursor is not poisoned");
                        let index = *cursor;
                        *cursor += 1;
                        index
                    };
                    if index >= all.len() {
                        break;
                    }
                    let task = &all[index];
                    match run_task(&claude, task, &options) {
                        Err(message) => {
                            eprintln!("error: {}: {message}", task.name);
                            std::process::exit(1);
                        }
                        Ok(run) => {
                            {
                                let _quiet = console.lock().expect("the console is not poisoned");
                                println!(
                                    "{:<34} A: {:>3} calls {:>3} edits {:>2} invalid target {:<3}  B: target {:<3}  B2: target {}",
                                    task.name,
                                    run.a.calls,
                                    run.a.edits,
                                    run.a.invalid,
                                    hit(run.a.reached),
                                    hit(run.b.reached),
                                    run.b2.as_ref().map_or("-", |arm| hit(arm.reached)),
                                );
                            }
                            let mut finished =
                                runs.lock().expect("the results are not poisoned");
                            finished[index] = Some(run);
                            if let Err(e) =
                                write_partial_transcript(&options.out, &header, &finished)
                            {
                                eprintln!("warning: cannot write the partial transcript: {e}");
                            }
                        }
                    }
                }
            });
        }
    });

    let runs: Vec<TaskRun> = runs
        .into_inner()
        .expect("the results are not poisoned")
        .into_iter()
        .map(|run| run.expect("every task was run"))
        .collect();

    let mut a = Totals::default();
    let mut b = Totals::default();
    let mut b2 = Totals::default();
    for run in &runs {
        a.absorb(&run.a);
        b.absorb(&run.b);
        if let Some(arm) = &run.b2 {
            b2.absorb(arm);
        }
    }

    let mut lines: Vec<String> = vec![run_header(&claude, &options, all.len())];
    for run in &runs {
        lines.extend(task_records(run));
    }

    let mut summary = vec![
        ("record", Json::str("summary")),
        ("model", Json::str(claude.model.clone())),
        ("tasks", Json::Int(all.len() as i64)),
        (
            "elapsed_seconds",
            Json::Float(started.elapsed().as_secs_f64()),
        ),
    ];
    summary.push(("condition_a", arm_json(&a)));
    summary.push(("condition_b", arm_json(&b)));
    if options.mode == Mode::Interactive {
        summary.push(("condition_b2", arm_json(&b2)));
    }
    lines.push(Json::obj(summary).to_string());

    if let Err(e) = std::fs::write(&options.out, lines.join("\n") + "\n") {
        eprintln!("error: cannot write {}: {e}", options.out.display());
        std::process::exit(1);
    }

    println!();
    println!("model                     {}", claude.model);
    println!("tasks                     {}", all.len());
    println!(
        "elapsed                   {:.1} s",
        started.elapsed().as_secs_f64()
    );
    println!();
    println!("| | edits | invalid | invalid-edit rate | reached target | failed calls | retries |");
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    let interactive = options.mode == Mode::Interactive;
    let a_label = if interactive {
        "**A — action protocol (interactive)**"
    } else {
        "**A — action protocol**"
    };
    for (label, totals) in [
        (a_label, &a),
        ("**B — text baseline (one shot)**", &b),
        ("**B2 — text baseline (interactive)**", &b2),
    ]
    .into_iter()
    .take(if interactive { 3 } else { 2 })
    {
        println!(
            "| {} | {} | {} | **{:.1} %** | {} / {} | {} | {} |",
            label,
            totals.edits,
            totals.invalid,
            totals.rate() * 100.0,
            totals.reached,
            all.len(),
            totals.failed_calls,
            totals.retries
        );
    }

    println!();
    println!(
        "| | model calls | tasks with an invalid edit | parse_error | did_not_apply | type error | steps leaving an ill-typed program |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    for (label, totals) in [("A", &a), ("B", &b), ("B2", &b2)]
        .into_iter()
        .take(if interactive { 3 } else { 2 })
    {
        println!(
            "| {} | {} | {} / {} ({:.1} %) | {} | {} | {} | {} |",
            label,
            totals.calls,
            totals.tasks_with_an_invalid_edit,
            all.len(),
            percent(totals.tasks_with_an_invalid_edit, all.len()),
            totals.parse_errors,
            totals.did_not_apply,
            totals.type_errors,
            totals.ill_typed_steps
        );
    }

    println!();
    println!(
        "| family | tasks | A calls | A edits | A invalid | A reached | B reached | B2 reached |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for family in families_in_order(&all) {
        let members: Vec<&TaskRun> = all
            .iter()
            .zip(&runs)
            .filter(|(task, _)| task.family == family)
            .map(|(_, run)| run)
            .collect();
        let calls: usize = members.iter().map(|r| r.a.calls).sum();
        let edits: usize = members.iter().map(|r| r.a.edits).sum();
        let invalid: usize = members.iter().map(|r| r.a.invalid).sum();
        let a_reached = members.iter().filter(|r| r.a.reached).count();
        let b_reached = members.iter().filter(|r| r.b.reached).count();
        let b2_reached = members
            .iter()
            .filter(|r| r.b2.as_ref().is_some_and(|arm| arm.reached))
            .count();
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            family.label(),
            members.len(),
            calls,
            edits,
            invalid,
            a_reached,
            b_reached,
            if interactive {
                b2_reached.to_string()
            } else {
                "-".to_string()
            }
        );
    }

    println!();
    println!(
        "| task | family | A calls | A edits | A inv | A hit | B inv | B hit | B2 edits | B2 inv | B2 hit |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for (task, run) in all.iter().zip(&runs) {
        println!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            task.name,
            family_short(task.family),
            run.a.calls,
            run.a.edits,
            run.a.invalid,
            hit(run.a.reached),
            run.b.invalid,
            hit(run.b.reached),
            run.b2
                .as_ref()
                .map_or("-".to_string(), |arm| arm.edits.to_string()),
            run.b2
                .as_ref()
                .map_or("-".to_string(), |arm| arm.invalid.to_string()),
            run.b2.as_ref().map_or("-", |arm| hit(arm.reached)),
        );
    }

    println!();
    println!("transcript                {}", options.out.display());
}

fn family_short(family: Family) -> &'static str {
    match family {
        Family::FillHole => "fill",
        Family::BuildFunction => "build",
        Family::FixQuarantine => "fix",
        Family::ExtendProgram => "extend",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_done_sentinel_is_recognised_however_it_is_written() {
        for spelling in ["done", "DONE", "Done", "`done`", "\n\n done \n"] {
            assert!(first_meaningful_line(spelling).eq_ignore_ascii_case(DONE_SENTINEL));
        }
    }

    #[test]
    fn a_well_typed_reply_that_matches_the_target_scores_as_reached() {
        let (outcome, _, rendered, reached) = score_text_reply("λn:Num. n + 1", "λn:Num. n + 1");
        assert_eq!(outcome, "well_typed");
        assert_eq!(rendered, "λn:Num. n + 1");
        assert!(reached);
    }

    #[test]
    fn an_ill_typed_reply_is_invalid_and_a_quarantine_is_not() {
        let (outcome, _, _, _) = score_text_reply("1 + true", "");
        assert_eq!(outcome, "not_well_typed");
        let (outcome, _, _, _) = score_text_reply("1 + ⦇true⦈", "");
        assert_eq!(outcome, "well_typed");
        let (outcome, _, _, _) = score_text_reply("λn:Num", "");
        assert_eq!(outcome, "parse_error");
    }

    #[test]
    fn the_interactive_action_prompt_carries_the_whole_post_b2_grammar() {
        let task = Task {
            name: "probe",
            family: Family::FillHole,
            goal: "fill the hole",
            setup: "",
            target: "1",
        };
        let session = start(&task).expect("the empty setup replays");
        let prompt = interactive_action_prompt(&InteractiveTurn {
            task: &task,
            session: &session,
            history: &[],
            step: 1,
            cap: 30,
        });
        for fragment in [
            "construct-str",
            "construct-cons",
            "construct-fold",
            "construct-record",
            "construct-match",
            "expected type at cursor",
            "answer `done`",
        ] {
            assert!(prompt.contains(fragment), "the prompt lacks `{fragment}`");
        }
    }

    #[test]
    fn the_interactive_text_prompt_carries_the_post_b2_legend() {
        let task = Task {
            name: "probe",
            family: Family::FillHole,
            goal: "fill the hole",
            setup: "",
            target: "1",
        };
        let prompt = interactive_text_prompt(&task, "⦇⦈", &[], 1, 5);
        for fragment in ["::", "nil", "fold", "match", "`Some", "answer `done`"] {
            assert!(prompt.contains(fragment), "the prompt lacks `{fragment}`");
        }
    }

    #[test]
    fn the_original_arm_prompts_are_the_ones_the_first_table_was_measured_with() {
        let task = Task {
            name: "probe",
            family: Family::FillHole,
            goal: "fill the hole",
            setup: "",
            target: "1",
        };
        let prompt = one_shot_text_prompt(&task, "⦇⦈", ORIGINAL_SYNTAX);
        assert!(prompt.contains("no strings, no lists and no recursion"));
        let session = start(&task).expect("the empty setup replays");
        let actions = one_shot_action_prompt(&task, &session);
        assert!(actions.contains(ORIGINAL_ACTION_GRAMMAR));
        assert!(!ORIGINAL_ACTION_GRAMMAR.contains("construct-fold"));
    }
}
