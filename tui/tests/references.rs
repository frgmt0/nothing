use nothing_action::script::replay_script;
use nothing_core::doc::Doc;
use nothing_core::exp::{Exp, HoleId, Id};
use nothing_core::ty::Ty;
use nothing_tui::AppState;
use nothing_tui::keyscript::{parse_keys, replay_keys};

struct Reference {
    name: &'static str,
    keys: &'static str,
    actions: &'static str,
    expected: &'static str,
    neovim: usize,
}

fn references() -> Vec<Reference> {
    vec![
        Reference {
            name: "factorial",
            keys: include_str!("keys/factorial.keys"),
            actions: include_str!("../../bench/fixtures/factorial.actions"),
            expected: include_str!("../../bench/fixtures/factorial.expected"),
            neovim: 84,
        },
        Reference {
            name: "list_map",
            keys: include_str!("keys/list_map.keys"),
            actions: include_str!("../../bench/fixtures/list_map.actions"),
            expected: include_str!("../../bench/fixtures/list_map.expected"),
            neovim: 114,
        },
        Reference {
            name: "record",
            keys: include_str!("keys/record.keys"),
            actions: include_str!("../../bench/fixtures/record.actions"),
            expected: include_str!("../../bench/fixtures/record.expected"),
            neovim: 65,
        },
        Reference {
            name: "state_machine",
            keys: include_str!("keys/state_machine.keys"),
            actions: include_str!("../../bench/fixtures/state_machine.actions"),
            expected: include_str!("../../bench/fixtures/state_machine.expected"),
            neovim: 151,
        },
        Reference {
            name: "nested_conditional",
            keys: include_str!("keys/nested_conditional.keys"),
            actions: include_str!("../../bench/fixtures/nested_conditional.actions"),
            expected: include_str!("../../bench/fixtures/nested_conditional.expected"),
            neovim: 146,
        },
        Reference {
            name: "greeting",
            keys: include_str!("keys/greeting.keys"),
            actions: include_str!("../../bench/fixtures/greeting.actions"),
            expected: include_str!("../../bench/fixtures/greeting.expected"),
            neovim: 127,
        },
    ]
}

fn canonical(id: Id, seen: &mut Vec<Id>) -> Id {
    let position = match seen.iter().position(|known| *known == id) {
        Some(i) => i,
        None => {
            seen.push(id);
            seen.len() - 1
        }
    };
    Id::from_u128(position as u128)
}

fn go(exp: &Exp, next: &mut u128, seen: &mut Vec<Id>) -> Exp {
    let mut fresh = || {
        let id = HoleId::from_u128(*next);
        *next += 1;
        id
    };
    match exp {
        Exp::Num(_) | Exp::Bool(_) | Exp::Str(_) => exp.clone(),
        Exp::Var(id) => Exp::var(canonical(*id, seen)),
        Exp::Lam(id, ty, body) => {
            let id = canonical(*id, seen);
            Exp::lam(id, ty.clone(), go(body, next, seen))
        }
        Exp::Ap(f, a) => {
            let f = go(f, next, seen);
            Exp::ap(f, go(a, next, seen))
        }
        Exp::BinOp(op, l, r) => {
            let l = go(l, next, seen);
            Exp::bin_op(*op, l, go(r, next, seen))
        }
        Exp::If(c, t, e) => {
            let c = go(c, next, seen);
            let t = go(t, next, seen);
            Exp::if_(c, t, go(e, next, seen))
        }
        Exp::Let(id, bound, body) => {
            let bound = go(bound, next, seen);
            let id = canonical(*id, seen);
            Exp::let_(id, bound, go(body, next, seen))
        }
        Exp::Pair(a, b) => {
            let a = go(a, next, seen);
            Exp::pair(a, go(b, next, seen))
        }
        Exp::Proj(side, e) => Exp::proj(*side, go(e, next, seen)),
        Exp::Nil => Exp::Nil,
        Exp::Cons(head, tail) => {
            let head = go(head, next, seen);
            Exp::cons(head, go(tail, next, seen))
        }
        Exp::Fold(list, init, step) => {
            let list = go(list, next, seen);
            let init = go(init, next, seen);
            Exp::fold(list, init, go(step, next, seen))
        }
        Exp::Record(fields) => Exp::record(
            fields
                .iter()
                .map(|(id, value)| (canonical(*id, seen), go(value, next, seen)))
                .collect::<Vec<_>>(),
        ),
        Exp::Field(subject, id) => {
            let subject = go(subject, next, seen);
            Exp::field(subject, canonical(*id, seen))
        }
        Exp::EmptyHole(_) => Exp::empty_hole(fresh()),
        Exp::NonEmptyHole(_, inner) => {
            let id = fresh();
            Exp::non_empty_hole(id, go(inner, next, seen))
        }
    }
}

fn normalize_doc(doc: &Doc) -> Vec<(Ty, Exp)> {
    let mut next = 0;
    let mut seen = doc.ids();
    doc.defs()
        .iter()
        .map(|def| (def.ann.clone(), go(&def.body, &mut next, &mut seen)))
        .collect()
}

#[test]
fn every_reference_program_is_buildable_from_the_keyboard() {
    for reference in references() {
        let typed = replay_keys(reference.keys, AppState::empty())
            .unwrap_or_else(|e| panic!("{}: {e}", reference.name));
        let scripted =
            replay_script(reference.actions).unwrap_or_else(|e| panic!("{}: {e}", reference.name));

        assert_eq!(
            typed.edit.render_document(),
            scripted.render_document(),
            "{}: the keyboard built a different document than the action fixture",
            reference.name
        );
        assert_eq!(
            normalize_doc(&typed.edit.doc()),
            normalize_doc(&scripted.doc()),
            "{}: same projection, different tree",
            reference.name
        );
        assert_eq!(
            typed.edit.render_document(),
            reference.expected.trim(),
            "{}: and neither matches the committed expected rendering",
            reference.name
        );
        assert!(
            typed.edit.is_well_typed(),
            "{}: the editor left a document that does not typecheck",
            reference.name
        );
    }
}

#[test]
fn the_fixture_line_count_is_the_keystroke_count() {
    for reference in references() {
        let lines = reference
            .keys
            .lines()
            .filter(|line| {
                let line = line.trim();
                !line.is_empty() && !line.starts_with('#')
            })
            .count();
        let keys = parse_keys(reference.keys).expect("the fixture parses");
        assert_eq!(
            keys.len(),
            lines,
            "{}: countable-line count and parsed-keystroke count disagree",
            reference.name
        );

        let typed = replay_keys(reference.keys, AppState::empty()).expect("the fixture replays");
        assert_eq!(
            typed.keystrokes(),
            keys.len(),
            "{}: {} keystrokes were pressed but only {} were recorded as undoable history",
            reference.name,
            keys.len(),
            typed.keystrokes(),
        );
    }
}

#[test]
fn no_reference_program_exceeds_the_three_times_guard() {
    for reference in references() {
        let keys = parse_keys(reference.keys)
            .expect("the fixture parses")
            .len();
        let typed = replay_keys(reference.keys, AppState::empty()).expect("the fixture replays");
        let ratio = keys as f64 / reference.neovim as f64;
        println!(
            "{:<20} {:>3} keystrokes  {:>3} actions  vs {:>3} in Neovim = {ratio:.2}×",
            reference.name,
            keys,
            typed.actions().len(),
            reference.neovim,
        );
        assert!(
            ratio <= 3.0,
            "{}: {keys} keystrokes against a baseline of {} is {ratio:.2}×",
            reference.name,
            reference.neovim
        );
    }
}

#[test]
fn undoing_every_keystroke_returns_to_the_empty_program() {
    use crossterm::event::KeyCode;
    use nothing_tui::keys::{ctrl, handle_key};

    for reference in references() {
        let keys = parse_keys(reference.keys).expect("the fixture parses");
        let mut state = replay_keys(reference.keys, AppState::empty()).expect("replays");
        let built = state.edit.render_document();

        for _ in 0..keys.len() {
            state = handle_key(ctrl(KeyCode::Char('z')), state);
        }
        assert!(
            matches!(state.program(), Exp::EmptyHole(_)),
            "{}: undoing everything left {}",
            reference.name,
            state.text()
        );
        assert_eq!(
            state.definition_count(),
            1,
            "{}: undoing everything left {} definitions",
            reference.name,
            state.definition_count()
        );
        assert_eq!(state.keystrokes(), 0);

        for _ in 0..keys.len() {
            state = handle_key(ctrl(KeyCode::Char('r')), state);
        }
        assert_eq!(
            state.edit.render_document(),
            built,
            "{}: redoing everything did not rebuild it",
            reference.name
        );
    }
}
