use crossterm::event::KeyCode;
use nothing_core::doc::references;
use nothing_core::ty::Ty;
use nothing_tui::keys::{ctrl, handle_key, key};
use nothing_tui::render::{definition_lines, definition_title, render_to_string};
use nothing_tui::{AppState, Slot};

fn typed(state: AppState, text: &str) -> AppState {
    text.chars()
        .fold(state, |state, c| handle_key(key(KeyCode::Char(c)), state))
}

fn two_definitions() -> AppState {
    let state = typed(AppState::empty(), "1");
    let state = handle_key(ctrl(KeyCode::Char('n')), state);
    typed(state, "helper")
}

#[test]
fn a_fresh_document_has_one_definition_called_main() {
    let state = AppState::empty();
    assert_eq!(state.definition_count(), 1);
    assert_eq!(state.definition_name(), "main");
    assert_eq!(state.definition_index(), 0);
}

#[test]
fn ctrl_n_adds_a_definition_and_puts_the_cursor_in_its_name() {
    let state = handle_key(ctrl(KeyCode::Char('n')), AppState::empty());
    assert_eq!(state.definition_count(), 2);
    assert_eq!(state.definition_index(), 1);
    assert_eq!(state.slot, Slot::DefName);
}

#[test]
fn typing_in_the_definition_name_slot_renames_the_definition() {
    let state = two_definitions();
    assert_eq!(state.definition_name(), "helper");
    assert_eq!(state.definition_count(), 2);
}

#[test]
fn ctrl_up_and_ctrl_down_walk_the_definition_list() {
    let state = two_definitions();
    assert_eq!(state.definition_index(), 1);

    let up = handle_key(ctrl(KeyCode::Up), state.clone());
    assert_eq!(up.definition_index(), 0);
    assert_eq!(up.definition_name(), "main");
    assert_eq!(up.slot, Slot::Node);

    let stuck = handle_key(ctrl(KeyCode::Up), up.clone());
    assert_eq!(stuck.definition_index(), 0);
    assert_eq!(stuck.hint.as_deref(), Some("this is the first definition"));

    let down = handle_key(ctrl(KeyCode::Down), up);
    assert_eq!(down.definition_index(), 1);

    let end = handle_key(ctrl(KeyCode::Down), down);
    assert_eq!(end.hint.as_deref(), Some("this is the last definition"));
}

#[test]
fn ctrl_t_sets_the_definitions_annotation() {
    let state = handle_key(ctrl(KeyCode::Char('t')), AppState::empty());
    assert_eq!(state.slot, Slot::DefAnn);
    let state = typed(state, "Num");
    assert_eq!(state.definition_ann(), Ty::Num);
    assert!(state.edit.is_well_typed());
}

#[test]
fn an_annotation_that_the_body_cannot_meet_is_refused_with_a_hint() {
    let state = typed(AppState::empty(), "1");
    let state = handle_key(ctrl(KeyCode::Char('t')), state);
    let state = typed(state, "Bool");
    assert_eq!(state.definition_ann(), Ty::Hole);
    assert!(state.hint.is_some(), "the refusal was silent");
}

#[test]
fn one_definition_can_call_another_by_name() {
    let state = typed(AppState::empty(), "1");
    let state = handle_key(ctrl(KeyCode::Char('n')), state);
    let state = typed(state, "helper");
    let state = handle_key(ctrl(KeyCode::Up), state);

    let state = typed(state, "helper");
    assert_eq!(state.text(), "helper");
    assert!(state.edit.is_well_typed());

    let helper_id = state.definitions()[1];
    assert!(references(&state.program(), helper_id));
}

#[test]
fn ctrl_d_drops_a_definition_and_leaves_holes_where_it_was_called() {
    let state = typed(AppState::empty(), "1");
    let state = handle_key(ctrl(KeyCode::Char('n')), state);
    let state = typed(state, "helper");
    let state = handle_key(ctrl(KeyCode::Up), state);
    let state = typed(state, "helper");
    let helper_id = state.definitions()[1];

    let state = handle_key(ctrl(KeyCode::Down), state);
    assert_eq!(state.definition_index(), 1);
    let state = handle_key(ctrl(KeyCode::Char('d')), state);

    assert_eq!(state.definition_count(), 1);
    assert_eq!(state.definition_name(), "main");
    assert!(!references(&state.program(), helper_id));
    assert_eq!(state.text(), "⦇⦈");
    assert!(state.edit.is_well_typed());
}

#[test]
fn the_last_definition_cannot_be_dropped() {
    let state = handle_key(ctrl(KeyCode::Char('d')), AppState::empty());
    assert_eq!(state.definition_count(), 1);
    assert_eq!(
        state.hint.as_deref(),
        Some("a document keeps at least one definition")
    );
}

#[test]
fn the_definition_pane_lists_every_definition_and_marks_the_current_one() {
    let state = two_definitions();
    let lines = definition_lines(&state);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("  main"), "{lines:?}");
    assert!(lines[1].starts_with("> helper"), "{lines:?}");
    assert_eq!(definition_title(&state), " defs 2/2 ");
}

#[test]
fn the_definition_pane_is_on_screen_next_to_the_program() {
    let state = two_definitions();
    let screen = render_to_string(&state, 80, 12);
    assert!(screen.contains("defs 2/2"), "{screen}");
    assert!(screen.contains("main"), "{screen}");
    assert!(screen.contains("helper"), "{screen}");
    assert!(screen.contains("C-q quit"), "{screen}");
}

#[test]
fn a_single_definition_document_keeps_the_whole_width() {
    let screen = render_to_string(&AppState::factorial(), 60, 10);
    assert!(!screen.contains("defs 1/1"), "{screen}");
    assert!(
        screen.contains("»λx0:Num. if x0 == 0 then 1 else x0 * main (x0 - 1)«"),
        "{screen}"
    );
}

#[test]
fn live_values_follow_the_cursor_from_one_definition_to_another() {
    let state = typed(AppState::empty(), "1");
    let state = handle_key(ctrl(KeyCode::Char('n')), state);
    let state = typed(state, "helper");
    let state = typed(state, "2");
    assert!(nothing_tui::live::live_line(&state).contains('2'));

    let state = handle_key(ctrl(KeyCode::Up), state);
    assert!(nothing_tui::live::live_line(&state).contains('1'));
}

#[test]
fn arrow_keys_reach_the_definition_head_and_come_back() {
    let state = AppState::factorial();
    let head = handle_key(key(KeyCode::Up), state.clone());
    assert_eq!(head.slot, Slot::DefName);
    let ann = handle_key(key(KeyCode::Down), head);
    assert_eq!(ann.slot, Slot::DefAnn);
    let body = handle_key(key(KeyCode::Down), ann);
    assert_eq!(body.slot, Slot::Node);
    assert_eq!(body.program(), state.program());
}

#[test]
fn undo_reaches_back_across_a_definition_boundary() {
    let state = two_definitions();
    assert_eq!(state.definition_count(), 2);
    let mut state = state;
    for _ in 0..40 {
        match state.undo() {
            Some(next) => state = next,
            None => break,
        }
    }
    assert_eq!(state.definition_count(), 1);
    assert_eq!(state.text(), "⦇⦈");
}
