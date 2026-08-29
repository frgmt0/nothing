use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use nothing_action::act::Action;
use nothing_core::exp::{Exp, Op, Side};
use nothing_core::render::{PREC_APP, PREC_ATOM, PREC_BINDER, PREC_CONS, Prec, op_prec};

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
    if ctrl && key.code == KeyCode::Char('p') {
        return state.cycle_projection();
    }

    if state.string_open && string_run_takes(key) {
        let opened = state.open_keystroke();
        return string_key(key, state).close_keystroke(opened);
    }

    let mut state = state;
    state.string_open = false;
    state.escape_armed = false;

    let opened = state.open_keystroke();
    dispatch(key, state).close_keystroke(opened)
}

fn string_run_takes(key: KeyEvent) -> bool {
    if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return false;
    }
    matches!(key.code, KeyCode::Char(_) | KeyCode::Backspace)
}

fn string_key(key: KeyEvent, state: AppState) -> AppState {
    match key.code {
        KeyCode::Backspace => string_backspace(state),
        KeyCode::Char(c) if state.escape_armed => {
            let mut next = state;
            next.escape_armed = false;
            match c {
                '"' => string_append("\"", next),
                '\\' => string_append("\\", next),
                other => string_append(&format!("\\{other}"), next),
            }
        }
        KeyCode::Char('"') => {
            let mut next = state;
            next.string_open = false;
            next
        }
        KeyCode::Char('\\') => {
            let mut next = state;
            next.escape_armed = true;
            next
        }
        KeyCode::Char(c) => string_append(&c.to_string(), state),
        _ => state,
    }
}

fn string_append(text: &str, state: AppState) -> AppState {
    if matches!(state.focus(), Exp::NonEmptyHole(..)) {
        return match state.apply_actions(&[Action::MoveChild(0)]) {
            Some(mut inner) => {
                inner.string_open = state.string_open;
                inner.escape_armed = state.escape_armed;
                string_append(text, inner)
            }
            None => state.with_hint("cannot look inside this hole"),
        };
    }
    let mut written = focused_string(&state);
    written.push_str(text);
    let open = state.string_open;
    let armed = state.escape_armed;
    match state.apply_actions(&[Action::ConstructStr(written)]) {
        Some(mut next) => {
            next.string_open = open;
            next.escape_armed = armed;
            next
        }
        None => state.with_hint("a string does not fit here"),
    }
}

fn string_backspace(state: AppState) -> AppState {
    if state.escape_armed {
        let mut next = state;
        next.escape_armed = false;
        return next;
    }
    if matches!(state.focus(), Exp::NonEmptyHole(..)) {
        return match state.apply_actions(&[Action::MoveChild(0)]) {
            Some(mut inner) => {
                inner.string_open = state.string_open;
                string_backspace(inner)
            }
            None => state.with_hint("cannot look inside this hole"),
        };
    }
    let mut written = focused_string(&state);
    if written.pop().is_none() {
        let mut next = match state.apply_actions(&[Action::Delete]) {
            Some(next) => next,
            None => state,
        };
        next.clear_entry();
        return next;
    }
    let open = state.string_open;
    match state.apply_actions(&[Action::ConstructStr(written)]) {
        Some(mut next) => {
            next.string_open = open;
            next
        }
        None => state.with_hint("a string does not fit here"),
    }
}

fn focused_string(state: &AppState) -> String {
    match state.focus() {
        Exp::Str(text) => text.clone(),
        _ => String::new(),
    }
}

fn open_string(state: AppState) -> AppState {
    if matches!(state.focus(), Exp::Str(_)) {
        let mut next = state;
        next.string_open = true;
        next.escape_armed = false;
        return next;
    }
    match state.apply_actions(&[Action::ConstructStr(String::new())]) {
        Some(mut next) => {
            next.string_open = true;
            next.escape_armed = false;
            next
        }
        None => state.with_hint("a string does not fit here"),
    }
}

fn dispatch(key: KeyEvent, state: AppState) -> AppState {
    if let Some(next) = crate::projection::active(&state).handle_key(key, state.clone()) {
        return next;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match (key.code, ctrl) {
        (KeyCode::Char('q'), true) => quit(state),

        (KeyCode::Down, true) => or_hint(
            to_node(state.apply_actions(&[Action::MoveNextDef])),
            state,
            "this is the last definition",
        ),
        (KeyCode::Up, true) => or_hint(
            to_node(state.apply_actions(&[Action::MovePrevDef])),
            state,
            "this is the first definition",
        ),
        (KeyCode::Char('n'), true) => add_row(state),
        (KeyCode::Char('d'), true) => drop_row(state),
        (KeyCode::Left, true) => move_field(state, Action::MoveFieldPrev, "earlier"),
        (KeyCode::Right, true) => move_field(state, Action::MoveFieldNext, "later"),
        (KeyCode::Char('l'), true) => to_def_name(state),
        (KeyCode::Char('t'), true) => to_def_ann(state),

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
        Slot::DefName => def_name_key(c, state),
        Slot::DefAnn => def_ann_key(c, state),
        Slot::FieldName => field_name_key(c, state),
        Slot::FieldPick => field_pick_key(c, state),
        Slot::ConstructorName => constructor_name_key(c, state),
        Slot::ConstructorPick => constructor_pick_key(c, state),
        Slot::Node => node_key(c, state),
    }
}

fn add_row(state: AppState) -> AppState {
    if state.in_record() {
        return match state.apply_actions(&[Action::AddField]) {
            Some(next) => match next.field_name_target() {
                Some(named) => named,
                None => next,
            },
            None => state.with_hint("a field cannot be added here"),
        };
    }
    if state.in_match() {
        return match state.apply_actions(&[Action::AddArm]) {
            Some(next) => match next.constructor_name_target() {
                Some(named) => named,
                None => next,
            },
            None => state.with_hint("an arm cannot be added here"),
        };
    }
    new_definition(state)
}

fn drop_row(state: AppState) -> AppState {
    if state.zipper().record_field_id().is_some() {
        return or_hint(
            to_node(state.apply_actions(&[Action::RemoveField])),
            state,
            "this field cannot be dropped",
        );
    }
    if state.zipper().arm_constructor_id().is_some() {
        return or_hint(
            to_node(state.apply_actions(&[Action::RemoveArm])),
            state,
            "something still injects this case, so the arm has to stay",
        );
    }
    or_hint(
        to_node(state.apply_actions(&[Action::DeleteDefinition])),
        state,
        "a document keeps at least one definition",
    )
}

fn move_field(state: AppState, action: Action, which: &str) -> AppState {
    let slot = state.slot;
    match state.apply_actions(&[action]) {
        Some(mut next) => {
            if slot == Slot::FieldName {
                next.slot = Slot::FieldName;
            }
            next
        }
        None => state.with_hint(format!("this field cannot move any {which}")),
    }
}

fn to_node(next: Option<AppState>) -> Option<AppState> {
    next.map(|mut state| {
        state.slot = Slot::Node;
        state.clear_entry();
        state
    })
}

fn new_definition(state: AppState) -> AppState {
    match to_node(state.apply_actions(&[Action::CreateDefinition])) {
        Some(next) => to_def_name(next),
        None => state.with_hint("a definition cannot be created here"),
    }
}

fn to_def_name(state: AppState) -> AppState {
    let mut next = state;
    next.slot = Slot::DefName;
    next.entry = String::new();
    next.entry_committed = false;
    next
}

fn to_def_ann(state: AppState) -> AppState {
    let mut next = state;
    next.slot = Slot::DefAnn;
    next.entry = String::new();
    next.entry_committed = false;
    next
}

fn name_definition(text: String, state: AppState, append: bool) -> AppState {
    let mut buffer = if append {
        state.entry.clone()
    } else {
        String::new()
    };
    buffer.push_str(&text);
    let id = state.definition_id();
    let mut next = state
        .apply_actions(&[Action::Rename(id, buffer.clone())])
        .expect("a rename is a name-table write: it cannot fail");
    next.slot = Slot::DefName;
    next.entry = buffer;
    next.entry_committed = true;
    next
}

fn set_def_ann(buffer: String, state: AppState) -> AppState {
    let ty = annot::parse(&buffer);
    match state.apply_actions(&[Action::SetDefAnn(ty.clone())]) {
        Some(mut next) => {
            next.slot = Slot::DefAnn;
            next.entry = buffer;
            next.entry_committed = true;
            next
        }
        None => {
            let mut next = state;
            next.entry = buffer;
            next.entry_committed = false;
            next.with_hint(format!("`{ty}` does not describe this definition"))
        }
    }
}

fn def_name_key(c: char, state: AppState) -> AppState {
    match c {
        ':' => to_def_ann(state),
        '.' => leave_def_slot(state),
        c if is_name_char(c) => name_definition(c.to_string(), state, true),
        _ => leave_def_slot(state).with_hint(format!("`{c}` means nothing in a definition name")),
    }
}

fn def_ann_key(c: char, state: AppState) -> AppState {
    if c == '.' {
        return leave_def_slot(state);
    }
    if c == ':' {
        return state.with_hint("already in the definition type slot");
    }
    match annot::accept(&state.entry, c) {
        Accept::Ignore => state.with_hint("there is no `(` to close"),
        Accept::Exit => {
            leave_def_slot(state).with_hint(format!("`{c}` means nothing in a definition type"))
        }
        Accept::Append | Accept::Swallow => {
            let mut buffer = state.entry.clone();
            buffer.push(c);
            set_def_ann(buffer, state)
        }
    }
}

fn leave_def_slot(state: AppState) -> AppState {
    let mut next = state;
    next.slot = Slot::Node;
    next.clear_entry();
    next
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
        '&' => operator(Op::Concat, state),

        '"' => open_string(state),

        ' ' => wrap(state, PREC_APP, Action::ConstructAp, "application"),
        '\\' => binder(state, Action::ConstructLam, pending, "λ"),
        '?' => wrap(state, PREC_BINDER, Action::ConstructIf, "if"),
        ';' => binder(state, Action::ConstructLet, pending, "let"),
        ',' => wrap(state, PREC_BINDER, Action::ConstructPair, "pair"),
        '[' => wrap(state, PREC_APP, Action::ConstructProj(Side::L), "fst"),
        ']' => wrap(state, PREC_APP, Action::ConstructProj(Side::R), "snd"),
        '/' => wrap(state, PREC_APP, Action::ConstructFold, "fold"),

        '!' => wrap(
            state,
            PREC_ATOM,
            Action::ConstructNonEmptyHole,
            "quarantine",
        ),

        '~' => negate(state),
        ':' if matches!(state.focus(), Exp::Lam(..)) => to_annotation(state),
        ':' => wrap(state, PREC_CONS, Action::ConstructCons, "cons"),
        '{' => record(state),
        '`' => inject(state),
        '|' => match_(state),
        '.' => project(state),
        _ => state.with_hint(format!("`{c}` is not bound here")),
    }
}

fn record(state: AppState) -> AppState {
    let mut actions = state.climb_actions(PREC_ATOM);
    actions.push(Action::ConstructRecord);
    let Some(next) = state.apply_actions(&actions) else {
        return state.with_hint("a record does not fit here");
    };
    match next.field_name_target() {
        Some(named) => named,
        None => next,
    }
}

fn inject(state: AppState) -> AppState {
    let mut actions = state.climb_actions(PREC_ATOM);
    actions.push(Action::ConstructInj);
    let Some(next) = state.apply_actions(&actions) else {
        return state.with_hint("an injection does not fit here");
    };
    let mut next = match next.focus() {
        Exp::Inj(..) => next,
        _ => next
            .apply_actions(&[Action::MoveParent])
            .expect("an injection has a parent when the cursor is in its payload"),
    };
    next.slot = Slot::ConstructorPick;
    next.entry = String::new();
    next.entry_committed = false;
    next
}

fn match_(state: AppState) -> AppState {
    let mut actions = state.climb_actions(PREC_BINDER);
    actions.push(Action::ConstructMatch);
    match state.apply_actions(&actions) {
        Some(next) => next,
        None => state.with_hint("a match does not fit here"),
    }
}

fn constructor_name_key(c: char, state: AppState) -> AppState {
    match c {
        '=' => leave_constructor_slot(state),
        c if is_name_char(c) => name_constructor(c.to_string(), state, true),
        _ => exit_constructor_and_reprocess(c, state),
    }
}

fn constructor_pick_key(c: char, state: AppState) -> AppState {
    match c {
        '=' => leave_constructor_slot(state),
        c if is_name_char(c) => pick_constructor(c.to_string(), state, true),
        _ => exit_constructor_and_reprocess(c, state),
    }
}

fn leave_constructor_slot(state: AppState) -> AppState {
    let mut next = state;
    if next.slot == Slot::ConstructorPick
        && let Some(into) = next.apply_actions(&[Action::MoveChild(0)])
    {
        next = into;
    }
    next.slot = Slot::Node;
    next.clear_entry();
    next
}

fn exit_constructor_and_reprocess(c: char, state: AppState) -> AppState {
    printable(c, leave_constructor_slot(state))
}

fn name_constructor(text: String, state: AppState, append: bool) -> AppState {
    let mut buffer = if append {
        state.entry.clone()
    } else {
        String::new()
    };
    buffer.push_str(&text);

    let Some(id) = state.constructor_slot_id() else {
        return state.with_hint("the cursor is not on a constructor");
    };

    let mut next = state
        .apply_actions(&[Action::Rename(id, buffer.clone())])
        .expect("a rename is a name-table write: it cannot fail");
    next.slot = Slot::ConstructorName;
    next.entry = buffer;
    next.entry_committed = true;
    next
}

fn pick_constructor(text: String, state: AppState, append: bool) -> AppState {
    let mut buffer = if append {
        state.entry.clone()
    } else {
        String::new()
    };
    buffer.push_str(&text);

    let Some((id, name)) = complete::best_constructor(&state, &buffer) else {
        return rename_this_constructor(buffer, state);
    };

    match state.apply_actions(&[Action::SetConstructor(id)]) {
        Some(mut next) => {
            next.slot = Slot::ConstructorPick;
            next.entry = buffer;
            next.entry_committed = true;
            next
        }
        None => {
            let mut next = state;
            next.entry = buffer.clone();
            next.entry_committed = false;
            next.with_hint(format!("`{name}` does not fit here"))
        }
    }
}

fn rename_this_constructor(buffer: String, state: AppState) -> AppState {
    let Some(id) = state.constructor_slot_id() else {
        let mut next = state;
        next.entry = buffer.clone();
        next.entry_committed = false;
        return next.with_hint(format!("no constructor in view starts with `{buffer}`"));
    };
    let mut next = state
        .apply_actions(&[Action::Rename(id, buffer.clone())])
        .expect("a rename is a name-table write: it cannot fail");
    next.slot = Slot::ConstructorPick;
    next.entry = buffer;
    next.entry_committed = true;
    next
}

fn project(state: AppState) -> AppState {
    let Some((id, name)) = complete::best_field(&state, "") else {
        return state
            .with_hint("`.` names a field, and this document has no record to name one in");
    };
    match state.apply_actions(&[Action::ConstructField(id)]) {
        Some(mut next) => {
            next.slot = Slot::FieldPick;
            next.entry = String::new();
            next.entry_committed = false;
            next
        }
        None => state.with_hint(format!("`{name}` cannot be projected from this")),
    }
}

fn field_name_key(c: char, state: AppState) -> AppState {
    match c {
        '=' => leave_field_slot(state),
        c if is_name_char(c) => name_field(c.to_string(), state, true),
        _ => exit_field_and_reprocess(c, state),
    }
}

fn field_pick_key(c: char, state: AppState) -> AppState {
    match c {
        '=' => leave_field_slot(state),
        c if is_name_char(c) => pick_field(c.to_string(), state, true),
        _ => exit_field_and_reprocess(c, state),
    }
}

fn leave_field_slot(state: AppState) -> AppState {
    let mut next = state;
    next.slot = Slot::Node;
    next.clear_entry();
    next
}

fn exit_field_and_reprocess(c: char, state: AppState) -> AppState {
    printable(c, leave_field_slot(state))
}

fn name_field(text: String, state: AppState, append: bool) -> AppState {
    let mut buffer = if append {
        state.entry.clone()
    } else {
        String::new()
    };
    buffer.push_str(&text);

    let Some(id) = state.field_slot_id() else {
        return state.with_hint("the cursor is not on a field");
    };

    let mut next = state
        .apply_actions(&[Action::Rename(id, buffer.clone())])
        .expect("a rename is a name-table write: it cannot fail");
    next.slot = Slot::FieldName;
    next.entry = buffer;
    next.entry_committed = true;
    next
}

fn pick_field(text: String, state: AppState, append: bool) -> AppState {
    let mut buffer = if append {
        state.entry.clone()
    } else {
        String::new()
    };
    buffer.push_str(&text);

    let Some((id, name)) = complete::best_field(&state, &buffer) else {
        let mut next = state;
        next.entry = buffer.clone();
        next.entry_committed = false;
        return next.with_hint(format!("no field in view starts with `{buffer}`"));
    };

    match state.apply_actions(&[Action::SetField(id)]) {
        Some(mut next) => {
            next.slot = Slot::FieldPick;
            next.entry = buffer;
            next.entry_committed = true;
            next
        }
        None => {
            let mut next = state;
            next.entry = buffer;
            next.entry_committed = false;
            next.with_hint(format!("`{name}` is not a field of this value"))
        }
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

    let Some(id) = state.focus_binder_id() else {
        return state.with_hint("the cursor is not on a binder");
    };

    let mut next = state
        .apply_actions(&[Action::Rename(id, buffer.clone())])
        .expect("a rename is a name-table write: it cannot fail");
    next.slot = Slot::BinderName;
    next.entry = buffer;
    next.entry_committed = true;
    next
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
        Slot::DefName => {
            let mut buffer = state.entry.clone();
            buffer.pop();
            if !buffer.is_empty() {
                return name_definition(buffer, state, false);
            }
            let mut next = state;
            next.entry = buffer;
            next.entry_committed = false;
            next
        }
        Slot::DefAnn => {
            let mut buffer = state.entry.clone();
            buffer.pop();
            set_def_ann(buffer, state)
        }
        Slot::BinderName => {
            let mut buffer = state.entry.clone();
            buffer.pop();
            if !buffer.is_empty() {
                return name_binder(buffer, state, false);
            }
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
        Slot::FieldName => {
            let mut buffer = state.entry.clone();
            buffer.pop();
            if !buffer.is_empty() {
                return name_field(buffer, state, false);
            }
            let mut next = state;
            next.entry = buffer;
            next.entry_committed = false;
            next
        }
        Slot::FieldPick => {
            let mut buffer = state.entry.clone();
            buffer.pop();
            if !buffer.is_empty() {
                return pick_field(buffer, state, false);
            }
            let mut next = state;
            next.entry = buffer;
            next.entry_committed = false;
            next
        }
        Slot::ConstructorName => {
            let mut buffer = state.entry.clone();
            buffer.pop();
            if !buffer.is_empty() {
                return name_constructor(buffer, state, false);
            }
            let mut next = state;
            next.entry = buffer;
            next.entry_committed = false;
            next
        }
        Slot::ConstructorPick => {
            let mut buffer = state.entry.clone();
            buffer.pop();
            if !buffer.is_empty() {
                return pick_constructor(buffer, state, false);
            }
            let mut next = state;
            next.entry = buffer;
            next.entry_committed = false;
            next
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

    fn type_chars(text: &str, state: AppState) -> AppState {
        text.chars()
            .fold(state, |state, c| handle_key(key(KeyCode::Char(c)), state))
    }

    fn typed(text: &str) -> String {
        type_chars(text, AppState::empty()).text()
    }

    #[test]
    fn cons_is_a_colon_and_a_written_lambda_keeps_its_annotation_slot() {
        assert_eq!(typed("1:"), "1 :: ⦇⦈");
        assert_eq!(
            typed("1:2:n"),
            "1 :: ⦇2 :: nil⦈",
            "the tail of `1 :: _` wants a list, so a bare 2 is quarantined and typing goes on \
             inside the quarantine"
        );
        let finished = handle_key(key(KeyCode::Enter), type_chars("1:2:n", AppState::empty()));
        assert_eq!(
            finished.text(),
            "1 :: 2 :: nil",
            "and one Enter finishes it, because by then it fits"
        );
        assert_eq!(
            typed("1+2:"),
            "1 + 2 :: ⦇⦈",
            "cons binds looser than addition, so `+` is climbed out of"
        );
        assert_eq!(
            typed("1<2:"),
            "1 < ⦇2 :: ⦇⦈⦈",
            "and tighter than comparison, so `<` is not climbed out of and the list lands \
             where a number was wanted"
        );

        let lambda = type_chars("\\x0:n.x0", AppState::empty());
        let lambda = handle_key(key(KeyCode::Esc), lambda);
        let lambda = handle_key(key(KeyCode::Up), lambda);
        assert!(matches!(lambda.focus(), Exp::Lam(..)));
        let annotated = handle_key(key(KeyCode::Char(':')), lambda);
        assert_eq!(
            annotated.slot,
            Slot::Annotation,
            "`:` on a written lambda still opens its annotation slot"
        );
    }

    #[test]
    fn fold_and_nil_are_a_key_and_a_candidate() {
        assert_eq!(typed("/"), "fold ⦇⦈ ⦇⦈ ⦇⦈");
        assert_eq!(typed("n"), "nil", "nil is a candidate like true and false");
        assert_eq!(typed("/n"), "fold nil ⦇⦈ ⦇⦈");

        let state = type_chars("\\x0:[n./x0", AppState::empty());
        assert_eq!(state.text(), "λx0:List Num. fold x0 ⦇⦈ ⦇⦈");
        let state = handle_key(key(KeyCode::Tab), state);
        assert_eq!(
            state.expected_ty(),
            nothing_core::ty::Ty::Hole,
            "an unannotated fold accumulates whatever its seed is"
        );
    }

    #[test]
    fn the_expected_type_of_a_cons_tail_comes_from_the_head_that_was_typed() {
        let state = type_chars("1:", AppState::empty());
        assert_eq!(state.text(), "1 :: ⦇⦈");
        assert_eq!(
            state.expected_ty(),
            nothing_core::ty::Ty::List(Box::new(nothing_core::ty::Ty::Num)),
            "the hole after `1 ::` wants a list of numbers, and nobody said so"
        );

        let head = type_chars(":", AppState::empty());
        assert_eq!(head.text(), "⦇⦈ :: ⦇⦈");
        let quarantined = type_chars("t:1:n", AppState::empty());
        assert_eq!(
            quarantined.text(),
            "true :: ⦇1 :: nil⦈",
            "a list settles on its first element's type and quarantines the rest"
        );
        assert!(
            !handle_key(key(KeyCode::Enter), quarantined.clone()).finishes(),
            "and this one never finishes: a list of numbers is not a list of booleans"
        );
        assert!(nothing_core::typing::is_well_typed(&head.program()));
        assert!(nothing_core::typing::is_well_typed(&quarantined.program()));
    }

    #[test]
    fn a_string_run_takes_every_printable_key_as_text() {
        assert_eq!(typed("\"hello\""), "\"hello\"");
        assert_eq!(
            typed("\"a b + 1; ? , [ ] ! ~ : .\""),
            "\"a b + 1; ? , [ ] ! ~ : .\""
        );
        assert_eq!(typed("\"\""), "\"\"");

        let state = type_chars("\"hi", AppState::empty());
        assert!(
            state.string_open,
            "the run stays open until a closing quote"
        );
        assert_eq!(state.text(), "\"hi\"");
        assert_eq!(state.keystrokes(), 3);
    }

    #[test]
    fn the_only_two_escapes_are_the_quote_and_the_backslash() {
        assert_eq!(typed("\"a\\\"b\""), "\"a\\\"b\"");
        assert_eq!(typed("\"a\\\\b\""), "\"a\\\\b\"");
        assert_eq!(typed("\"a\\nb\""), "\"a\\\\nb\"");

        let armed = type_chars("\"a\\", AppState::empty());
        assert!(armed.escape_armed);
        assert_eq!(armed.text(), "\"a\"", "an armed escape has not landed yet");
        let disarmed = handle_key(key(KeyCode::Backspace), armed);
        assert!(!disarmed.escape_armed);
        assert_eq!(disarmed.text(), "\"a\"");
    }

    #[test]
    fn a_quote_reopens_a_finished_string_at_its_end() {
        let state = type_chars("\"hi\"", AppState::empty());
        assert!(!state.string_open);
        let state = type_chars("\" there", state);
        assert_eq!(state.text(), "\"hi there\"");
    }

    #[test]
    fn backspace_un_types_one_character_and_then_the_literal() {
        let state = type_chars("\"ab", AppState::empty());
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(state.text(), "\"a\"");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(state.text(), "\"\"");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(state.text(), "⦇⦈");
        assert!(!state.string_open);
    }

    #[test]
    fn every_other_key_closes_the_run_and_is_reprocessed() {
        let state = type_chars("\"a", AppState::empty());
        let state = handle_key(key(KeyCode::Esc), state);
        assert!(!state.string_open);
        assert_eq!(state.text(), "\"a\"");

        let state = type_chars("\"a", AppState::empty());
        let state = handle_key(key(KeyCode::Enter), state);
        assert!(!state.string_open);
        assert_eq!(state.text(), "\"a\"");

        let state = type_chars("\"a", AppState::empty());
        let state = handle_key(key(KeyCode::Up), state);
        assert!(!state.string_open);
    }

    #[test]
    fn a_string_run_survives_the_descent_into_a_quarantine() {
        let state = type_chars("1+", AppState::empty());
        let state = type_chars("\"ab", state);
        assert!(state.string_open);
        assert_eq!(state.text(), "1 + ⦇\"ab\"⦈");
    }

    #[test]
    fn joining_text_is_one_key_and_climbs_like_addition() {
        assert_eq!(typed("\"a\"&\"b\""), "\"a\" ++ \"b\"");
        assert_eq!(typed("\"a\"&\"b\"&\"c\""), "\"a\" ++ \"b\" ++ \"c\"");
        assert_eq!(typed("1&"), "⦇1⦈ ++ ⦇⦈");
    }

    #[test]
    fn a_string_annotation_is_one_key_in_the_annotation_slot() {
        assert_eq!(typed("\\n:s."), "λn:Str. ⦇⦈");
        assert_eq!(typed("\\n:str."), "λn:Str. ⦇⦈");
        assert_eq!(typed("\\n:s>n."), "λn:Str -> Num. ⦇⦈");
    }

    #[test]
    fn undo_takes_back_one_keystroke_of_a_string() {
        let state = type_chars("\"abc", AppState::empty());
        let state = handle_key(ctrl(KeyCode::Char('z')), state);
        assert_eq!(state.text(), "\"ab\"");
        assert!(state.string_open, "undo restores the run it was typed in");
    }

    #[test]
    fn one_plus_two_is_three_keystrokes() {
        let state = type_chars("1+2", AppState::empty());
        assert_eq!(state.text(), "1 + 2");
        assert_eq!(state.keystrokes(), 3);
        assert_eq!(state.actions().len(), 3, "no hidden actions");
    }

    #[test]
    fn digits_extend_the_focused_number() {
        assert_eq!(typed("427"), "427");
        assert_eq!(typed("1+23"), "1 + 23");

        let state = type_chars("1+2", AppState::empty());
        let state = handle_key(key(KeyCode::Up), state);
        assert_eq!(type_chars("9", state).text(), "9");

        assert_eq!(typed("1+2 3"), "1 + ⦇2⦈ 3");
    }

    #[test]
    fn a_digit_typed_a_week_later_still_extends_the_number() {
        let state = type_chars("42", AppState::empty());
        let state = handle_key(key(KeyCode::Esc), state);
        assert_eq!(type_chars("7", state).text(), "427");
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
        assert_eq!(state.text(), "42");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(state.text(), "4");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(state.text(), "⦇⦈");
    }

    #[test]
    fn backspace_ascends_out_of_an_empty_hole_before_deleting_anything() {
        let state = type_chars("1+", AppState::empty());
        assert!(matches!(state.focus(), Exp::EmptyHole(_)));
        let up = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(up.text(), "1 + ⦇⦈", "nothing destroyed yet");
        assert!(
            matches!(up.focus(), Exp::BinOp(..)),
            "the whole `+` is now selected"
        );
        let gone = handle_key(key(KeyCode::Backspace), up);
        assert_eq!(gone.text(), "⦇⦈");
    }

    #[test]
    fn backspace_un_types_a_name_one_character_at_a_time() {
        let state = type_chars("\\x0:n.x0", AppState::empty());
        assert_eq!(state.text(), "λx0:Num. x0");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(state.entry, "x", "one character of the run is gone");
        assert_eq!(state.text(), "λx0:Num. x0", "and `x` still names x0");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert!(state.entry.is_empty());
        assert_eq!(state.text(), "λx0:Num. ⦇⦈", "the run wrote nothing");
    }

    #[test]
    fn backspace_in_the_annotation_slot_drops_a_token() {
        let state = type_chars("\\x0:n>n", AppState::empty());
        assert_eq!(state.text(), "λx0:Num -> Num. ⦇⦈");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(state.text(), "λx0:Num -> ?. ⦇⦈");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(state.text(), "λx0:Num. ⦇⦈");
        assert_eq!(state.slot, Slot::Annotation, "still annotating");
    }

    #[test]
    fn delete_replaces_the_focus_with_a_hole() {
        let state = type_chars("1+2", AppState::empty());
        let state = handle_key(key(KeyCode::Delete), state);
        assert_eq!(state.text(), "1 + ⦇⦈");
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Delete), state);
        assert_eq!(state.text(), "⦇⦈");
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
            after_x1.text(),
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
        assert_eq!(state.text(), "let total7 = ⦇⦈ in ⦇⦈");
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

        assert_eq!(typed("1<2=3"), "1 < 2 == ⦇3⦈");
    }

    #[test]
    fn equality_compares_at_whichever_type_its_operands_have() {
        assert_eq!(typed("1=2"), "1 == 2");
        assert_eq!(typed("t=f"), "true == false");
        assert_eq!(typed("\"a\"=\"b\""), "\"a\" == \"b\"");
        assert_eq!(typed("1=t"), "1 == ⦇true⦈");
        assert_eq!(typed("\"a\"=1"), "\"a\" == ⦇1⦈");
    }

    #[test]
    fn climbing_never_crosses_a_binder_or_a_conditional() {
        let state = type_chars("?t", AppState::empty());
        let state = handle_key(key(KeyCode::Tab), state);
        assert_eq!(
            type_chars("1+2", state).text(),
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
        assert_eq!(state.text(), "λx0:Num -> Num -> Num. x0 1 2");
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
        assert_eq!(state.text(), "λx0:Num -> Num. ⦇⦈");
        let state = type_chars(".", state);
        assert_eq!(state.slot, Slot::Node);
        assert!(matches!(state.focus(), Exp::EmptyHole(_)));
    }

    #[test]
    fn the_annotation_slot_commits_on_every_keystroke() {
        let state = type_chars("\\x0:n", AppState::empty());
        assert_eq!(state.text(), "λx0:Num. ⦇⦈");
        let state = type_chars(">", state);
        assert_eq!(state.text(), "λx0:Num -> ?. ⦇⦈");
        let state = type_chars("n", state);
        assert_eq!(state.text(), "λx0:Num -> Num. ⦇⦈");
    }

    #[test]
    fn an_annotation_that_would_break_the_body_declines_visibly() {
        let state = type_chars("\\x0:.x0+1", AppState::empty());
        assert_eq!(state.text(), "λx0:?. x0 + 1");
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Up), state);
        let state = type_chars(":b", state);
        assert_eq!(state.text(), "λx0:?. x0 + 1", "the program is untouched");
        assert_eq!(state.slot, Slot::Annotation, "the slot stays open");
        assert!(state.hint.unwrap().contains("Bool"));
    }

    #[test]
    fn a_let_names_then_binds_then_bodies() {
        let state = type_chars(";x0=1", AppState::empty());
        assert_eq!(state.text(), "let x0 = 1 in ⦇⦈");

        assert_eq!(index_path(state.zipper()), vec![0]);
    }

    #[test]
    fn a_character_a_slot_does_not_understand_exits_and_is_reprocessed() {
        let state = type_chars("\\x0:n.1", AppState::empty());
        let state = handle_key(key(KeyCode::Up), state);
        let state = type_chars(":", state);
        let state = type_chars("+", state);
        assert_eq!(state.text(), "λx0:Num. 1 + ⦇⦈");
    }

    #[test]
    fn a_type_inconsistent_entry_is_quarantined_rather_than_refused() {
        assert_eq!(typed("t<"), "⦇true⦈ < ⦇⦈");
        assert_eq!(typed("1 "), "⦇1⦈ ⦇⦈");
    }

    #[test]
    fn a_non_empty_hole_is_transparent_to_typing() {
        let state = type_chars("1!", AppState::empty());
        assert_eq!(state.text(), "⦇1⦈");

        let state = type_chars("2", state);
        assert_eq!(state.text(), "⦇12⦈");

        let state = type_chars("!", state);
        assert_eq!(state.text(), "⦇⦇12⦈⦈");
    }

    #[test]
    fn enter_finishes_a_quarantined_expression_that_now_fits() {
        let state = AppState::with_names(examples::add_with_non_empty_hole(), examples::names());

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
        assert_eq!(state.text(), "1 + 2");
    }

    #[test]
    fn enter_finishes_the_quarantine_the_cursor_is_inside() {
        let state = AppState::with_names(examples::add_with_non_empty_hole(), examples::names());
        let state = handle_key(key(KeyCode::Tab), state);
        let state = type_chars("2", state);
        assert_eq!(state.text(), "1 + ⦇2⦈");
        assert!(matches!(state.focus(), Exp::Num(2)), "inside the wrapper");

        let finished = handle_key(key(KeyCode::Enter), state.clone());
        assert_eq!(finished.text(), "1 + 2", "one key, not three");
        assert!(matches!(finished.focus(), Exp::Num(2)), "cursor kept");

        let walked = handle_key(
            key(KeyCode::Enter),
            handle_key(key(KeyCode::Up), state.clone()),
        );
        assert_eq!(walked.text(), finished.text());
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
        let state = AppState::with_names(examples::add_with_non_empty_hole(), examples::names());
        let tabbed = handle_key(key(KeyCode::Tab), state);
        assert!(
            matches!(tabbed.focus(), Exp::NonEmptyHole(..)),
            "Tab must reach the one unfinished thing in the program"
        );
        assert_eq!(tabbed.hint, None);

        let done = AppState::with_names(examples::increment_applied(), examples::names());
        let stuck = handle_key(key(KeyCode::Tab), done.clone());
        assert_eq!(stuck.program(), done.program());
        assert_eq!(stuck.hint.as_deref(), Some(NOTHING_UNFINISHED));
    }

    #[test]
    fn two_binders_may_wear_the_same_display_name_without_capture() {
        let state = type_chars("\\x0:n.\\x1:.x0+1", AppState::empty());
        assert_eq!(state.text(), "λx0:Num. λx1:?. x0 + 1");
        let before = state.program();

        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Up), state);
        assert!(matches!(state.focus(), Exp::Lam(..)));
        let state = handle_key(key(KeyCode::Down), state);
        assert_eq!(state.slot, Slot::BinderName);

        let after = type_chars("x0", state);
        assert_eq!(
            after.text(),
            "λx0:Num. λx0:?. x0 + 1",
            "both binders are shown as x0"
        );
        assert_eq!(after.hint, None, "a rename is never refused");
        assert_eq!(
            after.program(),
            before,
            "renaming is a name-table write: the tree is untouched"
        );

        let (outer, inner, used) = match before {
            Exp::Lam(outer, _, body) => match *body {
                Exp::Lam(inner, _, body) => match *body {
                    Exp::BinOp(_, lhs, _) => match *lhs {
                        Exp::Var(used) => (outer, inner, used),
                        other => panic!("expected a variable, got {other:?}"),
                    },
                    other => panic!("expected an addition, got {other:?}"),
                },
                other => panic!("expected a lambda, got {other:?}"),
            },
            other => panic!("expected a lambda, got {other:?}"),
        };
        assert_ne!(outer, inner, "two bindings, two identities");
        assert_eq!(used, outer, "the body still refers to the outer binder");
    }

    #[test]
    fn naming_a_binder_what_it_is_already_called_is_not_a_capture() {
        let state = type_chars("\\x0:n.\\x1:.x0+1", AppState::empty());
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Down), state);

        let same = type_chars("x1", state.clone());
        assert_eq!(same.text(), "λx0:Num. λx1:?. x0 + 1");
        assert_eq!(same.hint, None, "renaming x1 to x1 changes nothing");

        let fresh = type_chars("x7", state);
        assert_eq!(
            fresh.text(),
            "λx0:Num. λx7:?. x0 + 1",
            "an id nothing refers to is free to take"
        );
    }

    #[test]
    fn renaming_a_binder_renames_every_reference_to_it_at_once() {
        let state = type_chars("\\x1:n.x1", AppState::empty());
        let before = state.program();
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Down), state);

        let after = type_chars("x2", state);
        assert_eq!(after.text(), "λx2:Num. x2", "the use follows the binder");
        assert_eq!(after.hint, None, "a rename cannot fail");
        assert_eq!(after.program(), before, "and it does not touch the tree");
    }

    #[test]
    fn one_undo_undoes_one_keystroke_however_many_actions_it_expanded_to() {
        let state = type_chars("1<2", AppState::empty());
        let before = state.program();
        let with_if = type_chars("?", state);
        assert_eq!(with_if.text(), "if 1 < 2 then ⦇⦈ else ⦇⦈");

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
        assert_eq!(state.text(), "1 + 3");
        let state = handle_key(ctrl(KeyCode::Char('r')), state);
        assert_eq!(state.text(), "1 + 3", "nothing to redo");
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
        let alphabet: Vec<char> = "0123456789abnsxtf_+-*<=& \\?;,[]!~:.\"".chars().collect();

        for start in [
            AppState::empty(),
            AppState::factorial(),
            AppState::with_names(examples::pair_and_project(), examples::names()),
        ] {
            for &c in &alphabet {
                let after = handle_key(key(KeyCode::Char(c)), start.clone());
                assert!(
                    after.edit.is_well_typed(),
                    "`{c}` produced {:?}",
                    after.edit.doc()
                );
            }
        }
    }

    #[test]
    fn no_key_ever_panics_on_any_example() {
        let alphabet: Vec<char> = "0123456789abxz_+-*<=& \\?;,[]!~:.()>{}@#\""
            .chars()
            .collect();
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

        let head = handle_key(key(KeyCode::Up), root.clone());
        assert_eq!(head.edit, root.edit);
        assert_eq!(head.slot, Slot::DefName);

        let after = handle_key(key(KeyCode::Up), head.clone());
        assert_eq!(after.edit, head.edit);
        assert_eq!(after.slot, head.slot);
        assert_eq!(after.hint.as_deref(), Some("already at the root"));
    }

    #[test]
    fn tab_reaches_the_hole_and_shift_tab_comes_back() {
        let state = AppState::with_names(examples::add_with_empty_hole(), examples::names());
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
        let head = handle_key(key(KeyCode::Up), AppState::factorial());
        let state = handle_key(key(KeyCode::Up), head);
        assert!(state.hint.is_some());
        let next = handle_key(key(KeyCode::Down), state);
        assert_eq!(next.hint, None);
    }

    fn record_of_two_fields() -> AppState {
        let state = type_chars("{x=1", AppState::empty());
        let state = handle_key(ctrl(KeyCode::Char('n')), state);
        let state = type_chars("y=2", state);
        let state = handle_key(key(KeyCode::Up), state);
        assert!(
            matches!(state.focus(), Exp::Record(fields) if fields.len() == 2),
            "the fixture must stand on a two-field record"
        );
        state
    }

    #[test]
    fn a_brace_writes_a_record_and_lands_in_its_field_name() {
        let fresh = type_chars("{", AppState::empty());
        assert_eq!(fresh.text(), "{f0 = ⦇⦈}");
        assert_eq!(
            fresh.slot,
            Slot::FieldName,
            "a brace leaves the cursor in the new field's name, as a lambda does in its binder's"
        );
        assert_eq!(fresh.entry, "", "and the name run starts empty");

        assert_eq!(
            typed("1{"),
            "{f0 = 1}",
            "it wraps what was already there, like every other form key"
        );
        assert_eq!(
            typed("{x=1"),
            "{x = 1}",
            "the name slot is free text and `=` leaves it for the value"
        );

        let wanted = nothing_core::ty::Ty::Record(vec![
            (
                nothing_core::exp::Id::from_u128(0x11),
                nothing_core::ty::Ty::Num,
            ),
            (
                nothing_core::exp::Id::from_u128(0x22),
                nothing_core::ty::Ty::Bool,
            ),
        ]);
        let annotated = AppState::empty()
            .apply_actions(&[Action::SetDefAnn(wanted)])
            .expect("a fresh definition takes any annotation");
        let laid_out = type_chars("{", annotated);
        assert_eq!(
            laid_out.edit.field_ids(),
            vec![
                nothing_core::exp::Id::from_u128(0x11),
                nothing_core::exp::Id::from_u128(0x22)
            ],
            "where a record type is expected, a brace lays out that record's fields rather than \
             minting an identity nothing knows"
        );
        assert!(laid_out.edit.is_well_typed());
        assert_eq!(laid_out.slot, Slot::FieldName);
    }

    #[test]
    fn a_dot_projects_and_the_field_slot_picks_by_prefix() {
        let record = record_of_two_fields();

        let first = type_chars(".", record.clone());
        assert_eq!(first.text(), "{x = 1, y = 2}.x");
        assert_eq!(first.slot, Slot::FieldPick);

        let picked = type_chars(".y", record.clone());
        assert_eq!(
            picked.text(),
            "{x = 1, y = 2}.y",
            "the slot picks a field of the record being projected, by prefix"
        );

        let unknown = type_chars(".z", record.clone());
        assert_eq!(
            unknown.text(),
            "{x = 1, y = 2}.x",
            "a prefix that names nothing leaves the projection alone"
        );
        assert_eq!(
            unknown.hint.as_deref(),
            Some("no field in view starts with `z`")
        );

        let nowhere = type_chars(".", AppState::empty());
        assert_eq!(
            nowhere.text(),
            "⦇⦈",
            "a dot with no field to name writes nothing"
        );
        assert_eq!(
            nowhere.hint.as_deref(),
            Some("`.` names a field, and this document has no record to name one in")
        );

        let nested = type_chars("{x={y=1", AppState::empty());
        let nested = handle_key(key(KeyCode::Up), nested);
        let nested = handle_key(key(KeyCode::Up), nested);
        assert_eq!(
            type_chars("..", nested).text(),
            "{x = {y = 1}}.x.y",
            "a dot never climbs: a second one wraps the projection in place"
        );

        let applied = type_chars("\\f:?.f ", AppState::empty());
        let applied = type_chars("{x=1", applied);
        let applied = handle_key(key(KeyCode::Up), applied);
        assert_eq!(
            type_chars(".", applied).text(),
            "λf:?. f {x = 1}.x",
            "and a projection binds tighter than the application it stands in"
        );
    }

    #[test]
    fn control_n_and_d_address_a_field_inside_a_record_and_a_definition_outside_one() {
        let record = record_of_two_fields();
        assert_eq!(
            record.edit.def_count(),
            1,
            "two fields were added without adding a definition"
        );

        let grown = handle_key(ctrl(KeyCode::Char('n')), record.clone());
        assert_eq!(grown.text(), "{x = 1, y = 2, f0 = ⦇⦈}");
        assert_eq!(grown.slot, Slot::FieldName);

        let first = handle_key(key(KeyCode::Down), record.clone());
        let dropped = handle_key(ctrl(KeyCode::Char('d')), first.clone());
        assert_eq!(
            dropped.text(),
            "{y = 2}",
            "inside a record, C-d drops the field the cursor is in"
        );
        assert_eq!(dropped.edit.def_count(), 1);

        let outside = handle_key(ctrl(KeyCode::Char('n')), AppState::empty());
        assert_eq!(
            outside.edit.def_count(),
            2,
            "outside a record the same key still adds a definition"
        );
        let undone = handle_key(ctrl(KeyCode::Char('d')), outside);
        assert_eq!(undone.edit.def_count(), 1);
        let last = handle_key(ctrl(KeyCode::Char('d')), AppState::empty());
        assert_eq!(last.edit.def_count(), 1);
        assert_eq!(
            last.hint.as_deref(),
            Some("a document keeps at least one definition")
        );

        let moved = handle_key(ctrl(KeyCode::Left), record.clone());
        assert_eq!(
            moved.text(),
            "{x = 1, y = 2}",
            "C-left needs a field to move, not the record itself"
        );
        let moved = handle_key(ctrl(KeyCode::Right), first.clone());
        assert_eq!(
            moved.text(),
            "{y = 2, x = 1}",
            "and C-right reorders the field where it stands"
        );
        assert_eq!(
            handle_key(ctrl(KeyCode::Left), first).hint.as_deref(),
            Some("this field cannot move any earlier")
        );
    }

    #[test]
    fn renaming_a_field_renames_every_use_of_it_at_once() {
        let built = type_chars("{x=1", AppState::empty());
        let built = handle_key(key(KeyCode::Up), built);
        let projected = type_chars(".", built);
        assert_eq!(projected.text(), "{x = 1}.x");

        let name = handle_key(key(KeyCode::Up), projected);
        let name = handle_key(key(KeyCode::Down), name);
        let name = handle_key(key(KeyCode::Down), name);
        assert_eq!(
            name.slot,
            Slot::FieldName,
            "the value of a field is one step below the record, and its name is a slot"
        );

        let renamed = type_chars("count", name);
        assert_eq!(
            renamed.text(),
            "{count = 1}.count",
            "one name run renames the construction site and the projection together, because \
             they are the same identity"
        );
    }

    const RED: nothing_core::exp::Id = nothing_core::exp::Id::from_u128(0x11);
    const BLUE: nothing_core::exp::Id = nothing_core::exp::Id::from_u128(0x22);

    fn red_or_blue() -> nothing_core::ty::Ty {
        nothing_core::ty::variant(vec![
            (RED, nothing_core::ty::Ty::Num),
            (BLUE, nothing_core::ty::Ty::Bool),
        ])
    }

    fn matching_on_red_or_blue(state: AppState) -> AppState {
        let lam = state
            .apply_actions(&[
                Action::ConstructLam,
                Action::MoveParent,
                Action::SetAnn(red_or_blue()),
                Action::MoveChild(0),
                Action::Rename(RED, "Red".into()),
                Action::Rename(BLUE, "Blue".into()),
            ])
            .expect("a variant type is not spellable, so the annotation is set as an action");
        type_chars("x|", lam)
    }

    #[test]
    fn a_backtick_injects_and_lands_in_the_constructor_slot() {
        let fresh = type_chars("`", AppState::empty());
        assert_eq!(fresh.text(), "`C0 ⦇⦈");
        assert_eq!(
            fresh.slot,
            Slot::ConstructorPick,
            "an injection leaves the cursor on the case's name, as a brace does on a field's"
        );
        assert_eq!(fresh.entry, "");

        assert_eq!(
            typed("1`"),
            "`C0 1",
            "it wraps what was already there, like every other form key"
        );

        let expected = AppState::empty()
            .apply_actions(&[Action::SetDefAnn(red_or_blue())])
            .expect("a fresh definition takes any annotation");
        let adopted = type_chars("`", expected);
        assert_eq!(
            adopted.edit.constructor_ids(),
            vec![RED],
            "where a variant is expected the backtick adopts that variant's first constructor \
             rather than minting an identity the context could only quarantine"
        );
        assert!(adopted.edit.is_well_typed());
    }

    #[test]
    fn a_bar_writes_a_match_with_one_arm_per_constructor() {
        assert_eq!(
            typed("|"),
            "match ⦇⦈ {}",
            "an unknown scrutinee answers for nothing, so it needs no arms"
        );
        assert_eq!(
            typed("1+2|"),
            "match ⦇1 + 2⦈ {}",
            "`|` wraps the whole sum, and a number is not a variant, so it is quarantined \
             rather than refused"
        );

        let state = matching_on_red_or_blue(AppState::empty());
        assert_eq!(
            state.text(),
            "λx0:[Red: Num | Blue: Bool]. match x0 { Red x1 -> ⦇⦈ | Blue x2 -> ⦇⦈ }",
            "one arm per constructor, written by the action rather than by the user"
        );
        assert_eq!(
            index_path(&state.edit.zipper),
            vec![0, 1],
            "and the cursor lands in the first arm's body"
        );
        assert_eq!(
            type_chars("1", state).text(),
            "λx0:[Red: Num | Blue: Bool]. match x0 { Red x1 -> 1 | Blue x2 -> ⦇⦈ }"
        );
    }

    #[test]
    fn control_n_adds_an_arm_to_every_match_on_the_same_variant() {
        let first = matching_on_red_or_blue(AppState::empty());
        let second = first
            .apply_actions(&[Action::CreateDefinition])
            .expect("a second definition");
        let both = matching_on_red_or_blue(second);
        assert_eq!(
            both.edit.render_document(),
            "main : ? = λx0:[Red: Num | Blue: Bool]. match x0 { Red x1 -> ⦇⦈ | Blue x2 -> ⦇⦈ }\n\
             def : ? = λx3:[Red: Num | Blue: Bool]. match x3 { Red x4 -> ⦇⦈ | Blue x5 -> ⦇⦈ }"
        );

        let grown = handle_key(ctrl(KeyCode::Char('n')), both.clone());
        assert_eq!(
            grown.edit.render_document(),
            "main : ? = λx0:[Red: Num | Blue: Bool]. \
             match x0 { Red x1 -> ⦇⦈ | Blue x2 -> ⦇⦈ | C0 x7 -> ⦇⦈ }\n\
             def : ? = λx3:[Red: Num | Blue: Bool]. \
             match x3 { Red x4 -> ⦇⦈ | Blue x5 -> ⦇⦈ | C0 x6 -> ⦇⦈ }",
            "one key adds the case to every match that answers the same question — each with \
             its own payload binder, because a binder is an identity and not a name"
        );
        assert_eq!(grown.slot, Slot::ConstructorName, "and names it");
        assert_eq!(
            grown.actions().len(),
            both.actions().len() + 1,
            "in one action, so one C-z takes the whole sweep back"
        );

        let refused = handle_key(ctrl(KeyCode::Char('d')), both.clone());
        assert_eq!(
            refused.edit.render_document(),
            both.edit.render_document(),
            "and the arm cannot be dropped while a scrutinee still injects it"
        );
        assert_eq!(
            refused.hint.as_deref(),
            Some("something still injects this case, so the arm has to stay")
        );
    }

    #[test]
    fn renaming_a_constructor_renames_every_use_of_it_at_once() {
        let first = matching_on_red_or_blue(AppState::empty());
        let second = first
            .apply_actions(&[Action::CreateDefinition])
            .expect("a second definition");
        let both = matching_on_red_or_blue(second);

        let slot = handle_key(key(KeyCode::Left), both);
        assert_eq!(
            slot.slot,
            Slot::ConstructorName,
            "a step left out of an arm's body reaches the case's name"
        );
        let renamed = type_chars("Crimson", slot);
        assert_eq!(
            renamed.edit.render_document(),
            "main : ? = λx0:[Crimson: Num | Blue: Bool]. \
             match x0 { Crimson x1 -> ⦇⦈ | Blue x2 -> ⦇⦈ }\n\
             def : ? = λx3:[Crimson: Num | Blue: Bool]. \
             match x3 { Crimson x4 -> ⦇⦈ | Blue x5 -> ⦇⦈ }",
            "one name run renames the case in both matches and in the type itself, because \
             they are the same identity"
        );
    }
}
