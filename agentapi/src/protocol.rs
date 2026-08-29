use nothing_action::act::{Action, ctx_and_expected_ty_at_in};
use nothing_action::cursor_render::render_with_cursor;
use nothing_action::log::AuthorId;
use nothing_action::script::{HELP, parse_step};

use crate::encode::{action_json, entry_json, exp_json, exp_kind, holes, names_json, ty_json};
use crate::holectx::hole_context;
use crate::json::{Json, parse};
use crate::provenance::{Palette, annotate, annotate_document, provenance_json, provenance_of};
use crate::session::AgentSession;

pub const PROTOCOL_VERSION: &str = "1";

pub const METHODS: &[&str] = &[
    "state",
    "apply",
    "script",
    "hole_context",
    "undo",
    "redo",
    "reset",
    "save",
    "load",
    "log",
    "provenance",
    "annotate",
    "help",
    "quit",
];

#[derive(Clone, PartialEq, Debug)]
pub struct Outcome {
    pub value: Json,
    pub quit: bool,
}

fn state_json(session: &AgentSession) -> Json {
    let state = session.state();
    let exp = state.exp();
    let (empty, non_empty) = holes(&exp);
    let (_, expected) = ctx_and_expected_ty_at_in(&state.scope(), &state.zipper);
    let doc = state.doc();
    Json::obj(vec![
        ("render", Json::str(state.render())),
        ("render_document", Json::str(state.render_document())),
        (
            "definitions",
            Json::arr(
                doc.defs()
                    .iter()
                    .map(|def| {
                        Json::obj(vec![
                            ("id", Json::str(def.id.to_string())),
                            ("name", Json::str(session.names().display(def.id))),
                            ("ann", ty_json(&def.ann)),
                            ("ann_text", Json::str(def.ann.to_string())),
                            ("current", Json::Bool(def.id == state.def_id())),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("definition", Json::str(state.def_id().to_string())),
        (
            "definition_name",
            Json::str(session.names().display(state.def_id())),
        ),
        ("definition_index", Json::Int(state.def_index() as i64)),
        ("definition_count", Json::Int(state.def_count() as i64)),
        ("definition_ann", ty_json(state.def_ann())),
        (
            "render_with_cursor",
            Json::str(render_with_cursor(&state.zipper, &state.names)),
        ),
        (
            "cursor_path",
            Json::arr(
                session
                    .cursor_path()
                    .iter()
                    .map(|n| Json::Int(*n as i64))
                    .collect(),
            ),
        ),
        ("focus_kind", Json::str(exp_kind(&state.zipper.focus))),
        ("expected_ty", ty_json(&expected)),
        ("expected_ty_text", Json::str(expected.to_string())),
        ("well_typed", Json::Bool(state.is_well_typed())),
        ("empty_holes", Json::Int(empty as i64)),
        ("non_empty_holes", Json::Int(non_empty as i64)),
        ("complete", Json::Bool(empty == 0 && non_empty == 0)),
        ("log_len", Json::Int(session.cursor() as i64)),
        ("can_undo", Json::Bool(session.can_undo())),
        ("can_redo", Json::Bool(session.can_redo())),
        ("author", Json::Int(session.author().0 as i64)),
        ("exp", exp_json(&exp, session.names())),
        ("names", names_json(session.names())),
    ])
}

fn ok(id: Option<&Json>, applied: bool, session: &AgentSession, extra: Vec<(&str, Json)>) -> Json {
    let mut fields: Vec<(String, Json)> = vec![
        ("id".to_string(), id.cloned().unwrap_or(Json::Null)),
        ("ok".to_string(), Json::Bool(true)),
        ("applied".to_string(), Json::Bool(applied)),
    ];
    for (key, value) in extra {
        fields.push((key.to_string(), value));
    }
    fields.push(("state".to_string(), state_json(session)));
    Json::Obj(fields)
}

fn err(id: Option<&Json>, message: impl Into<String>, session: &AgentSession) -> Json {
    Json::Obj(vec![
        ("id".to_string(), id.cloned().unwrap_or(Json::Null)),
        ("ok".to_string(), Json::Bool(false)),
        ("applied".to_string(), Json::Bool(false)),
        ("error".to_string(), Json::str(message.into())),
        ("state".to_string(), state_json(session)),
    ])
}

fn action_of(session: &AgentSession, params: &Json) -> Result<Action, String> {
    if let Some(text) = params.get("step").and_then(Json::as_str) {
        let step = parse_step(text).map_err(|e| e.to_string())?;
        return session.resolve(&step).map_err(|e| e.to_string());
    }
    if let Some(value) = params.get("action") {
        return crate::encode::action_from_json(value);
    }
    Err("`apply` needs either a `step` string or an `action` object".to_string())
}

fn author_of(session: &AgentSession, params: &Json) -> AuthorId {
    params
        .get("author")
        .and_then(Json::as_u64)
        .map(AuthorId::new)
        .unwrap_or_else(|| session.author())
}

pub fn handle(session: &mut AgentSession, request: &Json) -> Outcome {
    let id = request.get("id");
    let empty = Json::Obj(Vec::new());
    let params = request.get("params").unwrap_or(&empty);
    let Some(method) = request.get("method").and_then(Json::as_str) else {
        return Outcome {
            value: err(id, "a request needs a `method` string", session),
            quit: false,
        };
    };

    let value = match method {
        "state" => ok(id, false, session, vec![]),

        "help" => ok(
            id,
            false,
            session,
            vec![
                ("protocol_version", Json::str(PROTOCOL_VERSION)),
                (
                    "methods",
                    Json::arr(METHODS.iter().map(|m| Json::str(*m)).collect()),
                ),
                ("step_grammar", Json::str(HELP)),
            ],
        ),

        "apply" => match action_of(session, params) {
            Err(message) => err(id, message, session),
            Ok(action) => {
                let author = author_of(session, params);
                let applied = session.apply_as(action.clone(), author);
                if applied {
                    ok(id, true, session, vec![("action", action_json(&action))])
                } else {
                    Json::Obj(vec![
                        ("id".to_string(), id.cloned().unwrap_or(Json::Null)),
                        ("ok".to_string(), Json::Bool(true)),
                        ("applied".to_string(), Json::Bool(false)),
                        (
                            "error".to_string(),
                            Json::str("the action does not apply at this cursor"),
                        ),
                        ("action".to_string(), action_json(&action)),
                        ("state".to_string(), state_json(session)),
                    ])
                }
            }
        },

        "script" => {
            let Some(items) = params
                .get("steps")
                .and_then(|v| v.as_arr().map(<[Json]>::to_vec))
            else {
                return Outcome {
                    value: err(id, "`script` needs a `steps` array", session),
                    quit: false,
                };
            };
            let author = author_of(session, params);
            let mut results = Vec::new();
            let mut all_applied = true;
            for (index, item) in items.iter().enumerate() {
                let as_params = match item {
                    Json::Str(text) => Json::obj(vec![("step", Json::str(text.clone()))]),
                    other => Json::obj(vec![("action", other.clone())]),
                };
                match action_of(session, &as_params) {
                    Err(message) => {
                        all_applied = false;
                        results.push(Json::obj(vec![
                            ("index", Json::Int(index as i64)),
                            ("applied", Json::Bool(false)),
                            ("error", Json::str(message)),
                        ]));
                        break;
                    }
                    Ok(action) => {
                        let applied = session.apply_as(action.clone(), author);
                        all_applied &= applied;
                        results.push(Json::obj(vec![
                            ("index", Json::Int(index as i64)),
                            ("applied", Json::Bool(applied)),
                            ("action", action_json(&action)),
                        ]));
                        if !applied {
                            break;
                        }
                    }
                }
            }
            ok(
                id,
                all_applied,
                session,
                vec![("steps", Json::arr(results))],
            )
        }

        "hole_context" => ok(
            id,
            false,
            session,
            vec![("hole_context", hole_context(session.state()).to_json())],
        ),

        "undo" => {
            let applied = session.undo();
            ok(id, applied, session, vec![])
        }

        "redo" => {
            let applied = session.redo();
            ok(id, applied, session, vec![])
        }

        "reset" => {
            session.reset();
            ok(id, true, session, vec![])
        }

        "save" => match params.get("path").and_then(Json::as_str) {
            None => err(id, "`save` needs a `path` string", session),
            Some(path) => match session.save(path) {
                Err(message) => err(id, message, session),
                Ok(bytes) => ok(
                    id,
                    true,
                    session,
                    vec![
                        ("path", Json::str(path)),
                        ("bytes", Json::Int(bytes as i64)),
                    ],
                ),
            },
        },

        "load" => match params.get("path").and_then(Json::as_str) {
            None => err(id, "`load` needs a `path` string", session),
            Some(path) => match session.load(path) {
                Err(message) => err(id, message, session),
                Ok(()) => ok(id, true, session, vec![("path", Json::str(path))]),
            },
        },

        "log" => ok(
            id,
            false,
            session,
            vec![(
                "log",
                Json::arr(session.applied_entries().iter().map(entry_json).collect()),
            )],
        ),

        "provenance" => {
            let map = provenance_of(session.base(), &session.applied_entries());
            ok(
                id,
                false,
                session,
                vec![("provenance", provenance_json(&map))],
            )
        }

        "annotate" => {
            let map = provenance_of(session.base(), &session.applied_entries());
            let agents: Vec<AuthorId> = params
                .get("agents")
                .and_then(Json::as_arr)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Json::as_u64)
                        .map(AuthorId::new)
                        .collect()
                })
                .unwrap_or_default();
            let palette = match params.get("style").and_then(Json::as_str) {
                Some("ansi") => Palette::ansi(),
                Some("plain") => Palette::plain(),
                _ => Palette::brackets(),
            };
            ok(
                id,
                false,
                session,
                vec![
                    (
                        "annotated",
                        Json::str(annotate(
                            &session.exp(),
                            session.names(),
                            &map.in_definition(session.state().def_id()),
                            &agents,
                            &palette,
                        )),
                    ),
                    (
                        "annotated_document",
                        Json::str(annotate_document(
                            &session.state().doc(),
                            session.names(),
                            &map,
                            &agents,
                            &palette,
                        )),
                    ),
                ],
            )
        }

        "quit" => {
            return Outcome {
                value: ok(id, true, session, vec![]),
                quit: true,
            };
        }

        other => err(id, format!("unknown method `{other}`"), session),
    };

    Outcome { value, quit: false }
}

pub fn handle_line(session: &mut AgentSession, line: &str) -> Option<Outcome> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    match parse(trimmed) {
        Err(e) => Some(Outcome {
            value: err(None, format!("malformed request: {e}"), session),
            quit: false,
        }),
        Ok(request) => Some(handle(session, &request)),
    }
}

pub fn author_from_args(args: &[String]) -> AuthorId {
    let mut author = 1u64;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--author" {
            if let Some(value) = args.get(i + 1).and_then(|v| v.parse::<u64>().ok()) {
                author = value;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    AuthorId::new(author)
}

pub fn run_stdio<R: std::io::BufRead, W: std::io::Write>(
    session: &mut AgentSession,
    input: R,
    mut output: W,
) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        let Some(outcome) = handle_line(session, &line) else {
            continue;
        };
        writeln!(output, "{}", outcome.value)?;
        output.flush()?;
        if outcome.quit {
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> AgentSession {
        AgentSession::new(AuthorId::new(1))
    }

    fn request(text: &str) -> Json {
        parse(text).unwrap()
    }

    #[test]
    fn every_response_reports_whether_the_action_applied_and_the_render() {
        let mut s = session();
        for text in [
            r#"{"method":"state"}"#,
            r#"{"method":"apply","params":{"step":"construct-lam"}}"#,
            r#"{"method":"apply","params":{"step":"move-parent"}}"#,
            r#"{"method":"apply","params":{"step":"frobnicate"}}"#,
            r#"{"method":"hole_context"}"#,
            r#"{"method":"undo"}"#,
            r#"{"method":"log"}"#,
            r#"{"method":"help"}"#,
            r#"{"method":"nope"}"#,
        ] {
            let out = handle(&mut s, &request(text));
            assert!(out.value.get("applied").is_some(), "{text}");
            let state = out.value.get("state").expect(text);
            assert!(
                state.get("render").and_then(Json::as_str).is_some(),
                "{text}"
            );
            assert!(
                state
                    .get("render_with_cursor")
                    .and_then(Json::as_str)
                    .is_some(),
                "{text}"
            );
            assert!(
                state.get("cursor_path").and_then(Json::as_arr).is_some(),
                "{text}"
            );
        }
    }

    #[test]
    fn a_step_string_and_a_structured_action_agree() {
        let mut a = session();
        let mut b = session();
        handle(
            &mut a,
            &request(r#"{"method":"apply","params":{"step":"construct-num 7"}}"#),
        );
        handle(
            &mut b,
            &request(
                r#"{"method":"apply","params":{"action":{"action":"ConstructNum","value":7}}}"#,
            ),
        );
        assert_eq!(a.state().render(), "7");
        assert_eq!(a.state().render(), b.state().render());
    }

    #[test]
    fn an_action_that_does_not_apply_answers_ok_but_not_applied() {
        let mut s = session();
        let out = handle(
            &mut s,
            &request(r#"{"method":"apply","params":{"step":"move-parent"}}"#),
        );
        assert_eq!(out.value.get("ok").unwrap().as_bool(), Some(true));
        assert_eq!(out.value.get("applied").unwrap().as_bool(), Some(false));
        assert_eq!(s.log().len(), 0);
    }

    #[test]
    fn an_unparseable_step_answers_not_ok() {
        let mut s = session();
        let out = handle(
            &mut s,
            &request(r#"{"method":"apply","params":{"step":"frobnicate"}}"#),
        );
        assert_eq!(out.value.get("ok").unwrap().as_bool(), Some(false));
        assert!(
            out.value
                .get("error")
                .unwrap()
                .as_str()
                .unwrap()
                .contains("unknown action")
        );
    }

    #[test]
    fn the_request_id_is_echoed() {
        let mut s = session();
        let out = handle(&mut s, &request(r#"{"id":42,"method":"state"}"#));
        assert_eq!(out.value.get("id").unwrap().as_i64(), Some(42));
        let out = handle(&mut s, &request(r#"{"id":"abc","method":"state"}"#));
        assert_eq!(out.value.get("id").unwrap().as_str(), Some("abc"));
    }

    #[test]
    fn a_script_runs_a_whole_sequence_in_one_request() {
        let mut s = session();
        let out = handle(
            &mut s,
            &request(
                r#"{"method":"script","params":{"steps":["construct-num 1","construct-binop add","construct-num 2"]}}"#,
            ),
        );
        assert_eq!(out.value.get("applied").unwrap().as_bool(), Some(true));
        assert_eq!(s.state().render(), "1 + 2");
        assert_eq!(out.value.get("steps").unwrap().as_arr().unwrap().len(), 3);
    }

    #[test]
    fn a_script_stops_at_the_first_step_that_does_not_apply() {
        let mut s = session();
        let out = handle(
            &mut s,
            &request(r#"{"method":"script","params":{"steps":["move-parent","construct-num 1"]}}"#),
        );
        assert_eq!(out.value.get("applied").unwrap().as_bool(), Some(false));
        assert_eq!(out.value.get("steps").unwrap().as_arr().unwrap().len(), 1);
        assert_eq!(s.log().len(), 0);
    }

    #[test]
    fn quit_sets_the_quit_flag_and_still_answers() {
        let mut s = session();
        let out = handle(&mut s, &request(r#"{"method":"quit"}"#));
        assert!(out.quit);
        assert_eq!(out.value.get("ok").unwrap().as_bool(), Some(true));
    }

    #[test]
    fn a_malformed_line_answers_rather_than_dying() {
        let mut s = session();
        let out = handle_line(&mut s, "{not json").unwrap();
        assert_eq!(out.value.get("ok").unwrap().as_bool(), Some(false));
        assert!(handle_line(&mut s, "   ").is_none());
        assert!(handle_line(&mut s, "# a comment").is_none());
    }

    #[test]
    fn hole_context_comes_back_with_constructions() {
        let mut s = session();
        handle(
            &mut s,
            &request(r#"{"method":"apply","params":{"step":"construct-if"}}"#),
        );
        let out = handle(&mut s, &request(r#"{"method":"hole_context"}"#));
        let hc = out.value.get("hole_context").unwrap();
        assert_eq!(hc.get("expected_ty_text").unwrap().as_str(), Some("Bool"));
        assert!(
            !hc.get("constructions")
                .unwrap()
                .as_arr()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn save_and_load_round_trip_through_the_protocol() {
        let dir = std::env::temp_dir().join("nothing-agentapi-protocol-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doc.nothing");
        let path = path.to_str().unwrap().to_string();

        let mut s = session();
        handle(
            &mut s,
            &request(
                r#"{"method":"script","params":{"steps":["construct-num 1","construct-binop add","construct-num 2"]}}"#,
            ),
        );
        let out = handle(
            &mut s,
            &request(&format!(
                r#"{{"method":"save","params":{{"path":"{path}"}}}}"#
            )),
        );
        assert_eq!(out.value.get("ok").unwrap().as_bool(), Some(true));

        let mut fresh = session();
        let out = handle(
            &mut fresh,
            &request(&format!(
                r#"{{"method":"load","params":{{"path":"{path}"}}}}"#
            )),
        );
        assert_eq!(out.value.get("ok").unwrap().as_bool(), Some(true));
        assert_eq!(
            out.value
                .get("state")
                .unwrap()
                .get("render")
                .unwrap()
                .as_str(),
            Some("1 + 2")
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn loading_a_missing_file_is_an_error_not_a_panic() {
        let mut s = session();
        let out = handle(
            &mut s,
            &request(r#"{"method":"load","params":{"path":"/nonexistent/nothing.bin"}}"#),
        );
        assert_eq!(out.value.get("ok").unwrap().as_bool(), Some(false));
    }

    #[test]
    fn reset_returns_the_empty_program() {
        let mut s = session();
        handle(
            &mut s,
            &request(r#"{"method":"apply","params":{"step":"construct-num 5"}}"#),
        );
        let out = handle(&mut s, &request(r#"{"method":"reset"}"#));
        assert_eq!(
            out.value
                .get("state")
                .unwrap()
                .get("render")
                .unwrap()
                .as_str(),
            Some("⦇⦈")
        );
    }

    #[test]
    fn every_response_is_a_single_line() {
        let mut s = session();
        for text in [
            r#"{"method":"help"}"#,
            r#"{"method":"apply","params":{"step":"construct-lam"}}"#,
            r#"{"method":"hole_context"}"#,
        ] {
            let out = handle(&mut s, &request(text));
            assert!(!out.value.to_string().contains('\n'), "{text}");
        }
    }
}
