use nothing_action::act::Action;
use nothing_action::script::replay_script;
use nothing_core::exp::Exp;
use nothing_tui::keys::{handle_key, key};
use nothing_tui::render::program_line;
use nothing_tui::{AppState, ProjectionKind};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const STATE_MACHINE_FIXTURE: &str = include_str!("../../bench/fixtures/state_machine.actions");

fn state_machine_state() -> AppState {
    let replayed = replay_script(STATE_MACHINE_FIXTURE)
        .expect("the embedded state machine fixture must replay cleanly");
    AppState::with_names(replayed.exp(), replayed.names.clone())
}

fn ctrl_p() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL)
}

#[test]
fn a_state_machine_shaped_function_auto_selects_the_table_without_being_told() {
    let state = state_machine_state();
    assert_eq!(state.active_projection(), ProjectionKind::StateMachine);

    let factorial = AppState::factorial();
    assert_eq!(factorial.active_projection(), ProjectionKind::Text);
}

#[test]
fn the_override_key_forces_text_back_and_then_cycles() {
    let state = state_machine_state();
    assert_eq!(state.active_projection(), ProjectionKind::StateMachine);

    let forced_text = handle_key(ctrl_p(), state.clone());
    assert_eq!(forced_text.active_projection(), ProjectionKind::Text);

    let back_to_table = handle_key(ctrl_p(), forced_text.clone());
    assert_eq!(
        back_to_table.active_projection(),
        ProjectionKind::StateMachine
    );

    let beginner = handle_key(ctrl_p(), back_to_table);
    assert_eq!(beginner.active_projection(), ProjectionKind::Beginner);

    let auto_again = handle_key(ctrl_p(), beginner);
    assert_eq!(auto_again.projection_override, None);
    assert_eq!(auto_again.active_projection(), ProjectionKind::StateMachine);
}

#[test]
fn an_edit_made_through_the_state_machine_projection_is_visible_in_the_text_projection() {
    let state = state_machine_state();
    assert_eq!(state.active_projection(), ProjectionKind::StateMachine);
    assert!(state.text().contains("Idle x0 -> `Running {}"));

    let on_row0_result = handle_key(key(KeyCode::Right), state);
    assert!(
        matches!(on_row0_result.focus(), Exp::Inj(..)),
        "table navigation must move the real cursor onto the row's result node: {:?}",
        on_row0_result.focus()
    );

    let edited = handle_key(key(KeyCode::Delete), on_row0_result);
    assert!(matches!(edited.focus(), Exp::EmptyHole(_)));
    assert!(
        edited.text().contains("Idle x0 -> ⦇⦈"),
        "the key pressed through the table reached the real AST: {}",
        edited.text()
    );
    assert!(
        program_line(&edited).contains("Idle x0 -> »⦇⦈«"),
        "the text projection shows the same edit: {}",
        program_line(&edited)
    );
}

#[test]
fn an_edit_made_through_the_text_projection_is_visible_in_the_state_machine_projection() {
    let state = state_machine_state();
    let forced_text = handle_key(ctrl_p(), state);
    assert_eq!(forced_text.active_projection(), ProjectionKind::Text);

    let edited = forced_text
        .apply_actions(&[
            Action::MoveChild(0),
            Action::MoveChild(1),
            Action::MoveChild(0),
            Action::ConstructNum(99),
        ])
        .expect("row 0's result is a plain node under ordinary tree movement");
    assert!(matches!(edited.focus(), Exp::Num(99)));

    let table = nothing_tui::state_machine::marked_text(&edited);
    assert!(
        table.contains("99"),
        "an edit made while the text projection was forced is visible back in the table: {table}"
    );
}

#[test]
fn cycling_to_beginner_shows_verbose_prose_for_the_same_program() {
    let state = AppState::factorial();
    assert_eq!(state.active_projection(), ProjectionKind::Text);
    let beginner = [ctrl_p(), ctrl_p(), ctrl_p()]
        .into_iter()
        .fold(state, |s, k| handle_key(k, s));
    assert_eq!(beginner.active_projection(), ProjectionKind::Beginner);

    let marked = nothing_tui::projection::active(&beginner).marked_text(&beginner);
    assert!(marked.contains("a function taking"));
    assert!(marked.contains("otherwise"));

    let plain = nothing_tui::beginner::phrase(&beginner.program(), beginner.names());
    for symbol in ["+", "-", "*", "<", "=="] {
        assert!(
            !plain.contains(symbol),
            "beginner prose still has `{symbol}`: {plain}"
        );
    }
}
