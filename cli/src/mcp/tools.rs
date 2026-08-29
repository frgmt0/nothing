use nothing_agentapi::holectx::hole_context;
use nothing_agentapi::json::Json;
use nothing_agentapi::protocol::handle;
use nothing_agentapi::session::AgentSession;
use nothing_eval::Recorded;

use crate::check::check_document;
use crate::run_cmd::perform_or_evaluate;

pub struct ToolOutcome {
    text: String,
    structured: Option<Json>,
    is_error: bool,
}

impl ToolOutcome {
    fn reported(text: String, structured: Json) -> ToolOutcome {
        ToolOutcome {
            text,
            structured: Some(structured),
            is_error: false,
        }
    }

    fn refused(text: String, structured: Json) -> ToolOutcome {
        ToolOutcome {
            text,
            structured: Some(structured),
            is_error: true,
        }
    }

    fn rejected(text: String) -> ToolOutcome {
        ToolOutcome {
            text,
            structured: None,
            is_error: true,
        }
    }

    pub fn into_result(self) -> Json {
        let content = Json::arr(vec![Json::obj(vec![
            ("type", Json::str("text")),
            ("text", Json::str(self.text)),
        ])]);
        let mut fields = vec![
            ("content".to_string(), content),
            ("isError".to_string(), Json::Bool(self.is_error)),
        ];
        if let Some(structured) = self.structured {
            fields.push(("structuredContent".to_string(), structured));
        }
        Json::Obj(fields)
    }
}

const EDITING_MODEL: &str = "There is no parser and no source text: you edit by naming actions, and \
every action either applies — leaving a program that is still well typed — or is refused, changing \
nothing. Call `hole_context` before choosing an action; it lists exactly the constructions that are \
well typed at the cursor.";

fn object_schema(properties: Vec<(&str, Json)>, required: Vec<&str>) -> Json {
    Json::obj(vec![
        ("type", Json::str("object")),
        ("properties", Json::obj(properties)),
        (
            "required",
            Json::arr(required.into_iter().map(Json::str).collect()),
        ),
    ])
}

fn tool(
    name: &str,
    description: String,
    properties: Vec<(&str, Json)>,
    required: Vec<&str>,
) -> Json {
    Json::obj(vec![
        ("name", Json::str(name)),
        ("description", Json::str(description)),
        ("inputSchema", object_schema(properties, required)),
    ])
}

fn text_argument(description: &str) -> Json {
    Json::obj(vec![
        ("type", Json::str("string")),
        ("description", Json::str(description)),
    ])
}

fn integer_argument(description: &str) -> Json {
    Json::obj(vec![
        ("type", Json::str("integer")),
        ("description", Json::str(description)),
    ])
}

fn flag_argument(description: &str) -> Json {
    Json::obj(vec![
        ("type", Json::str("boolean")),
        ("description", Json::str(description)),
    ])
}

fn object_argument(description: &str) -> Json {
    Json::obj(vec![
        ("type", Json::str("object")),
        ("description", Json::str(description)),
    ])
}

fn choice_argument(description: &str, choices: &[&str]) -> Json {
    Json::obj(vec![
        ("type", Json::str("string")),
        ("description", Json::str(description)),
        (
            "enum",
            Json::arr(choices.iter().map(|choice| Json::str(*choice)).collect()),
        ),
    ])
}

fn array_argument(description: &str, items: Json) -> Json {
    Json::obj(vec![
        ("type", Json::str("array")),
        ("description", Json::str(description)),
        ("items", items),
    ])
}

fn step_or_action_item() -> Json {
    Json::obj(vec![
        (
            "description",
            Json::str(
                "either a step string such as `construct-binop mul`, or a structured action \
                 object such as {\"action\":\"ConstructNum\",\"value\":42}",
            ),
        ),
        (
            "anyOf",
            Json::arr(vec![
                Json::obj(vec![("type", Json::str("string"))]),
                Json::obj(vec![("type", Json::str("object"))]),
            ]),
        ),
    ])
}

pub fn listing() -> Json {
    Json::obj(vec![("tools", Json::arr(catalogue()))])
}

fn catalogue() -> Vec<Json> {
    vec![
        tool(
            "get_state",
            format!(
                "Report the whole editor state: the rendered document, the definition and cursor \
                 position you are editing at, every definition with its type annotation, whether \
                 the document is well typed, and how many empty and non-empty holes are left. \
                 Start here, and call it again whenever you have lost track of where the cursor \
                 is. {EDITING_MODEL}"
            ),
            vec![],
            vec![],
        ),
        tool(
            "get_projection",
            "Render the program for reading. `document` (the default) prints every definition, one \
             per line, with its name and type. `definition` prints only the definition the cursor \
             is in. `cursor` prints that definition with the focus marked »like this«. `annotated` \
             prints the document with the spans written by the given author ids bracketed, which \
             is how you see what you wrote versus what a human wrote."
                .to_string(),
            vec![
                (
                    "projection",
                    choice_argument(
                        "which rendering to return; defaults to `document`",
                        &["document", "definition", "cursor", "annotated"],
                    ),
                ),
                (
                    "agents",
                    array_argument(
                        "for the `annotated` projection: the author ids to mark as agent-written",
                        integer_argument("an author id"),
                    ),
                ),
            ],
            vec![],
        ),
        tool(
            "hole_context",
            format!(
                "The single most useful query: what is expected at the cursor, and what can be \
                 written there. Returns the expected type, every binding in scope with its type \
                 and whether that type fits, and the constructions that are well typed here — \
                 each one checked by actually applying it, so an offered construction is \
                 guaranteed to apply and to leave no non-empty hole. Also lists the movements and \
                 the other actions that apply here. {EDITING_MODEL}"
            ),
            vec![],
            vec![],
        ),
        tool(
            "apply_action",
            format!(
                "Apply exactly one action at the cursor. Name it textually with `step` — \
                 `construct-lam`, `construct-num 42`, `construct-var xs`, `set-ann Num -> Bool`, \
                 `move-child 0`, `rename-def helper` — or structurally with `action`, which is the \
                 only way to name a binder shadowed by a nearer one of the same display name. Call \
                 `action_grammar` for every spelling. The action either applies, and the reply \
                 shows the new program, or it is refused and nothing changes. {EDITING_MODEL}"
            ),
            vec![
                (
                    "step",
                    text_argument(
                        "the action as a step string, for example `construct-binop mul`",
                    ),
                ),
                (
                    "action",
                    object_argument(
                        "the action in structured form, for example \
                         {\"action\":\"ConstructVar\",\"id\":\"…uuid…\"}",
                    ),
                ),
                (
                    "author",
                    integer_argument(
                        "attribute this action to this author id instead of the session default",
                    ),
                ),
            ],
            vec![],
        ),
        tool(
            "apply_actions",
            "Apply a sequence of actions in order, stopping at the first one that does not apply. \
             This is the fast way to build a definition: send the whole script and read back the \
             finished program. The reply says, per step, whether it applied, so a script that \
             stops early tells you exactly which step the calculus refused and what the program \
             looked like when it did. Steps already applied are kept; they are not rolled back."
                .to_string(),
            vec![
                (
                    "steps",
                    array_argument(
                        "the actions to apply, in order",
                        step_or_action_item(),
                    ),
                ),
                (
                    "author",
                    integer_argument(
                        "attribute these actions to this author id instead of the session default",
                    ),
                ),
            ],
            vec!["steps"],
        ),
        tool(
            "save_document",
            "Write the current document to a file in the binary `NTHG` format that `nothing edit`, \
             `nothing run` and `nothing check` read. The action log is written with it, so \
             provenance and undo survive the round trip. There is no text format to write instead."
                .to_string(),
            vec![("path", text_argument("where to write the document"))],
            vec!["path"],
        ),
        tool(
            "load_document",
            "Read a document from a file and adopt it as the session's program, replacing whatever \
             was being edited. The cursor lands in the first definition. Use this to continue work \
             on a document saved earlier, or one built in the TUI editor."
                .to_string(),
            vec![("path", text_argument("the document to read"))],
            vec!["path"],
        ),
        tool(
            "typecheck",
            "Report whether the document is well typed, whether it is complete (no holes left), \
             and the empty and non-empty hole counts for each definition by name. A `nothing` \
             program is well typed at every instant by construction, so the interesting answer \
             here is usually the hole count: it tells you how much of the program is still \
             unwritten and which definition to go and fill in."
                .to_string(),
            vec![],
            vec![],
        ),
        tool(
            "run",
            "Evaluate the definition named `main` and report the outcome. If `main` has a command \
             type it is performed instead: `print` writes a line, `readline` reads one from the \
             `stdin_lines` argument, `bind` sequences. What the program printed comes back inside \
             this tool result, so a run never disturbs the protocol stream. A program with a hole \
             on the path to its answer reports an indeterminate result and the holes it is blocked \
             on rather than failing."
                .to_string(),
            vec![
                (
                    "fuel",
                    integer_argument(
                        "the execution budget in steps; defaults to 200000",
                    ),
                ),
                (
                    "stdin_lines",
                    array_argument(
                        "the lines `readline` should return, in order; it returns nothing once \
                         they run out",
                        text_argument("one line of input"),
                    ),
                ),
            ],
            vec![],
        ),
        tool(
            "stdlib",
            "List the standard library: every name in scope that the document did not define, with \
             its type and its doc line. These are callable with `construct-var NAME` exactly like \
             the document's own definitions, and they are never written into a saved document."
                .to_string(),
            vec![(
                "filter",
                text_argument("only list entries whose name or doc line contains this text"),
            )],
            vec![],
        ),
        tool(
            "action_grammar",
            "The complete grammar of action names accepted by `apply_action` and `apply_actions`: \
             every movement, every construction, every definition-level edit, and the type syntax \
             `set-ann` and `set-def-ann` take. Read this once before your first edit."
                .to_string(),
            vec![],
            vec![],
        ),
        tool(
            "undo",
            "Undo the last applied action by truncating the action log and replaying it. The \
             program returns to exactly the state before that action; there is no partial undo, \
             because there was no partial edit."
                .to_string(),
            vec![],
            vec![],
        ),
        tool(
            "redo",
            "Re-apply the action that `undo` removed. Applying any new action after an undo \
             discards what could have been redone."
                .to_string(),
            vec![],
            vec![],
        ),
        tool(
            "reset",
            "Throw the document away and start again from a single empty definition with an empty \
             action log. The standard library stays in scope."
                .to_string(),
            vec![],
            vec![],
        ),
        tool(
            "move_to_hole",
            "Move the cursor to the next hole in this definition, wrapping around at the end, so \
             you can fill a program in without counting `move-parent` and `move-child` steps \
             yourself. Set `forward` to false to walk backwards."
                .to_string(),
            vec![(
                "forward",
                flag_argument("walk forwards to the next hole; defaults to true"),
            )],
            vec![],
        ),
    ]
}

fn text_of<'a>(value: &'a Json, key: &str) -> &'a str {
    value.get(key).and_then(Json::as_str).unwrap_or("")
}

fn number_of(value: &Json, key: &str) -> i64 {
    value.get(key).and_then(Json::as_i64).unwrap_or(0)
}

fn flag_of(value: &Json, key: &str) -> bool {
    value.get(key).and_then(Json::as_bool).unwrap_or(false)
}

fn cloned(value: &Json, key: &str) -> Json {
    value.get(key).cloned().unwrap_or(Json::Null)
}

fn digest(state: &Json) -> Json {
    Json::obj(vec![
        ("render_document", cloned(state, "render_document")),
        ("render", cloned(state, "render")),
        ("render_with_cursor", cloned(state, "render_with_cursor")),
        ("definition_name", cloned(state, "definition_name")),
        ("definition_index", cloned(state, "definition_index")),
        ("definition_count", cloned(state, "definition_count")),
        ("cursor_path", cloned(state, "cursor_path")),
        ("focus_kind", cloned(state, "focus_kind")),
        ("expected_ty_text", cloned(state, "expected_ty_text")),
        ("well_typed", cloned(state, "well_typed")),
        ("empty_holes", cloned(state, "empty_holes")),
        ("non_empty_holes", cloned(state, "non_empty_holes")),
        (
            "document_empty_holes",
            cloned(state, "document_empty_holes"),
        ),
        (
            "document_non_empty_holes",
            cloned(state, "document_non_empty_holes"),
        ),
        ("complete", cloned(state, "complete")),
        ("can_undo", cloned(state, "can_undo")),
        ("can_redo", cloned(state, "can_redo")),
        ("log_len", cloned(state, "log_len")),
    ])
}

fn without_type_trees(value: &Json, drop: &[&str]) -> Json {
    match value {
        Json::Obj(fields) => Json::Obj(
            fields
                .iter()
                .filter(|(key, _)| !drop.contains(&key.as_str()))
                .map(|(key, nested)| (key.clone(), without_type_trees(nested, drop)))
                .collect(),
        ),
        Json::Arr(items) => Json::Arr(
            items
                .iter()
                .map(|item| without_type_trees(item, drop))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn compact_hole_context(context: &Json) -> Json {
    without_type_trees(context, &["ty", "expected_ty", "definition_ann"])
}

fn compact_definitions(state: &Json) -> Json {
    without_type_trees(&cloned(state, "definitions"), &["ann"])
}

fn state_summary(state: &Json) -> String {
    let mut out = String::new();
    out.push_str("the document now reads:\n");
    for line in text_of(state, "render_document").lines() {
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }
    let definition = text_of(state, "definition_name");
    let cursor = text_of(state, "render_with_cursor");
    out.push_str(&format!(
        "\nthe cursor sits in `{definition}`, marked »…«:\n  {cursor}\n"
    ));
    let expected = text_of(state, "expected_ty_text");
    let focus = text_of(state, "focus_kind");
    out.push_str(&format!(
        "\nexpected type at the cursor: {expected}\nthe kind of node under the cursor: {focus}\n"
    ));
    let well_typed = flag_of(state, "well_typed");
    let empty = number_of(state, "document_empty_holes");
    let non_empty = number_of(state, "document_non_empty_holes");
    let definitions = number_of(state, "definition_count");
    out.push_str(&format!(
        "well-typed: {well_typed}; {definitions} definition(s) holding {empty} empty and \
         {non_empty} non-empty hole(s)\n"
    ));
    out
}

fn state_of(reply: &Json) -> Json {
    reply
        .get("state")
        .cloned()
        .unwrap_or_else(|| Json::Obj(Vec::new()))
}

fn through_protocol(session: &mut AgentSession, method: &str, params: Json) -> Json {
    let request = Json::obj(vec![("method", Json::str(method)), ("params", params)]);
    handle(session, &request).value
}

fn state_json(session: &mut AgentSession) -> Json {
    state_of(&through_protocol(session, "state", Json::Obj(Vec::new())))
}

fn outcome_from(reply: &Json, headline: String) -> ToolOutcome {
    let ok = flag_of(reply, "ok");
    let state = state_of(reply);
    let mut text = headline;
    text.push('\n');
    if let Some(error) = reply.get("error").and_then(Json::as_str) {
        text.push('\n');
        text.push_str(error);
        text.push('\n');
    }
    text.push('\n');
    text.push_str(&state_summary(&state));
    let structured = Json::obj(vec![
        ("ok", Json::Bool(ok)),
        ("applied", Json::Bool(flag_of(reply, "applied"))),
        ("state", digest(&state)),
    ]);
    if ok {
        ToolOutcome::reported(text, structured)
    } else {
        ToolOutcome::refused(text, structured)
    }
}

pub fn call(session: &mut AgentSession, name: &str, arguments: &Json) -> ToolOutcome {
    match name {
        "get_state" => get_state(session),
        "get_projection" => get_projection(session, arguments),
        "hole_context" => hole_context_tool(session),
        "apply_action" => apply_action(session, arguments),
        "apply_actions" => apply_actions(session, arguments),
        "save_document" => save_document(session, arguments),
        "load_document" => load_document(session, arguments),
        "typecheck" => typecheck(session),
        "run" => run_main(session, arguments),
        "stdlib" => stdlib(session, arguments),
        "action_grammar" => action_grammar(session),
        "undo" => undo(session),
        "redo" => redo(session),
        "reset" => reset(session),
        "move_to_hole" => move_to_hole(session, arguments),
        other => {
            let names: Vec<String> = catalogue()
                .iter()
                .map(|entry| text_of(entry, "name").to_string())
                .collect();
            let known = names.join(", ");
            ToolOutcome::rejected(format!(
                "there is no tool named `{other}`. This server offers: {known}."
            ))
        }
    }
}

fn get_state(session: &mut AgentSession) -> ToolOutcome {
    let reply = through_protocol(session, "state", Json::Obj(Vec::new()));
    let state = state_of(&reply);
    let mut text = state_summary(&state);
    text.push_str("\nthe definitions are:\n");
    for definition in state
        .get("definitions")
        .and_then(Json::as_arr)
        .unwrap_or_default()
    {
        let name = text_of(definition, "name");
        let ann = text_of(definition, "ann_text");
        let here = if flag_of(definition, "current") {
            "   <- the cursor is here"
        } else {
            ""
        };
        text.push_str(&format!("  {name} : {ann}{here}\n"));
    }
    let structured = Json::obj(vec![
        ("state", digest(&state)),
        ("definitions", compact_definitions(&state)),
        ("stdlib_count", cloned(&state, "stdlib_count")),
    ]);
    ToolOutcome::reported(text, structured)
}

fn get_projection(session: &mut AgentSession, arguments: &Json) -> ToolOutcome {
    let projection = arguments
        .get("projection")
        .and_then(Json::as_str)
        .unwrap_or("document");
    if projection == "annotated" {
        let agents = arguments
            .get("agents")
            .cloned()
            .unwrap_or_else(|| Json::Arr(Vec::new()));
        let params = Json::obj(vec![("agents", agents), ("style", Json::str("brackets"))]);
        let reply = through_protocol(session, "annotate", params);
        let rendered = text_of(&reply, "annotated_document").to_string();
        let text = format!(
            "the document, with spans written by the named authors in ⟦…⟧ and human-written \
             spans nested inside them in ⟨…⟩:\n\n{rendered}\n"
        );
        let structured = Json::obj(vec![
            ("projection", Json::str("annotated")),
            ("rendered", Json::str(rendered)),
            ("state", digest(&state_of(&reply))),
        ]);
        return ToolOutcome::reported(text, structured);
    }

    let state = state_json(session);
    let (key, caption) = match projection {
        "definition" => ("render", "the definition the cursor is in"),
        "cursor" => (
            "render_with_cursor",
            "the definition the cursor is in, with the focus marked »…«",
        ),
        "document" => (
            "render_document",
            "the whole document, one definition a line",
        ),
        other => {
            return ToolOutcome::rejected(format!(
                "there is no projection named `{other}`. Ask for `document`, `definition`, \
                 `cursor` or `annotated`."
            ));
        }
    };
    let rendered = text_of(&state, key).to_string();
    let text = format!("{caption}:\n\n{rendered}\n");
    let structured = Json::obj(vec![
        ("projection", Json::str(projection)),
        ("rendered", Json::str(rendered)),
        ("state", digest(&state)),
    ]);
    ToolOutcome::reported(text, structured)
}

fn hole_context_tool(session: &mut AgentSession) -> ToolOutcome {
    let context = hole_context(session.state());
    let block = context.to_prompt_block();
    let structured_context = compact_hole_context(&context.to_json());
    let state = state_json(session);
    let text = format!(
        "{block}\nEvery construction listed above applies at this cursor and leaves no non-empty \
         hole; anything not listed either does not apply or would not be well typed here. The \
         bindings marked `(definition)` include the whole standard library — call `stdlib` with a \
         `filter` to search it.\n"
    );
    let structured = Json::obj(vec![
        ("hole_context", structured_context),
        ("state", digest(&state)),
    ]);
    ToolOutcome::reported(text, structured)
}

fn apply_action(session: &mut AgentSession, arguments: &Json) -> ToolOutcome {
    let mut params: Vec<(String, Json)> = Vec::new();
    if let Some(step) = arguments.get("step") {
        params.push(("step".to_string(), step.clone()));
    }
    if let Some(action) = arguments.get("action") {
        params.push(("action".to_string(), action.clone()));
    }
    if let Some(author) = arguments.get("author") {
        params.push(("author".to_string(), author.clone()));
    }
    if params.is_empty() {
        return ToolOutcome::rejected(
            "`apply_action` needs either a `step` string such as `construct-num 42` or a \
             structured `action` object. Call `action_grammar` for the step spellings."
                .to_string(),
        );
    }
    let label = arguments
        .get("step")
        .and_then(Json::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| "the structured action".to_string());
    let reply = through_protocol(session, "apply", Json::Obj(params));
    let headline = if flag_of(&reply, "applied") {
        format!("`{label}` applied, and the program is still well typed.")
    } else {
        format!("`{label}` was refused; the program is unchanged.")
    };
    outcome_from(&reply, headline)
}

fn apply_actions(session: &mut AgentSession, arguments: &Json) -> ToolOutcome {
    let Some(steps) = arguments.get("steps").and_then(Json::as_arr) else {
        return ToolOutcome::rejected(
            "`apply_actions` needs a `steps` array of step strings or structured action objects."
                .to_string(),
        );
    };
    let labels: Vec<String> = steps
        .iter()
        .map(|step| match step.as_str() {
            Some(text) => text.to_string(),
            None => step.to_string(),
        })
        .collect();
    let mut params: Vec<(String, Json)> = vec![("steps".to_string(), Json::Arr(steps.to_vec()))];
    if let Some(author) = arguments.get("author") {
        params.push(("author".to_string(), author.clone()));
    }
    let reply = through_protocol(session, "script", Json::Obj(params));

    let results = reply
        .get("steps")
        .and_then(Json::as_arr)
        .map(<[Json]>::to_vec)
        .unwrap_or_default();
    let applied = results
        .iter()
        .filter(|entry| flag_of(entry, "applied"))
        .count();
    let total = labels.len();
    let mut headline = format!("{applied} of {total} action(s) applied.");
    for entry in &results {
        let index = number_of(entry, "index") as usize;
        let label = labels.get(index).cloned().unwrap_or_default();
        if flag_of(entry, "applied") {
            headline.push_str(&format!("\n  {index}: {label} — applied"));
        } else {
            let why = entry
                .get("error")
                .and_then(Json::as_str)
                .unwrap_or("the action does not apply at this cursor");
            headline.push_str(&format!("\n  {index}: {label} — REFUSED: {why}"));
            headline.push_str("\n  the script stopped here; later actions were not attempted");
        }
    }
    outcome_from(&reply, headline)
}

fn save_document(session: &mut AgentSession, arguments: &Json) -> ToolOutcome {
    let Some(path) = arguments.get("path").and_then(Json::as_str) else {
        return ToolOutcome::rejected("`save_document` needs a `path` string.".to_string());
    };
    let params = Json::obj(vec![("path", Json::str(path))]);
    let reply = through_protocol(session, "save", params);
    let headline = if flag_of(&reply, "ok") {
        let bytes = number_of(&reply, "bytes");
        format!("wrote {bytes} bytes to {path}.")
    } else {
        format!("could not write {path}.")
    };
    outcome_from(&reply, headline)
}

fn load_document(session: &mut AgentSession, arguments: &Json) -> ToolOutcome {
    let Some(path) = arguments.get("path").and_then(Json::as_str) else {
        return ToolOutcome::rejected("`load_document` needs a `path` string.".to_string());
    };
    let params = Json::obj(vec![("path", Json::str(path))]);
    let reply = through_protocol(session, "load", params);
    let headline = if flag_of(&reply, "ok") {
        format!("adopted the document in {path}.")
    } else {
        format!("could not read {path}.")
    };
    outcome_from(&reply, headline)
}

fn typecheck(session: &mut AgentSession) -> ToolOutcome {
    let document = session.document();
    let report = check_document(&document, session.state().prelude());
    let state = state_json(session);

    let mut text = format!("well-typed: {}\n", report.well_typed);
    if report.complete() {
        text.push_str("complete: yes — there is no hole left anywhere in the document\n");
    } else {
        text.push_str(&format!(
            "complete: no — {} empty and {} non-empty hole(s) remain\n",
            report.empty_holes(),
            report.non_empty_holes()
        ));
    }
    text.push_str(&format!("definitions ({}):\n", report.definitions.len()));
    for definition in &report.definitions {
        text.push_str(&format!(
            "  {} : {}   {} empty hole(s), {} non-empty hole(s)\n",
            definition.name, definition.ann, definition.empty, definition.non_empty
        ));
    }
    text.push_str(&format!(
        "stdlib definitions in scope: {}\n",
        report.stdlib_definitions
    ));

    let structured = Json::obj(vec![
        ("well_typed", Json::Bool(report.well_typed)),
        ("complete", Json::Bool(report.complete())),
        ("empty_holes", Json::Int(report.empty_holes() as i64)),
        (
            "non_empty_holes",
            Json::Int(report.non_empty_holes() as i64),
        ),
        (
            "stdlib_definitions",
            Json::Int(report.stdlib_definitions as i64),
        ),
        (
            "definitions",
            Json::arr(
                report
                    .definitions
                    .iter()
                    .map(|definition| {
                        Json::obj(vec![
                            ("name", Json::str(definition.name.clone())),
                            ("ann", Json::str(definition.ann.clone())),
                            ("empty_holes", Json::Int(definition.empty as i64)),
                            ("non_empty_holes", Json::Int(definition.non_empty as i64)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("state", digest(&state)),
    ]);
    ToolOutcome::reported(text, structured)
}

fn status_meaning(status: i32) -> &'static str {
    match status {
        0 => "the run produced a value",
        2 => "the run is indeterminate: a hole blocks the answer",
        3 => "the run ran out of fuel",
        _ => "the run could not be attempted",
    }
}

fn run_main(session: &mut AgentSession, arguments: &Json) -> ToolOutcome {
    let fuel = arguments
        .get("fuel")
        .and_then(Json::as_usize)
        .unwrap_or(nothing_eval::DEFAULT_FUEL);
    if fuel == 0 {
        return ToolOutcome::rejected(
            "`fuel` of 0 would not let the program take a single step.".to_string(),
        );
    }
    let input: Vec<String> = arguments
        .get("stdin_lines")
        .and_then(Json::as_arr)
        .map(|lines| {
            lines
                .iter()
                .filter_map(Json::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let document = session.document();
    let mut io = Recorded::with_input(input);
    let report = match perform_or_evaluate(&document, session.state().prelude(), fuel, &mut io) {
        Ok(report) => report,
        Err(message) => return ToolOutcome::rejected(message),
    };

    let mut text = if report.performed {
        "`main` has a command type, so it was performed rather than evaluated to a value.\n"
            .to_string()
    } else {
        "`main` was evaluated.\n".to_string()
    };
    if io.written.is_empty() {
        text.push_str("\nthe program printed nothing.\n");
    } else {
        text.push_str("\nwhat the program printed:\n");
        for line in &io.written {
            text.push_str("  ");
            text.push_str(line);
            text.push('\n');
        }
    }
    if !report.lines.is_empty() {
        text.push('\n');
        for line in &report.lines {
            text.push_str(line);
            text.push('\n');
        }
    }
    if report.performed
        && let Some(value) = &report.value
    {
        text.push_str(&format!("\nthe command finished, yielding {value}\n"));
    }
    let status = report.status;
    let meaning = status_meaning(status);
    text.push_str(&format!("\nexit status {status}: {meaning}\n"));

    let structured = Json::obj(vec![
        ("performed", Json::Bool(report.performed)),
        ("status", Json::Int(status as i64)),
        (
            "value",
            match &report.value {
                Some(value) => Json::str(value.clone()),
                None => Json::Null,
            },
        ),
        (
            "printed",
            Json::arr(io.written.iter().map(Json::str).collect()),
        ),
        (
            "report",
            Json::arr(report.lines.iter().map(Json::str).collect()),
        ),
        ("fuel", Json::Int(fuel as i64)),
    ]);
    ToolOutcome::reported(text, structured)
}

fn stdlib(session: &mut AgentSession, arguments: &Json) -> ToolOutcome {
    let filter = arguments
        .get("filter")
        .and_then(Json::as_str)
        .unwrap_or("")
        .to_lowercase();
    let reply = through_protocol(session, "stdlib", Json::Obj(Vec::new()));
    let entries: Vec<Json> = reply
        .get("stdlib")
        .and_then(Json::as_arr)
        .map(<[Json]>::to_vec)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| {
            if filter.is_empty() {
                return true;
            }
            let name = text_of(entry, "name").to_lowercase();
            let doc = entry
                .get("doc")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_lowercase();
            name.contains(&filter) || doc.contains(&filter)
        })
        .collect();

    let count = entries.len();
    let mut text = format!(
        "{count} standard-library definition(s) in scope; call any of them with \
         `construct-var NAME`:\n"
    );
    for entry in &entries {
        let name = text_of(entry, "name");
        let ann = text_of(entry, "ann_text");
        let doc = entry.get("doc").and_then(Json::as_str).unwrap_or("");
        if doc.is_empty() {
            text.push_str(&format!("  {name} : {ann}\n"));
        } else {
            text.push_str(&format!("  {name} : {ann}   -- {doc}\n"));
        }
    }
    let structured = Json::obj(vec![
        ("count", Json::Int(count as i64)),
        ("stdlib", Json::Arr(entries)),
    ]);
    ToolOutcome::reported(text, structured)
}

fn action_grammar(session: &mut AgentSession) -> ToolOutcome {
    let reply = through_protocol(session, "help", Json::Obj(Vec::new()));
    let grammar = text_of(&reply, "step_grammar").to_string();
    let version = text_of(&reply, "protocol_version").to_string();
    let text = format!(
        "Every edit is one of these actions, named as a step string. \
         `apply_action` takes one; `apply_actions` takes a list.\n\n{grammar}\n"
    );
    let structured = Json::obj(vec![
        ("step_grammar", Json::str(grammar)),
        ("agent_protocol_version", Json::str(version)),
        ("methods", cloned(&reply, "methods")),
    ]);
    ToolOutcome::reported(text, structured)
}

fn undo(session: &mut AgentSession) -> ToolOutcome {
    let reply = through_protocol(session, "undo", Json::Obj(Vec::new()));
    let headline = if flag_of(&reply, "applied") {
        "undone.".to_string()
    } else {
        "there was nothing left to undo.".to_string()
    };
    outcome_from(&reply, headline)
}

fn redo(session: &mut AgentSession) -> ToolOutcome {
    let reply = through_protocol(session, "redo", Json::Obj(Vec::new()));
    let headline = if flag_of(&reply, "applied") {
        "redone.".to_string()
    } else {
        "there was nothing to redo.".to_string()
    };
    outcome_from(&reply, headline)
}

fn reset(session: &mut AgentSession) -> ToolOutcome {
    let reply = through_protocol(session, "reset", Json::Obj(Vec::new()));
    outcome_from(
        &reply,
        "reset to a single empty definition with an empty action log.".to_string(),
    )
}

fn move_to_hole(session: &mut AgentSession, arguments: &Json) -> ToolOutcome {
    let forward = arguments
        .get("forward")
        .and_then(Json::as_bool)
        .unwrap_or(true);
    let params = Json::obj(vec![("forward", Json::Bool(forward))]);
    let reply = through_protocol(session, "move_to_hole", params);
    let headline = if flag_of(&reply, "ok") {
        "moved the cursor to a hole.".to_string()
    } else {
        "the cursor did not move.".to_string()
    };
    outcome_from(&reply, headline)
}
