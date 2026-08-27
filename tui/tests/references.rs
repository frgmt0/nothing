//! Phase 4, "Wire construction keys" — the acceptance criterion, headlessly.
//!
//! > **Done when** you can build all five reference programs in the TUI
//! > without touching the REPL harness.
//!
//! Taken literally. For each of the five reference programs there is a
//! `tests/keys/<name>.keys` fixture: the keystrokes, one per line, that a
//! person types into the editor to build it. This test drives them through
//! the *pure* key handler — the same function the terminal loop calls, with
//! no REPL, no script parser and no hand-built `Exp` anywhere on the path —
//! and asserts the result is the same program `bench/fixtures/<name>.actions`
//! produces through the Phase 3 action calculus.
//!
//! "The same program" means structurally equal up to hole *identity*: the
//! two routes mint hole ids in different orders, and a hole id is an
//! identity, not a value. Everything else — shape, binder ids, literals,
//! annotations, and where the holes are — must match exactly, and the
//! rendered projection is compared too, so a difference is legible when this
//! fails.
//!
//! The `.keys` files are also the input for the next benchmark run: their
//! non-blank, non-comment line count is the keystroke count, and
//! [`AppState::actions`] beside it is the primitive-action count that
//! `KEYS.md` §Coverage asks the harness to record as well.

use nothing_action::script::replay_script;
use nothing_core::exp::{Exp, HoleId};
use nothing_core::render::render;
use nothing_core::typing::is_well_typed;
use nothing_tui::AppState;
use nothing_tui::keyscript::{parse_keys, replay_keys};

/// One reference program: what the keyboard types, what the action calculus
/// records, what the projection should read, and the permanent Neovim
/// baseline from `bench/references.md`.
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
    ]
}

/// Renumber every hole id in source order.
///
/// A hole id is an identity — which hole this is, across edits — not part of
/// the program's value, and the keyboard and the action script arrive at the
/// same program having minted holes in different orders. Normalising is the
/// honest comparison; it still catches a hole in the wrong *place*, a
/// missing hole, or an extra one.
fn normalize(exp: &Exp) -> Exp {
    fn go(exp: &Exp, next: &mut u64) -> Exp {
        let mut fresh = || {
            let id = HoleId::new(*next);
            *next += 1;
            id
        };
        match exp {
            Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) => exp.clone(),
            Exp::Lam(id, ty, body) => Exp::lam(*id, ty.clone(), go(body, next)),
            Exp::Ap(f, a) => {
                let f = go(f, next);
                Exp::ap(f, go(a, next))
            }
            Exp::BinOp(op, l, r) => {
                let l = go(l, next);
                Exp::bin_op(*op, l, go(r, next))
            }
            Exp::If(c, t, e) => {
                let c = go(c, next);
                let t = go(t, next);
                Exp::if_(c, t, go(e, next))
            }
            Exp::Let(id, bound, body) => {
                let bound = go(bound, next);
                Exp::let_(*id, bound, go(body, next))
            }
            Exp::Pair(a, b) => {
                let a = go(a, next);
                Exp::pair(a, go(b, next))
            }
            Exp::Proj(side, e) => Exp::proj(*side, go(e, next)),
            Exp::EmptyHole(_) => Exp::empty_hole(fresh()),
            Exp::NonEmptyHole(_, inner) => {
                let id = fresh();
                Exp::non_empty_hole(id, go(inner, next))
            }
        }
    }
    let mut next = 0;
    go(exp, &mut next)
}

#[test]
fn every_reference_program_is_buildable_from_the_keyboard() {
    for reference in references() {
        let typed = replay_keys(reference.keys, AppState::empty())
            .unwrap_or_else(|e| panic!("{}: {e}", reference.name));
        let scripted =
            replay_script(reference.actions).unwrap_or_else(|e| panic!("{}: {e}", reference.name));

        assert_eq!(
            render(&typed.program()),
            render(&scripted.exp()),
            "{}: the keyboard built a different program than the action fixture",
            reference.name
        );
        assert_eq!(
            normalize(&typed.program()),
            normalize(&scripted.exp()),
            "{}: same projection, different tree",
            reference.name
        );
        assert_eq!(
            render(&typed.program()),
            reference.expected.trim(),
            "{}: and neither matches the committed expected rendering",
            reference.name
        );
        assert!(
            is_well_typed(&typed.program()),
            "{}: the editor left a program that does not typecheck",
            reference.name
        );
    }
}

/// The `.keys` files are the benchmark's input, so the count has to be the
/// thing anyone can read off the file: one keystroke, one line.
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

/// Phase 0's failure-mode guard, checked here rather than waited for: if a
/// reference program costs more than 3× its Neovim baseline, the grammar is
/// wrong and the next sprint is spent fixing it.
///
/// The dated ratios belong in `bench/RESULTS.md`, written by the benchmark
/// re-run; this is only the tripwire.
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

/// Undo is per keystroke, so undoing every keystroke of a reference program
/// gets back to the empty hole it started from — a stronger statement than
/// any single undo test, and it exercises replay from the base snapshot at
/// every depth.
#[test]
fn undoing_every_keystroke_returns_to_the_empty_program() {
    use crossterm::event::KeyCode;
    use nothing_tui::keys::{ctrl, handle_key};

    for reference in references() {
        let keys = parse_keys(reference.keys).expect("the fixture parses");
        let mut state = replay_keys(reference.keys, AppState::empty()).expect("replays");
        let built = state.program();

        for _ in 0..keys.len() {
            state = handle_key(ctrl(KeyCode::Char('z')), state);
        }
        assert!(
            matches!(state.program(), Exp::EmptyHole(_)),
            "{}: undoing everything left {}",
            reference.name,
            render(&state.program())
        );
        assert_eq!(state.keystrokes(), 0);

        for _ in 0..keys.len() {
            state = handle_key(ctrl(KeyCode::Char('r')), state);
        }
        assert_eq!(
            render(&state.program()),
            render(&built),
            "{}: redoing everything did not rebuild it",
            reference.name
        );
    }
}
