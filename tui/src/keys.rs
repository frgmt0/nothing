
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use nothing_action::act::Action;
use nothing_core::exp::{Exp, Id, Op, Side};
use nothing_core::render::{PREC_APP, PREC_ATOM, PREC_BINDER, Prec, op_prec};

use crate::annot::{self, Accept};
use crate::app::{AppState, Slot};
use crate::complete;

pub fn handle_key(key: KeyEvent, state: AppState) -> AppState {
    if key.kind == KeyEventKind::Release {
        return state;
    }
    let state = state.clear_hint();


    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && key.code == KeyCode::Char('z') {
        return or_hint(state.undo(), state, "nothing to undo");
    }
    if ctrl && key.code == KeyCode::Char('r') {
        return or_hint(state.redo(), state, "nothing to redo");
    }

    let opened = state.open_keystroke();
    dispatch(key, state).close_keystroke(opened)
}

fn dispatch(key: KeyEvent, state: AppState) -> AppState {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match (key.code, ctrl) {

        (KeyCode::Char('q'), true) => quit(state),


        (KeyCode::Down, false) => {
            or_hint(state.move_down(), state, "nothing below: this is a leaf")
        }
        (KeyCode::Up, false) => or_hint(state.move_up(), state, "already at the root"),
        (KeyCode::Right, false) => or_hint(state.move_next(), state, "no next sibling"),
        (KeyCode::Left, false) => or_hint(state.move_prev(), state, "no previous sibling"),
        (KeyCode::Tab, false) => or_hint(state.move_to_hole(true), state, NOTHING_UNFINISHED),
        (KeyCode::BackTab, _) => or_hint(state.move_to_hole(false), state, NOTHING_UNFINISHED),


        (KeyCode::Backspace, false) => backspace(state),
        (KeyCode::Delete, false) => delete(state),
        (KeyCode::Enter, false) => enter(state),


        (KeyCode::Esc, false) => end_run(state),


        (KeyCode::Char(c), false) => printable(c, state),

        _ => unbound(key, state),
    }
}


fn printable(c: char, state: AppState) -> AppState {
    match state.slot {
        Slot::BinderName => binder_name_key(c, state),
        Slot::Annotation => annotation_key(c, state),
        Slot::Node => node_key(c, state),
    }
}

fn node_key(c: char, state: AppState) -> AppState {


    if matches!(state.focus(), Exp::NonEmptyHole(..)) && c != '!' {
        return match state.apply_actions(&[Action::MoveChild(0)]) {
            Some(mut inner) => {


                inner.entry = state.entry.clone();
                inner.entry_committed = state.entry_committed;
                node_key(c, inner)
            }
            None => state.with_hint("cannot look inside this hole"),
        };
    }


    if !state.entry.is_empty() && is_name_char(c) {
        return name_run(c, state);
    }


    let pending = (!state.entry_committed && !state.entry.is_empty()).then(|| state.entry.clone());
    let mut state = state;
    state.clear_entry();

    match c {
        '0'..='9' => digit(c, state),
        c if is_name_start(c) => name_run(c, state),

        '+' => operator(Op::Add, state),
        '-' => operator(Op::Sub, state),
        '*' => operator(Op::Mul, state),
        '<' => operator(Op::Lt, state),
        '=' => operator(Op::Eq, state),

        ' ' => wrap(state, PREC_APP, Action::ConstructAp, "application"),
        '\\' => binder(state, Action::ConstructLam, pending, "λ"),
        '?' => wrap(state, PREC_BINDER, Action::ConstructIf, "if"),
        ';' => binder(state, Action::ConstructLet, pending, "let"),
        ',' => wrap(state, PREC_BINDER, Action::ConstructPair, "pair"),
        '[' => wrap(state, PREC_APP, Action::ConstructProj(Side::L), "fst"),
        ']' => wrap(state, PREC_APP, Action::ConstructProj(Side::R), "snd"),


        '!' => wrap(
            state,
            PREC_ATOM,
            Action::ConstructNonEmptyHole,
            "quarantine",
        ),

        '~' => negate(state),
        ':' => to_annotation(state),
        '.' => state.with_hint("`.` addresses a binder's body; the cursor is not on a binder"),
        _ => state.with_hint(format!("`{c}` is not bound here")),
    }
}

fn binder_name_key(c: char, state: AppState) -> AppState {
    match c {
        ':' if matches!(state.focus(), Exp::Lam(..)) => to_annotation(state),
        '=' if matches!(state.focus(), Exp::Let(..)) => {


            match state.apply_actions(&[Action::MoveChild(0)]) {
                Some(next) => next,
                None => state.with_hint("this let has no bound expression"),
            }
        }
        '.' => to_body(state),

        '~' => state.with_hint("`~` negates a number"),
        c if is_name_char(c) => name_binder(c.to_string(), state, true),
        _ => exit_and_reprocess(c, state),
    }
}

fn annotation_key(c: char, state: AppState) -> AppState {
    if c == '.' {
        return to_body(state);
    }
    if c == ':' {


        return state.with_hint("already in the annotation slot");
    }
    match annot::accept(&state.entry, c) {
        Accept::Ignore => state.with_hint("there is no `(` to close"),
        Accept::Exit => exit_and_reprocess(c, state),
        Accept::Append | Accept::Swallow => {
            let mut buffer = state.entry.clone();
            buffer.push(c);
            set_ann(buffer, state)
        }
    }
}


fn digit(c: char, state: AppState) -> AppState {
    let d = i64::from(c.to_digit(10).expect("matched against 0-9"));
    let action = match state.focus() {
        Exp::Num(n) => {
            let extended = if *n < 0 {
                n.checked_mul(10).and_then(|shifted| shifted.checked_sub(d))
            } else {
                n.checked_mul(10).and_then(|shifted| shifted.checked_add(d))
            };
            match extended {
                Some(value) => Action::ConstructNum(value),
                None => return state.with_hint("that number does not fit in 64 bits"),
            }
        }
        _ => Action::ConstructNum(d),
    };
    apply_or_hint(state, &[action], "a number does not fit here")
}

fn negate(state: AppState) -> AppState {
    match state.focus() {
        Exp::Num(n) => match n.checked_neg() {
            Some(negated) => apply_or_hint(
                state,
                &[Action::ConstructNum(negated)],
                "a number does not fit here",
            ),
            None => state.with_hint("that number cannot be negated"),
        },
        _ => state.with_hint("`~` negates a number"),
    }
}

fn name_run(c: char, state: AppState) -> AppState {
    let mut buffer = state.entry.clone();
    buffer.push(c);
    commit_run(buffer, state)
}

fn commit_run(buffer: String, state: AppState) -> AppState {
    let committed = state.entry_committed;
    let Some(candidate) = complete::best(&state, &buffer) else {


        let mut next = match (committed, state.apply_actions(&[Action::Delete])) {
            (true, Some(deleted)) => deleted,
            _ => state,
        };
        next.entry = buffer.clone();
        next.entry_committed = false;
        return next.with_hint(format!("no name in scope starts with `{buffer}`"));
    };

    let mut actions = Vec::new();
    if committed {
        actions.push(Action::Delete);
    }
    actions.push(candidate.action());

    match state.apply_actions(&actions) {
        Some(mut next) => {
            next.entry = buffer;
            next.entry_committed = true;
            next
        }
        None => state.with_hint(format!("`{}` does not apply here", candidate.name)),
    }
}

fn run_backspace(state: AppState) -> AppState {
    let mut buffer = state.entry.clone();
    buffer.pop();
    if !buffer.is_empty() {
        return commit_run(buffer, state);
    }
    let mut next = match (
        state.entry_committed,
        state.apply_actions(&[Action::Delete]),
    ) {
        (true, Some(deleted)) => deleted,
        _ => state,
    };
    next.clear_entry();
    next
}

fn name_binder(text: String, state: AppState, append: bool) -> AppState {
    let mut buffer = if append {
        state.entry.clone()
    } else {
        String::new()
    };
    buffer.push_str(&text);

    let digits: String = buffer.chars().filter(char::is_ascii_digit).collect();
    let Ok(id) = digits.parse::<u64>() else {
        let mut next = state;
        next.slot = Slot::BinderName;
        next.entry = buffer;
        next.entry_committed = false;
        return next
            .with_hint("pre-Phase-5 a binder is identified by the digits in its name, as in `x0`");
    };


    if let Some(conflict) = state.rename_conflict(Id::new(id)) {
        let mut next = state;
        next.slot = Slot::BinderName;
        next.entry = buffer;
        next.entry_committed = false;
        return next.with_hint(conflict.explain());
    }

    match state.apply_actions(&[Action::SetBinderId(Id::new(id))]) {
        Some(mut next) => {
            next.slot = Slot::BinderName;
            next.entry = buffer;
            next.entry_committed = true;
            next
        }


        None => {
            let mut next = state;
            next.slot = Slot::BinderName;
            next.entry = buffer;
            next.entry_committed = false;
            next.with_hint(format!("x{id} would leave a reference unbound here"))
        }
    }
}

fn set_ann(buffer: String, state: AppState) -> AppState {
    let ty = annot::parse(&buffer);
    match state.apply_actions(&[Action::SetAnn(ty.clone())]) {
        Some(mut next) => {
            next.slot = Slot::Annotation;
            next.entry = buffer;
            next.entry_committed = true;
            next
        }


        None => {
            let mut next = state;
            next.entry = buffer;
            next.entry_committed = false;
            next.with_hint(format!("`{ty}` would leave the body untypable"))
        }
    }
}


fn wrap(state: AppState, prec: Prec, action: Action, what: &str) -> AppState {
    let mut actions = state.climb_actions(prec);
    actions.push(action);
    apply_or_hint(state, &actions, &format!("{what} does not apply here"))
}

fn binder(state: AppState, action: Action, pending: Option<String>, what: &str) -> AppState {
    let mut actions = state.climb_actions(PREC_BINDER);
    actions.push(action);
    let Some(next) = state.apply_actions(&actions) else {
        return state.with_hint(format!("{what} does not apply here"));
    };
    let Some(mut binder) = next.focus_binder() else {
        return next.with_hint(format!("{what} was built but its binder is unreachable"));
    };
    binder.slot = Slot::BinderName;
    match pending {
        Some(text) => name_binder(text, binder, false),
        None => binder,
    }
}

fn operator(op: Op, state: AppState) -> AppState {
    wrap(
        state,
        op_prec(op),
        Action::ConstructBinOp(op),
        "this operator",
    )
}


fn to_annotation(state: AppState) -> AppState {
    if !matches!(state.focus(), Exp::Lam(..)) {
        return state.with_hint("annotations live on lambdas");
    }
    let mut next = state;
    next.slot = Slot::Annotation;
    next.entry = String::new();
    next.entry_committed = false;
    next
}

fn to_body(state: AppState) -> AppState {
    match state.exit_slot_to_body() {
        Some(next) => next,
        None => state.with_hint("the cursor is not on a binder"),
    }
}

fn exit_and_reprocess(c: char, state: AppState) -> AppState {
    match state.exit_slot_to_body() {
        Some(next) => printable(c, next),
        None => state.with_hint(format!("`{c}` means nothing in this slot")),
    }
}


fn backspace(state: AppState) -> AppState {
    match state.slot {
        Slot::BinderName => {
            let mut buffer = state.entry.clone();
            buffer.pop();
            let mut next = state;
            next.entry = buffer;
            next.entry_committed = false;
            next
        }
        Slot::Annotation => {
            let mut buffer = state.entry.clone();
            buffer.pop();
            set_ann(buffer, state)
        }
        Slot::Node => {
            if !state.entry.is_empty() {
                return run_backspace(state);
            }
            match *state.focus() {
                Exp::Num(n) if n / 10 != 0 => apply_or_hint(
                    state,
                    &[Action::ConstructNum(n / 10)],
                    "a number does not fit here",
                ),
                Exp::EmptyHole(_) => or_hint(
                    state.move_up(),
                    state,
                    "the whole program is one empty hole",
                ),
                _ => delete(state),
            }
        }
    }
}

fn delete(state: AppState) -> AppState {
    if state.slot != Slot::Node {
        return state
            .with_hint("Del removes an expression — press ↑ to leave the binder slot first");
    }
    apply_or_hint(state, &[Action::Delete], "nothing to delete here")
}

fn enter(state: AppState) -> AppState {
    if state.slot == Slot::Node && matches!(state.focus(), Exp::NonEmptyHole(..)) {
        return match state.apply_actions(&[Action::Finish]) {
            Some(next) => next,
            None => {
                let expected = state.expected_ty();
                state.with_hint(format!("does not fit yet: expected {expected}"))
            }
        };
    }

    if state.slot == Slot::Node
        && let Some(steps) = state.enclosing_quarantine()
    {
        let ups = vec![Action::MoveParent; steps];
        let mut actions = ups.clone();
        actions.push(Action::Finish);
        return match state.apply_actions(&actions) {
            Some(next) => next,
            None => {


                let expected = match state.apply_actions(&ups) {
                    Some(wrapper) => wrapper.expected_ty().to_string(),
                    None => "its context".to_string(),
                };
                state.with_hint(format!(
                    "the ⦇⦈ around the cursor does not fit yet: expected {expected}"
                ))
            }
        };
    }

    or_hint(state.move_to_hole(true), state, NOTHING_UNFINISHED)
}

const NOTHING_UNFINISHED: &str = "nothing unfinished: this program has no holes";


fn quit(mut state: AppState) -> AppState {
    state.quit = true;
    state
}

fn end_run(mut state: AppState) -> AppState {
    if state.entry.is_empty() {
        return state;
    }
    state.clear_entry();
    state
}

fn unbound(key: KeyEvent, state: AppState) -> AppState {
    state.with_hint(format!("{} is not bound", describe(key)))
}

fn or_hint(moved: Option<AppState>, state: AppState, why: &str) -> AppState {
    match moved {
        Some(next) => next,
        None => state.with_hint(why),
    }
}

fn apply_or_hint(state: AppState, actions: &[Action], why: &str) -> AppState {
    or_hint(state.apply_actions(actions), state, why)
}

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn describe(key: KeyEvent) -> String {
    let mut name = String::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        name.push_str("C-");
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        name.push_str("M-");
    }
    match key.code {
        KeyCode::Char(' ') => name.push_str("space"),
        KeyCode::Char(c) => name.push(c),
        other => name.push_str(&format!("{other:?}").to_lowercase()),
    }
    name
}

pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

pub fn ctrl(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::index_path;
    use nothing_core::examples;
    use nothing_core::render::render;

    fn type_chars(text: &str, state: AppState) -> AppState {
        text.chars()
            .fold(state, |state, c| handle_key(key(KeyCode::Char(c)), state))
    }

    fn typed(text: &str) -> String {
        render(&type_chars(text, AppState::empty()).program())
    }


    #[test]
    fn one_plus_two_is_three_keystrokes() {
        let state = type_chars("1+2", AppState::empty());
        assert_eq!(render(&state.program()), "1 + 2");
        assert_eq!(state.keystrokes(), 3);
        assert_eq!(state.actions().len(), 3, "no hidden actions");
    }


    #[test]
    fn digits_extend_the_focused_number() {
        assert_eq!(typed("427"), "427");
        assert_eq!(typed("1+23"), "1 + 23");


        let state = type_chars("1+2", AppState::empty());
        let state = handle_key(key(KeyCode::Up), state);
        assert_eq!(render(&type_chars("9", state).program()), "9");


        assert_eq!(typed("1+2 3"), "1 + ⦇2⦈ 3");
    }

    #[test]
    fn a_digit_typed_a_week_later_still_extends_the_number() {

        let state = type_chars("42", AppState::empty());
        let state = handle_key(key(KeyCode::Esc), state);
        assert_eq!(render(&type_chars("7", state).program()), "427");
    }

    #[test]
    fn tilde_negates_and_minus_stays_subtraction() {
        assert_eq!(typed("2~"), "-2");
        assert_eq!(typed("2~5"), "-25", "digits extend a negative number");
        assert_eq!(typed("1-2"), "1 - 2");
    }

    #[test]
    fn backspace_drops_one_digit_then_deletes() {
        let state = type_chars("427", AppState::empty());
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(render(&state.program()), "42");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(render(&state.program()), "4");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(render(&state.program()), "⦇⦈");
    }

    #[test]
    fn backspace_ascends_out_of_an_empty_hole_before_deleting_anything() {

        let state = type_chars("1+", AppState::empty());
        assert!(matches!(state.focus(), Exp::EmptyHole(_)));
        let up = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(render(&up.program()), "1 + ⦇⦈", "nothing destroyed yet");
        assert!(
            matches!(up.focus(), Exp::BinOp(..)),
            "the whole `+` is now selected"
        );
        let gone = handle_key(key(KeyCode::Backspace), up);
        assert_eq!(render(&gone.program()), "⦇⦈");
    }

    #[test]
    fn backspace_un_types_a_name_one_character_at_a_time() {
        let state = type_chars("\\x0:n.x0", AppState::empty());
        assert_eq!(render(&state.program()), "λx0:Num. x0");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(state.entry, "x", "one character of the run is gone");
        assert_eq!(
            render(&state.program()),
            "λx0:Num. x0",
            "and `x` still names x0"
        );
        let state = handle_key(key(KeyCode::Backspace), state);
        assert!(state.entry.is_empty());
        assert_eq!(
            render(&state.program()),
            "λx0:Num. ⦇⦈",
            "the run wrote nothing"
        );
    }

    #[test]
    fn backspace_in_the_annotation_slot_drops_a_token() {
        let state = type_chars("\\x0:n>n", AppState::empty());
        assert_eq!(render(&state.program()), "λx0:Num -> Num. ⦇⦈");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(render(&state.program()), "λx0:Num -> ?. ⦇⦈");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(render(&state.program()), "λx0:Num. ⦇⦈");
        assert_eq!(state.slot, Slot::Annotation, "still annotating");
    }

    #[test]
    fn delete_replaces_the_focus_with_a_hole() {
        let state = type_chars("1+2", AppState::empty());
        let state = handle_key(key(KeyCode::Delete), state);
        assert_eq!(render(&state.program()), "1 + ⦇⦈");
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Delete), state);
        assert_eq!(render(&state.program()), "⦇⦈");
    }

    #[test]
    fn enter_jumps_to_the_next_hole_when_there_is_nothing_to_finish() {
        let state = type_chars("\\x0:n.?", AppState::empty());
        let state = handle_key(key(KeyCode::Enter), state);
        assert!(matches!(state.focus(), Exp::EmptyHole(_)));
        assert_eq!(index_path(state.zipper()), vec![0, 1], "the then-branch");
    }


    #[test]
    fn a_name_run_commits_live_and_refines() {


        let state = type_chars("\\x0:n.\\x1:n.", AppState::empty());
        let after_x = type_chars("x", state);
        assert!(
            matches!(after_x.focus(), Exp::Var(_)),
            "the first keystroke of a run already wrote a variable"
        );
        let after_x1 = type_chars("1", after_x);
        assert_eq!(
            render(&after_x1.program()),
            "λx0:Num. λx1:Num. x1",
            "the second keystroke re-derived the commit"
        );
        assert_eq!(after_x1.entry, "x1");
    }

    #[test]
    fn an_unmatched_name_leaves_the_program_alone() {
        let state = type_chars("\\x0:n.", AppState::empty());
        let before = state.program();
        let after = type_chars("zz", state);
        assert_eq!(after.program(), before, "a free variable has no meaning");
        assert_eq!(after.entry, "zz");
        assert!(!after.entry_committed);
        assert!(after.hint.is_some(), "and the status line says so");
    }

    #[test]
    fn an_unresolved_run_becomes_the_new_binders_name() {


        let state = type_chars("total7;", AppState::empty());
        assert_eq!(render(&state.program()), "let x7 = ⦇⦈ in ⦇⦈");
        assert_eq!(state.slot, Slot::BinderName);
        assert_eq!(state.entry, "total7");
    }

    #[test]
    fn true_and_false_are_candidates_not_keys() {
        assert_eq!(typed("t"), "true");
        assert_eq!(typed("f"), "false");
        assert_eq!(typed("?t"), "if true then ⦇⦈ else ⦇⦈");
    }


    #[test]
    fn operators_climb_so_left_to_right_typing_means_what_it_says() {
        assert_eq!(typed("1*2+3"), "1 * 2 + 3");
        assert_eq!(typed("1+2*3"), "1 + 2 * 3");
        assert_eq!(typed("1+2+3"), "1 + 2 + 3");


        assert_eq!(typed("1<2=3"), "⦇1 < 2⦈ == 3");
    }

    #[test]
    fn climbing_never_crosses_a_binder_or_a_conditional() {

        let state = type_chars("?t", AppState::empty());
        let state = handle_key(key(KeyCode::Tab), state);
        assert_eq!(
            render(&type_chars("1+2", state).program()),
            "if true then 1 + 2 else ⦇⦈"
        );
        assert_eq!(typed("\\x0:n.1+2"), "λx0:Num. 1 + 2");
    }

    #[test]
    fn an_empty_hole_wraps_in_place_rather_than_climbing() {


        assert_eq!(typed("1+*2"), "1 + 2 * ⦇⦈");
    }

    #[test]
    fn application_climbs_left_associatively() {

        let state = type_chars("\\x0:n>n>n.x0 1 2", AppState::empty());
        assert_eq!(render(&state.program()), "λx0:Num -> Num -> Num. x0 1 2");
    }

    #[test]
    fn every_form_key_builds_its_form() {


        assert_eq!(typed("1 "), "⦇1⦈ ⦇⦈");
        assert_eq!(typed("\\"), "λx0:?. ⦇⦈");
        assert_eq!(typed("1?"), "if ⦇1⦈ then ⦇⦈ else ⦇⦈");
        assert_eq!(typed("1;"), "let x0 = 1 in ⦇⦈");
        assert_eq!(typed("1,2"), "(1, 2)");
        assert_eq!(typed("1["), "fst ⦇1⦈");
        assert_eq!(typed("1]"), "snd ⦇1⦈");
        assert_eq!(typed("1!"), "⦇1⦈");
    }


    #[test]
    fn a_lambda_is_named_and_annotated_from_the_slots() {
        let state = type_chars("\\", AppState::empty());
        assert_eq!(state.slot, Slot::BinderName, "λ lands on the name");
        let state = type_chars("x0", state);
        let state = type_chars(":", state);
        assert_eq!(state.slot, Slot::Annotation);
        let state = type_chars("n>n", state);
        assert_eq!(render(&state.program()), "λx0:Num -> Num. ⦇⦈");
        let state = type_chars(".", state);
        assert_eq!(state.slot, Slot::Node);
        assert!(matches!(state.focus(), Exp::EmptyHole(_)));
    }

    #[test]
    fn the_annotation_slot_commits_on_every_keystroke() {
        let state = type_chars("\\x0:n", AppState::empty());
        assert_eq!(render(&state.program()), "λx0:Num. ⦇⦈");
        let state = type_chars(">", state);
        assert_eq!(render(&state.program()), "λx0:Num -> ?. ⦇⦈");
        let state = type_chars("n", state);
        assert_eq!(render(&state.program()), "λx0:Num -> Num. ⦇⦈");
    }

    #[test]
    fn an_annotation_that_would_break_the_body_declines_visibly() {


        let state = type_chars("\\x0:.x0+1", AppState::empty());
        assert_eq!(render(&state.program()), "λx0:?. x0 + 1");
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Up), state);
        let state = type_chars(":b", state);
        assert_eq!(
            render(&state.program()),
            "λx0:?. x0 + 1",
            "the program is untouched"
        );
        assert_eq!(state.slot, Slot::Annotation, "the slot stays open");
        assert!(state.hint.unwrap().contains("Bool"));
    }

    #[test]
    fn a_let_names_then_binds_then_bodies() {
        let state = type_chars(";x0=1", AppState::empty());
        assert_eq!(render(&state.program()), "let x0 = 1 in ⦇⦈");

        assert_eq!(index_path(state.zipper()), vec![0]);
    }

    #[test]
    fn a_character_a_slot_does_not_understand_exits_and_is_reprocessed() {


        let state = type_chars("\\x0:n.1", AppState::empty());
        let state = handle_key(key(KeyCode::Up), state);
        let state = type_chars(":", state);
        let state = type_chars("+", state);
        assert_eq!(render(&state.program()), "λx0:Num. 1 + ⦇⦈");
    }


    #[test]
    fn a_type_inconsistent_entry_is_quarantined_rather_than_refused() {

        assert_eq!(typed("t<"), "⦇true⦈ < ⦇⦈");
        assert_eq!(typed("1 "), "⦇1⦈ ⦇⦈");
    }

    #[test]
    fn a_non_empty_hole_is_transparent_to_typing() {
        let state = type_chars("1!", AppState::empty());
        assert_eq!(render(&state.program()), "⦇1⦈");


        let state = type_chars("2", state);
        assert_eq!(render(&state.program()), "⦇12⦈");

        let state = type_chars("!", state);
        assert_eq!(render(&state.program()), "⦇⦇12⦈⦈");
    }

    #[test]
    fn enter_finishes_a_quarantined_expression_that_now_fits() {

        let state = AppState::new(examples::add_with_non_empty_hole());


        let state = handle_key(key(KeyCode::Tab), state);
        assert!(matches!(state.focus(), Exp::NonEmptyHole(..)));

        let refused = handle_key(key(KeyCode::Enter), state.clone());
        assert!(
            matches!(refused.focus(), Exp::NonEmptyHole(..)),
            "true still does not fit"
        );
        assert!(refused.hint.unwrap().contains("does not fit yet"));

        let state = type_chars("2", state);
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Enter), state);
        assert_eq!(render(&state.program()), "1 + 2");
    }

    #[test]
    fn enter_finishes_the_quarantine_the_cursor_is_inside() {


        let state = AppState::new(examples::add_with_non_empty_hole());
        let state = handle_key(key(KeyCode::Tab), state);
        let state = type_chars("2", state);
        assert_eq!(render(&state.program()), "1 + ⦇2⦈");
        assert!(matches!(state.focus(), Exp::Num(2)), "inside the wrapper");

        let finished = handle_key(key(KeyCode::Enter), state.clone());
        assert_eq!(render(&finished.program()), "1 + 2", "one key, not three");
        assert!(matches!(finished.focus(), Exp::Num(2)), "cursor kept");


        let walked = handle_key(
            key(KeyCode::Enter),
            handle_key(key(KeyCode::Up), state.clone()),
        );
        assert_eq!(render(&walked.program()), render(&finished.program()));
        assert_eq!(finished.keystrokes() + 1, walked.keystrokes());
    }

    #[test]
    fn enter_inside_a_quarantine_that_does_not_fit_says_so_instead_of_jumping() {


        let state = type_chars("1?", AppState::empty());
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Down), state);
        let state = handle_key(key(KeyCode::Down), state);
        assert!(matches!(state.focus(), Exp::Num(1)));
        assert_eq!(state.enclosing_quarantine(), Some(1));

        let after = handle_key(key(KeyCode::Enter), state.clone());
        assert_eq!(after.program(), state.program(), "nothing changed");
        assert_eq!(
            index_path(after.zipper()),
            index_path(state.zipper()),
            "and the cursor did not teleport to an unrelated hole"
        );
        let hint = after.hint.expect("Enter must say why");
        assert!(hint.contains("does not fit yet"), "{hint}");
        assert!(hint.contains("Bool"), "and what it wanted: {hint}");
    }

    #[test]
    fn tab_walks_quarantines_too_and_says_when_nothing_is_left() {


        let state = AppState::new(examples::add_with_non_empty_hole());
        let tabbed = handle_key(key(KeyCode::Tab), state);
        assert!(
            matches!(tabbed.focus(), Exp::NonEmptyHole(..)),
            "Tab must reach the one unfinished thing in the program"
        );
        assert_eq!(tabbed.hint, None);


        let done = AppState::new(examples::increment_applied());
        let stuck = handle_key(key(KeyCode::Tab), done.clone());
        assert_eq!(stuck.program(), done.program());
        assert_eq!(stuck.hint.as_deref(), Some(NOTHING_UNFINISHED));
    }


    #[test]
    fn renaming_a_binder_onto_an_id_already_in_scope_is_declined() {


        let state = type_chars("\\x0:n.\\x1:.x0+1", AppState::empty());
        assert_eq!(render(&state.program()), "λx0:Num. λx1:?. x0 + 1");


        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Up), state);
        assert!(matches!(state.focus(), Exp::Lam(..)));
        let state = handle_key(key(KeyCode::Down), state);
        assert_eq!(state.slot, Slot::BinderName);

        let after = type_chars("x0", state.clone());
        assert_eq!(
            render(&after.program()),
            "λx0:Num. λx1:?. x0 + 1",
            "the program must be untouched"
        );
        assert_eq!(after.slot, Slot::BinderName, "the slot stays open");
        let hint = after.hint.expect("the warning is live, not silent");
        assert!(hint.contains("capture"), "{hint}");
        assert!(hint.contains("x0"), "{hint}");
    }

    #[test]
    fn naming_a_binder_what_it_is_already_called_is_not_a_capture() {


        let state = type_chars("\\x0:n.\\x1:.x0+1", AppState::empty());
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Down), state);

        let same = type_chars("x1", state.clone());
        assert_eq!(render(&same.program()), "λx0:Num. λx1:?. x0 + 1");
        assert_eq!(same.hint, None, "renaming x1 to x1 changes nothing");

        let fresh = type_chars("x7", state);
        assert_eq!(
            render(&fresh.program()),
            "λx0:Num. λx7:?. x0 + 1",
            "an id nothing refers to is free to take"
        );
    }

    #[test]
    fn a_binder_whose_reference_would_be_orphaned_still_declines() {


        let state = type_chars("\\x1:n.x1", AppState::empty());
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Down), state);
        let after = type_chars("x2", state);
        assert_eq!(render(&after.program()), "λx1:Num. x1");
        assert!(after.hint.unwrap().contains("unbound"));
    }


    #[test]
    fn one_undo_undoes_one_keystroke_however_many_actions_it_expanded_to() {


        let state = type_chars("1<2", AppState::empty());
        let before = state.program();
        let with_if = type_chars("?", state);
        assert_eq!(render(&with_if.program()), "if 1 < 2 then ⦇⦈ else ⦇⦈");

        let undone = handle_key(ctrl(KeyCode::Char('z')), with_if.clone());
        assert_eq!(undone.program(), before, "one C-z, one keystroke");

        let redone = handle_key(ctrl(KeyCode::Char('r')), undone);
        assert_eq!(redone.program(), with_if.program());
    }

    #[test]
    fn undo_walks_back_to_the_program_the_session_opened_on() {
        let start = AppState::factorial();
        let mut state = type_chars("1+2", start.clone());
        for _ in 0..3 {
            state = handle_key(ctrl(KeyCode::Char('z')), state);
        }
        assert_eq!(state.program(), start.program());
        let stuck = handle_key(ctrl(KeyCode::Char('z')), state.clone());
        assert_eq!(stuck.program(), start.program());
        assert_eq!(stuck.hint.as_deref(), Some("nothing to undo"));
    }

    #[test]
    fn typing_after_an_undo_discards_the_redo_tail() {
        let state = type_chars("1+2", AppState::empty());
        let state = handle_key(ctrl(KeyCode::Char('z')), state);
        let state = type_chars("3", state);
        assert_eq!(render(&state.program()), "1 + 3");
        let state = handle_key(ctrl(KeyCode::Char('r')), state);
        assert_eq!(render(&state.program()), "1 + 3", "nothing to redo");
    }

    #[test]
    fn undo_restores_the_slot_the_keystroke_started_in() {
        let state = type_chars("\\x", AppState::empty());
        assert_eq!(state.slot, Slot::BinderName);
        let state = type_chars("0", state);
        let state = handle_key(ctrl(KeyCode::Char('z')), state);
        assert_eq!(state.slot, Slot::BinderName, "still naming the binder");
    }


    #[test]
    fn every_printable_key_leaves_a_well_typed_program() {
        use nothing_core::typing::is_well_typed;
        let alphabet: Vec<char> = "0123456789abnxtf_+-*<= \\?;,[]!~:.".chars().collect();

        for start in [
            AppState::empty(),
            AppState::factorial(),
            AppState::new(examples::pair_and_project()),
        ] {
            for &c in &alphabet {
                let after = handle_key(key(KeyCode::Char(c)), start.clone());
                assert!(
                    is_well_typed(&after.program()),
                    "`{c}` produced {:?}",
                    after.program()
                );
            }
        }
    }

    #[test]
    fn no_key_ever_panics_on_any_example() {
        let alphabet: Vec<char> = "0123456789abxz_+-*<= \\?;,[]!~:.()>{}@#".chars().collect();
        let mut state = AppState::factorial();
        for (i, c) in alphabet.iter().cycle().take(200).enumerate() {
            state = handle_key(key(KeyCode::Char(*c)), state);
            if i % 7 == 0 {
                state = handle_key(key(KeyCode::Tab), state);
            }
            if i % 11 == 0 {
                state = handle_key(ctrl(KeyCode::Char('z')), state);
            }
        }
        assert!(nothing_core::typing::is_well_typed(&state.program()));
    }


    #[test]
    fn control_q_quits_and_nothing_else_does() {
        let state = AppState::factorial();
        assert!(handle_key(ctrl(KeyCode::Char('q')), state.clone()).quit);
        for code in [
            KeyCode::Char('q'),
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Tab,
            KeyCode::Esc,
            KeyCode::Char('x'),
        ] {
            assert!(!handle_key(key(code), state.clone()).quit, "{code:?} quit");
        }
    }

    #[test]
    fn the_arrows_walk_the_editor_tree() {
        let state = AppState::factorial();

        let name = handle_key(key(KeyCode::Down), state.clone());
        assert_eq!(name.slot, Slot::BinderName);

        let ann = handle_key(key(KeyCode::Right), name.clone());
        assert_eq!(ann.slot, Slot::Annotation);

        let body = handle_key(key(KeyCode::Right), ann);
        assert_eq!(body.slot, Slot::Node);
        assert_eq!(index_path(body.zipper()), vec![0]);

        let back = handle_key(key(KeyCode::Up), body);
        assert_eq!(index_path(back.zipper()), Vec::<usize>::new());
        assert_eq!(back.slot, Slot::Node);
    }

    #[test]
    fn a_declining_motion_changes_nothing_and_explains() {
        let root = AppState::factorial();
        let after = handle_key(key(KeyCode::Up), root.clone());
        assert_eq!(after.edit, root.edit);
        assert_eq!(after.slot, root.slot);
        assert_eq!(after.hint.as_deref(), Some("already at the root"));
    }

    #[test]
    fn tab_reaches_the_hole_and_shift_tab_comes_back() {
        let state = AppState::new(examples::add_with_empty_hole());
        let hole = handle_key(key(KeyCode::Tab), state.clone());
        assert_eq!(index_path(hole.zipper()), vec![1]);
        let same = handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT), hole);
        assert_eq!(index_path(same.zipper()), vec![1], "one hole, so it stays");
    }

    #[test]
    fn movement_never_changes_the_program() {
        let start = AppState::factorial();
        let program = start.program();
        let mut state = start;
        for code in [
            KeyCode::Down,
            KeyCode::Right,
            KeyCode::Right,
            KeyCode::Tab,
            KeyCode::Left,
            KeyCode::Up,
            KeyCode::Esc,
            KeyCode::BackTab,
        ] {
            state = handle_key(key(code), state);
            assert_eq!(state.program(), program, "{code:?} changed the program");
        }
    }

    #[test]
    fn an_unbound_key_is_inert_but_visible() {
        let state = AppState::factorial();
        let after = handle_key(key(KeyCode::Char('@')), state.clone());
        assert_eq!(after.edit, state.edit);
        assert_eq!(after.hint.as_deref(), Some("`@` is not bound here"));
    }

    #[test]
    fn a_key_release_is_ignored_entirely() {
        let state = AppState::factorial().with_hint("kept");
        let mut release = key(KeyCode::Down);
        release.kind = KeyEventKind::Release;
        let after = handle_key(release, state.clone());
        assert_eq!(after, state, "a release must not even clear the hint");
    }

    #[test]
    fn esc_ends_the_token_run() {
        let mut state = AppState::factorial();
        state.entry = "fac".to_string();
        let after = handle_key(key(KeyCode::Esc), state);
        assert!(after.entry.is_empty());
    }

    #[test]
    fn the_hint_lasts_exactly_one_keystroke() {
        let state = handle_key(key(KeyCode::Up), AppState::factorial());
        assert!(state.hint.is_some());
        let next = handle_key(key(KeyCode::Down), state);
        assert_eq!(next.hint, None);
    }
}