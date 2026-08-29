use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use nothing_action::act::EditState;
use nothing_action::log::{ActionLog, AuthorId};
use nothing_agentapi::json::{Json, escape, parse};
use nothing_agentapi::protocol::{METHODS, handle};
use nothing_agentapi::session::AgentSession;
use nothing_core::doc::Def;
use nothing_core::docs::DocTable;
use nothing_core::exp::{Exp, Id};
use nothing_core::names::NameTable;
use nothing_core::prelude::Prelude;
use nothing_core::ty::Ty;

const TEMP_DIR_MARKER: &str = "$TMPDIR/";

const UPDATE_FIXTURES_ENV: &str = "NOTHING_UPDATE_FIXTURES";

const SESSION_DESCRIPTION: &str = "a session over a two-definition prelude, built by `setup`";

const BUILD_STEPS: &[&str] = &[
    "construct-lam",
    "move-parent",
    "rename x0",
    "set-ann Num",
    "move-child 0",
    "construct-if",
    "construct-binop eq",
    "construct-var x0",
    "move-next-sibling",
    "construct-num 0",
    "move-parent",
    "move-next-sibling",
    "construct-num 1",
    "move-next-sibling",
    "construct-var twice",
    "construct-binop mul",
];

struct Case {
    fixture: &'static str,
    covers_method: Option<&'static str>,
    setup: Vec<Json>,
    request: Json,
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("protocol")
        .join("v1")
}

fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join("nothing-agentapi-protocol-v1-compat")
}

fn resolve_scratch_paths(value: &Json) -> Json {
    match value {
        Json::Str(text) => match text.strip_prefix(TEMP_DIR_MARKER) {
            Some(rest) => Json::Str(scratch_dir().join(rest).display().to_string()),
            None => value.clone(),
        },
        Json::Arr(items) => Json::Arr(items.iter().map(resolve_scratch_paths).collect()),
        Json::Obj(fields) => Json::Obj(
            fields
                .iter()
                .map(|(key, child)| (key.clone(), resolve_scratch_paths(child)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn prelude() -> Arc<Prelude> {
    let twice = Id::fresh();
    let unused = Id::fresh();
    let mut names = NameTable::new();
    names.set(twice, "twice");
    names.set(unused, "unused");
    let mut docs = DocTable::new();
    docs.set(twice, "two of them");
    docs.set(unused, "never called");
    Arc::new(Prelude::from_defs(
        vec![
            Def::new(twice, Ty::Num, Exp::Num(2)),
            Def::new(unused, Ty::Num, Exp::Num(0)),
        ],
        names,
        docs,
    ))
}

fn session() -> AgentSession {
    AgentSession::from_base(
        EditState::empty().under(prelude()),
        ActionLog::new(),
        AuthorId::new(1),
    )
}

fn request_of(method: &str, params: Vec<(&str, Json)>) -> Json {
    let mut fields = vec![("id", Json::Int(1)), ("method", Json::str(method))];
    if !params.is_empty() {
        fields.push(("params", Json::obj(params)));
    }
    Json::obj(fields)
}

fn steps_of(steps: &[&str]) -> Json {
    Json::arr(steps.iter().map(|step| Json::str(*step)).collect())
}

fn build() -> Json {
    request_of("script", vec![("steps", steps_of(BUILD_STEPS))])
}

fn apply_of(step: &str) -> Json {
    request_of("apply", vec![("step", Json::str(step))])
}

fn save_of() -> Json {
    request_of(
        "save",
        vec![("path", Json::str("$TMPDIR/document.nothing"))],
    )
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            fixture: "state",
            covers_method: Some("state"),
            setup: vec![build()],
            request: request_of("state", vec![]),
        },
        Case {
            fixture: "apply",
            covers_method: Some("apply"),
            setup: vec![build()],
            request: apply_of("move-parent"),
        },
        Case {
            fixture: "script",
            covers_method: Some("script"),
            setup: vec![build()],
            request: request_of(
                "script",
                vec![("steps", steps_of(&["move-parent", "move-parent"]))],
            ),
        },
        Case {
            fixture: "hole_context",
            covers_method: Some("hole_context"),
            setup: vec![build()],
            request: request_of("hole_context", vec![]),
        },
        Case {
            fixture: "stdlib",
            covers_method: Some("stdlib"),
            setup: vec![build()],
            request: request_of("stdlib", vec![]),
        },
        Case {
            fixture: "move_to_hole",
            covers_method: Some("move_to_hole"),
            setup: vec![build(), apply_of("move-parent")],
            request: request_of("move_to_hole", vec![("forward", Json::Bool(true))]),
        },
        Case {
            fixture: "undo",
            covers_method: Some("undo"),
            setup: vec![build()],
            request: request_of("undo", vec![]),
        },
        Case {
            fixture: "redo",
            covers_method: Some("redo"),
            setup: vec![build(), request_of("undo", vec![])],
            request: request_of("redo", vec![]),
        },
        Case {
            fixture: "reset",
            covers_method: Some("reset"),
            setup: vec![build()],
            request: request_of("reset", vec![]),
        },
        Case {
            fixture: "save",
            covers_method: Some("save"),
            setup: vec![build()],
            request: save_of(),
        },
        Case {
            fixture: "load",
            covers_method: Some("load"),
            setup: vec![build(), save_of()],
            request: request_of(
                "load",
                vec![("path", Json::str("$TMPDIR/document.nothing"))],
            ),
        },
        Case {
            fixture: "log",
            covers_method: Some("log"),
            setup: vec![build()],
            request: request_of("log", vec![]),
        },
        Case {
            fixture: "provenance",
            covers_method: Some("provenance"),
            setup: vec![build()],
            request: request_of("provenance", vec![]),
        },
        Case {
            fixture: "annotate",
            covers_method: Some("annotate"),
            setup: vec![build()],
            request: request_of(
                "annotate",
                vec![
                    ("agents", Json::arr(vec![Json::Int(1)])),
                    ("style", Json::str("brackets")),
                ],
            ),
        },
        Case {
            fixture: "help",
            covers_method: Some("help"),
            setup: vec![build()],
            request: request_of("help", vec![]),
        },
        Case {
            fixture: "version",
            covers_method: Some("version"),
            setup: vec![build()],
            request: request_of("version", vec![]),
        },
        Case {
            fixture: "quit",
            covers_method: Some("quit"),
            setup: vec![build()],
            request: request_of("quit", vec![]),
        },
        Case {
            fixture: "error_unknown_method",
            covers_method: None,
            setup: vec![build()],
            request: request_of("frobnicate", vec![]),
        },
        Case {
            fixture: "error_missing_method",
            covers_method: None,
            setup: vec![build()],
            request: Json::obj(vec![("id", Json::Int(1))]),
        },
        Case {
            fixture: "error_action_did_not_apply",
            covers_method: None,
            setup: vec![build()],
            request: apply_of("move-child 0"),
        },
        Case {
            fixture: "error_step_did_not_parse",
            covers_method: None,
            setup: vec![build()],
            request: apply_of("frobnicate"),
        },
        Case {
            fixture: "error_script_did_not_apply",
            covers_method: None,
            setup: vec![build()],
            request: request_of("script", vec![("steps", steps_of(&["frobnicate"]))]),
        },
    ]
}

fn kind_of(value: &Json) -> &'static str {
    match value {
        Json::Null => "null",
        Json::Bool(_) => "bool",
        Json::Int(_) => "int",
        Json::Float(_) => "float",
        Json::Str(_) => "string",
        Json::Arr(_) => "array",
        Json::Obj(_) => "object",
    }
}

fn collect_kinds(value: &Json, path: &str, kinds: &mut BTreeMap<String, BTreeSet<&'static str>>) {
    kinds
        .entry(path.to_string())
        .or_default()
        .insert(kind_of(value));
    match value {
        Json::Obj(fields) => {
            for (key, child) in fields {
                collect_kinds(child, &format!("{path}.{key}"), kinds);
            }
        }
        Json::Arr(items) => {
            for item in items {
                collect_kinds(item, &format!("{path}[]"), kinds);
            }
        }
        _ => {}
    }
}

fn union_of(mut kinds: BTreeSet<&'static str>) -> String {
    if kinds.len() > 1 {
        kinds.remove("null");
    }
    kinds.into_iter().collect::<Vec<&str>>().join("|")
}

fn shape_of(value: &Json) -> Vec<String> {
    let mut kinds = BTreeMap::new();
    collect_kinds(value, "$", &mut kinds);
    kinds
        .into_iter()
        .map(|(path, set)| format!("{path}: {}", union_of(set)))
        .collect()
}

fn shape_map(entries: &[String]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|entry| match entry.split_once(": ") {
            Some((path, kind)) => (path.to_string(), kind.to_string()),
            None => panic!("`{entry}` is not a `path: type` shape entry"),
        })
        .collect()
}

fn same_kind(pinned: &str, actual: &str) -> bool {
    pinned == actual || pinned == "null" || actual == "null"
}

fn pretty(value: &Json, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    let inner = "  ".repeat(indent + 1);
    match value {
        Json::Obj(fields) if !fields.is_empty() => {
            let body: Vec<String> = fields
                .iter()
                .map(|(key, child)| {
                    format!("{inner}{}: {}", escape(key), pretty(child, indent + 1))
                })
                .collect();
            format!("{{\n{}\n{pad}}}", body.join(",\n"))
        }
        Json::Arr(items) if !items.is_empty() => {
            let body: Vec<String> = items
                .iter()
                .map(|item| format!("{inner}{}", pretty(item, indent + 1)))
                .collect();
            format!("[\n{}\n{pad}]", body.join(",\n"))
        }
        other => other.to_string(),
    }
}

fn fixture_document(case: &Case, shape: &[String]) -> Json {
    Json::obj(vec![
        (
            "covers_method",
            match case.covers_method {
                Some(method) => Json::str(method),
                None => Json::Null,
            },
        ),
        ("session", Json::str(SESSION_DESCRIPTION)),
        ("setup", Json::arr(case.setup.clone())),
        ("request", case.request.clone()),
        (
            "response_shape",
            Json::arr(
                shape
                    .iter()
                    .map(|entry| Json::str(entry.as_str()))
                    .collect(),
            ),
        ),
    ])
}

fn response_shape_of(case: &Case) -> Vec<String> {
    let mut agent = session();
    for request in &case.setup {
        let outcome = handle(&mut agent, &resolve_scratch_paths(request));
        assert_eq!(
            outcome.value.get("ok").and_then(Json::as_bool),
            Some(true),
            "setup request for `{}` failed: {}",
            case.fixture,
            outcome.value
        );
    }
    let outcome = handle(&mut agent, &resolve_scratch_paths(&case.request));
    shape_of(&outcome.value)
}

fn fixture_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(fixture_dir())
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", fixture_dir().display()))
        .map(|entry| entry.expect("a readable directory entry").file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".json"))
        .map(|name| name.trim_end_matches(".json").to_string())
        .collect();
    names.sort();
    names
}

fn updating_fixtures() -> bool {
    std::env::var(UPDATE_FIXTURES_ENV).is_ok_and(|value| value == "1")
}

#[test]
fn every_path_pinned_by_protocol_v1_is_still_present_with_the_same_type() {
    let dir = scratch_dir();
    std::fs::create_dir_all(&dir).expect("the scratch directory is creatable");
    let update = updating_fixtures();
    let mut failures: Vec<String> = Vec::new();

    for case in cases() {
        let shape = response_shape_of(&case);
        let path = fixture_dir().join(format!("{}.json", case.fixture));
        if update {
            std::fs::create_dir_all(fixture_dir()).expect("the fixture directory is creatable");
            let document = fixture_document(&case, &shape);
            std::fs::write(&path, format!("{}\n", pretty(&document, 0)))
                .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
        }
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "cannot read {} ({e}); regenerate with {UPDATE_FIXTURES_ENV}=1",
                path.display()
            )
        });
        let document =
            parse(&text).unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()));

        let pinned_request = document
            .get("request")
            .unwrap_or_else(|| panic!("{} has no `request`", path.display()));
        assert_eq!(
            pinned_request,
            &case.request,
            "{} pins a different request than the test sends",
            path.display()
        );

        let pinned: Vec<String> = document
            .get("response_shape")
            .and_then(Json::as_arr)
            .unwrap_or_else(|| panic!("{} has no `response_shape`", path.display()))
            .iter()
            .map(|entry| {
                entry
                    .as_str()
                    .expect("a shape entry is a string")
                    .to_string()
            })
            .collect();

        let pinned = shape_map(&pinned);
        let actual = shape_map(&shape);

        for (pinned_path, pinned_kind) in &pinned {
            match actual.get(pinned_path) {
                None => failures.push(format!(
                    "{}: `{pinned_path}` was removed (pinned as {pinned_kind})",
                    case.fixture
                )),
                Some(actual_kind) if !same_kind(pinned_kind, actual_kind) => {
                    failures.push(format!(
                        "{}: `{pinned_path}` changed type from {pinned_kind} to {actual_kind}",
                        case.fixture
                    ));
                }
                Some(_) => {}
            }
        }

        let added: Vec<&String> = actual
            .keys()
            .filter(|path| !pinned.contains_key(*path))
            .collect();
        if !added.is_empty() {
            println!(
                "{}: {} path(s) added since v1 was pinned, which is allowed: {added:?}",
                case.fixture,
                added.len()
            );
        }
    }

    std::fs::remove_dir_all(&dir).ok();
    assert!(
        failures.is_empty(),
        "protocol v1 was broken:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_method_the_protocol_advertises_has_a_pinned_v1_fixture() {
    let covered: BTreeSet<&str> = cases()
        .iter()
        .filter_map(|case| case.covers_method)
        .collect();
    let missing: Vec<&&str> = METHODS
        .iter()
        .filter(|method| !covered.contains(*method))
        .collect();
    assert!(
        missing.is_empty(),
        "these methods have no pinned fixture: {missing:?}"
    );
    let stray: Vec<&str> = covered
        .iter()
        .copied()
        .filter(|method| !METHODS.contains(method))
        .collect();
    assert!(
        stray.is_empty(),
        "these fixtures pin methods the protocol no longer has: {stray:?}"
    );
}

#[test]
fn the_v1_fixture_directory_holds_exactly_the_pinned_cases() {
    let mut expected: Vec<String> = cases()
        .iter()
        .map(|case| case.fixture.to_string())
        .collect();
    expected.sort();
    assert_eq!(
        fixture_names(),
        expected,
        "the fixture directory and the pinned cases disagree; regenerate with {UPDATE_FIXTURES_ENV}=1 and delete anything stale"
    );
}

#[test]
fn a_removed_field_and_a_changed_type_are_both_caught_but_an_added_field_is_not() {
    let pinned = shape_of(&Json::obj(vec![
        ("ok", Json::Bool(true)),
        ("count", Json::Int(1)),
        ("note", Json::str("here")),
    ]));
    let pinned = shape_map(&pinned);

    let removed = shape_map(&shape_of(&Json::obj(vec![
        ("ok", Json::Bool(true)),
        ("count", Json::Int(1)),
    ])));
    assert!(!removed.contains_key("$.note"));

    let retyped = shape_map(&shape_of(&Json::obj(vec![
        ("ok", Json::Bool(true)),
        ("count", Json::str("1")),
        ("note", Json::str("here")),
    ])));
    assert!(!same_kind(&pinned["$.count"], &retyped["$.count"]));

    let added = shape_map(&shape_of(&Json::obj(vec![
        ("ok", Json::Bool(true)),
        ("count", Json::Int(1)),
        ("note", Json::str("here")),
        ("extra", Json::Bool(false)),
    ])));
    for (path, kind) in &pinned {
        assert!(same_kind(kind, &added[path]), "{path}");
    }
}

#[test]
fn a_nullable_field_is_pinned_leniently_and_array_elements_are_unioned() {
    assert!(same_kind("null", "string"));
    assert!(same_kind("string", "null"));
    assert!(!same_kind("string", "int"));

    let shape = shape_of(&Json::obj(vec![(
        "xs",
        Json::arr(vec![
            Json::obj(vec![("doc", Json::Null)]),
            Json::obj(vec![("doc", Json::str("a line"))]),
        ]),
    )]));
    let shape = shape_map(&shape);
    assert_eq!(shape["$.xs"], "array");
    assert_eq!(shape["$.xs[]"], "object");
    assert_eq!(shape["$.xs[].doc"], "string");
}
