use std::path::PathBuf;

use nothing_action::log::AuthorId;
use nothing_agentapi::json::{Json, parse};
use nothing_agentapi::provenance::{Palette, annotate, provenance_of};
use nothing_agentapi::session::AgentSession;
use nothing_core::render::render;
use nothing_core::typing::is_well_typed;

const HUMAN: AuthorId = AuthorId::new(1);
const MODEL: AuthorId = AuthorId::new(2);

fn transcript(name: &str) -> Vec<Json> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("bench/agent-transcripts")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| parse(line).unwrap_or_else(|e| panic!("bad transcript line: {e}")))
        .collect()
}

fn record<'a>(records: &'a [Json], kind: &str) -> &'a Json {
    records
        .iter()
        .find(|r| r.get("record").and_then(Json::as_str) == Some(kind))
        .unwrap_or_else(|| panic!("no `{kind}` record in the transcript"))
}

fn model_actions(records: &[Json]) -> Vec<String> {
    records
        .iter()
        .filter(|r| r.get("record").and_then(Json::as_str) == Some("step"))
        .filter(|r| r.get("applied").and_then(Json::as_bool) == Some(true))
        .filter_map(|r| r.get("action").and_then(Json::as_str))
        .map(str::to_string)
        .collect()
}

fn human_setup(records: &[Json]) -> Vec<String> {
    record(records, "run")
        .get("setup")
        .and_then(Json::as_arr)
        .map(|items| {
            items
                .iter()
                .filter_map(Json::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn replay(setup: &[String], model: &[String]) -> AgentSession {
    let mut session = AgentSession::new(HUMAN);
    for step in setup {
        assert!(
            session
                .apply_text(step)
                .unwrap_or_else(|e| panic!("`{step}`: {e}")),
            "human setup step `{step}` did not apply"
        );
    }
    session.set_author(MODEL);
    for step in model {
        assert!(
            session
                .apply_text(step)
                .unwrap_or_else(|e| panic!("`{step}`: {e}")),
            "model step `{step}` did not apply"
        );
    }
    session
}

fn strip(text: &str) -> String {
    text.replace('⟦', "")
        .replace('⟧', "")
        .replace('⟨', "")
        .replace('⟩', "")
}

#[test]
fn the_recorded_model_run_replays_to_the_factorial_reference_program() {
    let records = transcript("factorial.jsonl");
    let run = record(&records, "run");
    assert_eq!(
        run.get("model").and_then(Json::as_str),
        Some("claude-haiku-4-5-20251001")
    );
    let target = run
        .get("target")
        .and_then(Json::as_str)
        .unwrap()
        .to_string();

    let summary = record(&records, "summary");
    assert_eq!(
        summary.get("reached_target").and_then(Json::as_bool),
        Some(true)
    );

    let actions = model_actions(&records);
    assert!(
        !actions.is_empty(),
        "the transcript recorded no applied actions"
    );
    let session = replay(&[], &actions);
    assert_eq!(session.state().render(), target);
    assert!(is_well_typed(&session.exp()));
    assert_eq!(
        summary.get("final_render").and_then(Json::as_str),
        Some(target.as_str())
    );
}

#[test]
fn every_recorded_reply_that_applied_still_applies_on_replay() {
    for name in ["factorial.jsonl", "mixed-authorship.jsonl"] {
        let records = transcript(name);
        let session = replay(&human_setup(&records), &model_actions(&records));
        let summary = record(&records, "summary");
        assert_eq!(
            session.state().render(),
            summary.get("final_render").and_then(Json::as_str).unwrap(),
            "{name} replayed to a different program"
        );
    }
}

#[test]
fn the_real_model_transcript_replayed_over_a_human_base_distinguishes_the_authors() {
    let records = transcript("mixed-authorship.jsonl");
    let setup = human_setup(&records);
    assert!(
        !setup.is_empty(),
        "this transcript has a human-authored base"
    );
    let model = model_actions(&records);
    assert!(!model.is_empty());

    let session = replay(&setup, &model);
    let map = provenance_of(session.base(), &session.applied_entries());

    let marked = annotate(
        &session.exp(),
        session.names(),
        &map,
        &[MODEL],
        &Palette::brackets(),
    );
    assert_eq!(marked, "λn:Num. if n < 0 then ⟦0 - n⟧ else n");
    assert_eq!(
        strip(&marked),
        render(&session.exp(), session.names()),
        "the annotation must not change the projection"
    );
    assert_eq!(
        marked,
        record(&records, "summary")
            .get("annotated_render")
            .and_then(Json::as_str)
            .unwrap(),
        "the replayed annotation must match what the harness recorded live"
    );

    let authors = map.authors();
    assert!(authors.contains(&HUMAN), "{authors:?}");
    assert!(authors.contains(&MODEL), "{authors:?}");

    assert_eq!(map.get(&[]).map(|v| v.author), Some(HUMAN));
    assert_eq!(map.get(&[0, 0]).map(|v| v.author), Some(HUMAN));
    assert_eq!(map.get(&[0, 2]).map(|v| v.author), Some(HUMAN));
    assert_eq!(map.get(&[0, 1]).map(|v| v.author), Some(MODEL));
    assert_eq!(map.get(&[0, 1, 0]).map(|v| v.author), Some(MODEL));
    assert_eq!(map.get(&[0, 1, 1]).map(|v| v.author), Some(MODEL));
}

#[test]
fn a_fully_model_authored_program_is_marked_end_to_end() {
    let records = transcript("factorial.jsonl");
    let session = replay(&[], &model_actions(&records));
    let map = provenance_of(session.base(), &session.applied_entries());
    let marked = annotate(
        &session.exp(),
        session.names(),
        &map,
        &[MODEL],
        &Palette::brackets(),
    );
    assert!(marked.starts_with('⟦') && marked.ends_with('⟧'), "{marked}");
    assert_eq!(strip(&marked), render(&session.exp(), session.names()));
    assert_eq!(map.authors(), vec![MODEL]);
}

#[test]
fn the_provenance_filter_over_the_diff_agrees_with_the_node_projection() {
    use nothing_merge::provenance::{Filter, attribute_from};

    let records = transcript("mixed-authorship.jsonl");
    let setup = human_setup(&records);
    let model = model_actions(&records);

    let mut base = AgentSession::new(HUMAN);
    for step in &setup {
        assert!(base.apply_text(step).unwrap());
    }
    let base_state = base.state().clone();

    let session = replay(&setup, &model);
    let entries: Vec<_> = session
        .applied_entries()
        .into_iter()
        .skip(setup.len())
        .collect();

    let attributed = attribute_from(&base_state, &entries);
    assert!(!attributed.ops.is_empty());
    assert_eq!(attributed.authors(), vec![MODEL]);
    assert!(attributed.by(HUMAN).is_empty());
    assert_eq!(
        attributed.filter(&Filter::only(MODEL)).len(),
        attributed.ops.len()
    );
    assert_eq!(attributed.head.render(), session.state().render());
}
