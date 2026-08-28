use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use nothing_agentapi::json::{Json, parse};
use nothing_agentapi::measure::claude::{Claude, first_meaningful_line};

const HUMAN_AUTHOR: u64 = 1;
const MODEL_AUTHOR: u64 = 2;

struct Editor {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Editor {
    fn start(bin: &str) -> Result<Editor, String> {
        let mut child = Command::new(bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("cannot start `{bin}`: {e}"))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("no stdout")?);
        Ok(Editor {
            child,
            stdin,
            stdout,
        })
    }

    fn request(&mut self, value: &Json) -> Result<Json, String> {
        writeln!(self.stdin, "{value}").map_err(|e| format!("cannot write: {e}"))?;
        self.stdin
            .flush()
            .map_err(|e| format!("cannot flush: {e}"))?;
        let mut reply = String::new();
        let read = self
            .stdout
            .read_line(&mut reply)
            .map_err(|e| format!("cannot read: {e}"))?;
        if read == 0 {
            return Err("the editor closed its output".to_string());
        }
        parse(reply.trim()).map_err(|e| format!("bad reply `{reply}`: {e}"))
    }

    fn method(&mut self, method: &str) -> Result<Json, String> {
        self.request(&Json::obj(vec![("method", Json::str(method))]))
    }

    fn apply(&mut self, step: &str) -> Result<Json, String> {
        self.request(&Json::obj(vec![
            ("method", Json::str("apply")),
            (
                "params",
                Json::obj(vec![
                    ("step", Json::str(step)),
                    ("author", Json::Int(MODEL_AUTHOR as i64)),
                ]),
            ),
        ]))
    }

    fn script(&mut self, steps: &[String], author: u64) -> Result<Json, String> {
        self.request(&Json::obj(vec![
            ("method", Json::str("script")),
            (
                "params",
                Json::obj(vec![
                    (
                        "steps",
                        Json::arr(steps.iter().map(|s| Json::str(s.clone())).collect()),
                    ),
                    ("author", Json::Int(author as i64)),
                ]),
            ),
        ]))
    }

    fn finish(mut self) {
        let _ = self.method("quit");
        drop(self.stdin);
        let _ = self.child.wait();
    }
}

fn text(value: &Json, path: &[&str]) -> String {
    let mut cursor = value;
    for key in path {
        match cursor.get(key) {
            Some(next) => cursor = next,
            None => return String::new(),
        }
    }
    cursor.as_str().unwrap_or_default().to_string()
}

fn list(value: &Json, key: &str) -> Vec<Json> {
    value
        .get(key)
        .and_then(Json::as_arr)
        .map(<[Json]>::to_vec)
        .unwrap_or_default()
}

fn context_block(context: &Json) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "cursor is on: {} (a {})\n",
        text(context, &["focus_render"]),
        text(context, &["focus_kind"])
    ));
    out.push_str(&format!(
        "expected type at the cursor: {}\n",
        text(context, &["expected_ty_text"])
    ));

    let bindings = list(context, "bindings");
    if bindings.is_empty() {
        out.push_str("names in scope: none\n");
    } else {
        out.push_str("names in scope:\n");
        for binding in &bindings {
            out.push_str(&format!(
                "  {} : {}{}\n",
                text(binding, &["name"]),
                text(binding, &["ty_text"]),
                if binding
                    .get("consistent_with_expected")
                    .and_then(Json::as_bool)
                    .unwrap_or(false)
                {
                    "  (fits here)"
                } else {
                    ""
                }
            ));
        }
    }

    out.push_str("actions that are well typed at the cursor right now:\n");
    for construction in list(context, "constructions") {
        let label = match construction.get("template").and_then(Json::as_str) {
            Some(template) => template.to_string(),
            None => text(&construction, &["step"]),
        };
        if label.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "  {label:<26} gives   {}\n",
            text(&construction, &["produces"])
        ));
    }

    let movements: Vec<String> = list(context, "movements")
        .iter()
        .filter_map(|m| m.as_str().map(str::to_string))
        .collect();
    if !movements.is_empty() {
        out.push_str(&format!("movement actions: {}\n", movements.join(", ")));
    }
    let other: Vec<String> = list(context, "other_actions")
        .iter()
        .filter_map(|m| m.as_str().map(str::to_string))
        .collect();
    if !other.is_empty() {
        out.push_str(&format!("other actions: {}\n", other.join(", ")));
    }
    out
}

const RULES: &str = "\
How this editor works. There is no text and no parser: you cannot type a program.
You change the program only by naming one action at a time, and every action either
applies and leaves a well-typed program, or is refused. The cursor is shown between
» and « in the rendering.

Rules for the constructions:
  - at an empty hole ⦇⦈, a construction fills the hole
  - on a non-hole expression e, `construct-binop OP` makes `e OP ⦇⦈`,
    `construct-ap` makes `e ⦇⦈`, `construct-if` makes `if e then ⦇⦈ else ⦇⦈`,
    `construct-let` makes `let x = e in ⦇⦈`, `construct-pair` makes `(e, ⦇⦈)`,
    `construct-lam` makes `λx:?. e`, `construct-proj SIDE` makes `fst e` / `snd e`
  - after a construction the cursor lands on the first new empty hole, if there is one
  - `move-child N` descends to child N (0-based, in source order),
    `move-parent` ascends, `move-next-sibling` / `move-prev-sibling` walk siblings
  - `delete` replaces whatever is under the cursor with an empty hole
  - `rename NAME` renames the binder under the cursor (cursor must be on the λ or the let)
  - `set-ann TYPE` sets the parameter type of the λ under the cursor

Answer with exactly one action, on one line, with no explanation, no quotes and no
backticks. If the program already matches the goal, answer `done`.";

fn prompt(goal: &str, target: &str, state: &Json, context: &Json, history: &[String]) -> String {
    let mut out = String::new();
    out.push_str("You are editing a program in a structural editor.\n\n");
    out.push_str(&format!("Goal: {goal}\n"));
    out.push_str(&format!(
        "The finished program must render exactly as:\n  {target}\n\n"
    ));
    out.push_str(&format!(
        "Current program (cursor between » and «):\n  {}\n\n",
        text(state, &["render_with_cursor"])
    ));
    out.push_str(&context_block(context));
    if !history.is_empty() {
        out.push_str("\nWhat you have done so far, most recent last:\n");
        for line in history.iter().rev().take(8).rev() {
            out.push_str(&format!("  {line}\n"));
        }
    }
    out.push('\n');
    out.push_str(RULES);
    out.push('\n');
    out
}

struct Options {
    goal: String,
    target: String,
    max_steps: usize,
    transcript: PathBuf,
    editor_bin: String,
    setup: Vec<String>,
}

fn default_editor_bin() -> String {
    if let Ok(path) = std::env::var("NOTHING_PROTOCOL_BIN") {
        return path;
    }
    match std::env::current_exe() {
        Ok(exe) => exe
            .parent()
            .map(|dir| dir.join("protocol"))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "protocol".to_string()),
        Err(_) => "protocol".to_string(),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn options() -> Options {
    let args: Vec<String> = std::env::args().collect();
    let mut options = Options {
        goal: "Build the factorial reference program. The recursive call cannot be written \
               in this language yet, so the last operand stays an empty hole."
            .to_string(),
        target: "λx0:Num. if x0 == 0 then 1 else x0 * ⦇⦈".to_string(),
        max_steps: 40,
        transcript: repo_root().join("bench/agent-transcripts/factorial.jsonl"),
        editor_bin: default_editor_bin(),
        setup: Vec::new(),
    };
    let mut i = 1;
    while i < args.len() {
        let next = args.get(i + 1).cloned();
        match args[i].as_str() {
            "--goal" => {
                if let Some(v) = next {
                    options.goal = v;
                }
                i += 2;
            }
            "--target" => {
                if let Some(v) = next {
                    options.target = v;
                }
                i += 2;
            }
            "--max-steps" => {
                if let Some(v) = next.and_then(|v| v.parse().ok()) {
                    options.max_steps = v;
                }
                i += 2;
            }
            "--transcript" => {
                if let Some(v) = next {
                    options.transcript = PathBuf::from(v);
                }
                i += 2;
            }
            "--editor" => {
                if let Some(v) = next {
                    options.editor_bin = v;
                }
                i += 2;
            }
            "--setup" => {
                if let Some(v) = next {
                    options.setup = v
                        .split(';')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect();
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

    let mut editor = match Editor::start(&options.editor_bin) {
        Ok(editor) => editor,
        Err(message) => {
            eprintln!("error: {message}");
            std::process::exit(1);
        }
    };

    if let Some(parent) = options.transcript.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("error: cannot create {}: {e}", parent.display());
        std::process::exit(1);
    }

    let mut lines: Vec<String> = Vec::new();
    lines.push(
        Json::obj(vec![
            ("record", Json::str("run")),
            ("model", Json::str(claude.model.clone())),
            ("goal", Json::str(options.goal.clone())),
            ("target", Json::str(options.target.clone())),
            ("max_steps", Json::Int(options.max_steps as i64)),
            ("human_author", Json::Int(HUMAN_AUTHOR as i64)),
            ("model_author", Json::Int(MODEL_AUTHOR as i64)),
            (
                "setup",
                Json::arr(options.setup.iter().map(|s| Json::str(s.clone())).collect()),
            ),
        ])
        .to_string(),
    );

    if !options.setup.is_empty() {
        match editor.script(&options.setup, HUMAN_AUTHOR) {
            Ok(reply) if reply.get("applied").and_then(Json::as_bool) == Some(true) => {
                println!(
                    "  0  (human-authored base)      {}",
                    text(&reply, &["state", "render_with_cursor"])
                );
            }
            Ok(reply) => {
                eprintln!("error: the human-authored setup did not apply: {reply}");
                std::process::exit(1);
            }
            Err(message) => {
                eprintln!("error: {message}");
                std::process::exit(1);
            }
        }
    }

    let mut history: Vec<String> = Vec::new();
    let mut applied_count = 0usize;
    let mut rejected_count = 0usize;
    let mut retries = 0usize;
    let mut reached = false;
    let mut steps_used = 0usize;

    for step in 1..=options.max_steps {
        steps_used = step;
        let state = match editor.method("state") {
            Ok(value) => value,
            Err(message) => {
                eprintln!("error: {message}");
                break;
            }
        };
        let render = text(&state, &["state", "render"]);
        if render == options.target {
            reached = true;
            steps_used = step - 1;
            break;
        }
        let context_reply = match editor.method("hole_context") {
            Ok(value) => value,
            Err(message) => {
                eprintln!("error: {message}");
                break;
            }
        };
        let empty = Json::Obj(Vec::new());
        let context = context_reply.get("hole_context").unwrap_or(&empty);
        let state_value = state.get("state").unwrap_or(&empty);

        let text_prompt = prompt(
            &options.goal,
            &options.target,
            state_value,
            context,
            &history,
        );

        let reply = match claude.ask(&text_prompt) {
            Ok(reply) => {
                if reply.attempts > 1 {
                    retries += reply.attempts - 1;
                }
                reply
            }
            Err(message) => {
                eprintln!("error: the model call failed: {message}");
                lines.push(
                    Json::obj(vec![
                        ("record", Json::str("step")),
                        ("step", Json::Int(step as i64)),
                        ("error", Json::str(message)),
                    ])
                    .to_string(),
                );
                break;
            }
        };

        let action = first_meaningful_line(&reply.text);
        if action.eq_ignore_ascii_case("done") {
            lines.push(
                Json::obj(vec![
                    ("record", Json::str("step")),
                    ("step", Json::Int(step as i64)),
                    ("prompt", Json::str(text_prompt)),
                    ("reply", Json::str(reply.text.clone())),
                    ("action", Json::str("done")),
                    ("applied", Json::Bool(false)),
                    ("render", Json::str(render.clone())),
                ])
                .to_string(),
            );
            break;
        }

        let outcome = match editor.apply(&action) {
            Ok(value) => value,
            Err(message) => {
                eprintln!("error: {message}");
                break;
            }
        };
        let applied = outcome
            .get("applied")
            .and_then(Json::as_bool)
            .unwrap_or(false);
        let error = outcome
            .get("error")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_string();
        let after = text(&outcome, &["state", "render_with_cursor"]);
        if applied {
            applied_count += 1;
            history.push(format!("{action}   ->   {after}"));
        } else {
            rejected_count += 1;
            history.push(format!("{action}   ->   REFUSED ({error})"));
        }

        println!(
            "{step:>3}  {action:<28} {}",
            if applied { &after } else { "refused" }
        );

        lines.push(
            Json::obj(vec![
                ("record", Json::str("step")),
                ("step", Json::Int(step as i64)),
                ("prompt", Json::str(text_prompt)),
                ("reply", Json::str(reply.text)),
                ("attempts", Json::Int(reply.attempts as i64)),
                ("action", Json::str(action)),
                ("applied", Json::Bool(applied)),
                ("error", Json::str(error)),
                ("render", Json::str(text(&outcome, &["state", "render"]))),
                ("render_with_cursor", Json::str(after)),
            ])
            .to_string(),
        );
    }

    let final_state = editor.method("state").ok();
    let final_render = final_state
        .as_ref()
        .map(|s| text(s, &["state", "render"]))
        .unwrap_or_default();
    if final_render == options.target {
        reached = true;
    }
    let log = editor.method("log").ok();
    let provenance = editor.method("provenance").ok();
    let annotated = editor
        .request(&Json::obj(vec![
            ("method", Json::str("annotate")),
            (
                "params",
                Json::obj(vec![
                    ("agents", Json::arr(vec![Json::Int(MODEL_AUTHOR as i64)])),
                    ("style", Json::str("brackets")),
                ]),
            ),
        ]))
        .ok();

    lines.push(
        Json::obj(vec![
            ("record", Json::str("summary")),
            ("reached_target", Json::Bool(reached)),
            ("steps", Json::Int(steps_used as i64)),
            ("actions_applied", Json::Int(applied_count as i64)),
            ("actions_refused", Json::Int(rejected_count as i64)),
            ("model_call_retries", Json::Int(retries as i64)),
            ("final_render", Json::str(final_render.clone())),
            (
                "annotated_render",
                Json::str(
                    annotated
                        .as_ref()
                        .map(|a| text(a, &["annotated"]))
                        .unwrap_or_default(),
                ),
            ),
            (
                "log",
                log.and_then(|l| l.get("log").cloned())
                    .unwrap_or(Json::Null),
            ),
            (
                "provenance",
                provenance
                    .and_then(|p| p.get("provenance").cloned())
                    .unwrap_or(Json::Null),
            ),
        ])
        .to_string(),
    );

    editor.finish();

    let body = lines.join("\n") + "\n";
    if let Err(e) = std::fs::write(&options.transcript, body) {
        eprintln!("error: cannot write {}: {e}", options.transcript.display());
        std::process::exit(1);
    }

    println!();
    println!("model            {}", claude.model);
    println!("target           {}", options.target);
    println!("final            {final_render}");
    println!("reached target   {reached}");
    println!("steps            {steps_used}");
    println!("actions applied  {applied_count}");
    println!("actions refused  {rejected_count}");
    println!("model retries    {retries}");
    println!("transcript       {}", options.transcript.display());

    if !reached {
        std::process::exit(2);
    }
}
