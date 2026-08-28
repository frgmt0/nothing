use std::collections::BTreeSet;

use crossterm::event::KeyCode;
use nothing_action::zipper::all_positions;
use nothing_core::examples;
use nothing_core::exp::Exp;
use nothing_tui::app::index_path;
use nothing_tui::keys::{handle_key, key};
use nothing_tui::render::render_to_string;
use nothing_tui::{AppState, Slot};

const WIDTH: u16 = 100;
const HEIGHT: u16 = 16;

type Position = (Vec<usize>, Slot);

fn position(state: &AppState) -> Position {
    (index_path(state.zipper()), state.slot)
}

fn all_example_programs() -> Vec<(&'static str, Exp)> {
    vec![
        ("let_identity", examples::let_identity()),
        ("increment_applied", examples::increment_applied()),
        ("clamp_to_one", examples::clamp_to_one()),
        ("pair_and_project", examples::pair_and_project()),
        ("pair_with_empty_hole", examples::pair_with_empty_hole()),
        ("add_with_empty_hole", examples::add_with_empty_hole()),
        ("square_and_compare", examples::square_and_compare()),
        (
            "identity_hole_annotated_applied",
            examples::identity_hole_annotated_applied(),
        ),
        (
            "add_with_non_empty_hole",
            examples::add_with_non_empty_hole(),
        ),
        (
            "if_over_pairs_with_hole",
            examples::if_over_pairs_with_hole(),
        ),
    ]
}

fn expected_positions(exp: &Exp) -> BTreeSet<Position> {
    let mut out = BTreeSet::new();
    for z in all_positions(exp) {
        let path = index_path(&z);
        out.insert((path.clone(), Slot::Node));
        match z.focus {
            Exp::Lam(..) => {
                out.insert((path.clone(), Slot::BinderName));
                out.insert((path, Slot::Annotation));
            }
            Exp::Let(..) => {
                out.insert((path, Slot::BinderName));
            }
            _ => {}
        }
    }
    out
}

struct Walk {
    program: Exp,
    name: &'static str,
    screens: Vec<String>,
    visited: Vec<Position>,
    keys: usize,
}

impl Walk {
    fn start(name: &'static str, state: &AppState) -> Walk {
        Walk {
            program: state.program(),
            name,
            screens: vec![render_to_string(state, WIDTH, HEIGHT)],
            visited: vec![position(state)],
            keys: 0,
        }
    }

    fn record(&mut self, code: KeyCode, state: &AppState) {
        let screen = render_to_string(state, WIDTH, HEIGHT);
        assert_ne!(
            self.screens.last().expect("started"),
            &screen,
            "{}: pressing {code:?} to reach {:?} did not change the screen",
            self.name,
            position(state)
        );
        assert_eq!(
            state.program(),
            self.program,
            "{}: pressing {code:?} changed the program",
            self.name
        );
        self.screens.push(screen);
        self.visited.push(position(state));
        self.keys += 1;
    }
}

fn press(code: KeyCode, state: &AppState) -> Option<AppState> {
    let next = handle_key(key(code), state.clone());
    (position(&next) != position(state)).then_some(next)
}

fn walk_subtree(state: &AppState, walk: &mut Walk) -> AppState {
    let mut cur = match press(KeyCode::Down, state) {
        None => return state.clone(),
        Some(child) => {
            walk.record(KeyCode::Down, &child);
            child
        }
    };

    loop {
        cur = walk_subtree(&cur, walk);
        match press(KeyCode::Right, &cur) {
            Some(next) => {
                walk.record(KeyCode::Right, &next);
                cur = next;
            }
            None => break,
        }
    }

    let back = press(KeyCode::Up, &cur).expect("↑ must lead back out of a child");
    walk.record(KeyCode::Up, &back);
    assert_eq!(
        position(&back),
        position(state),
        "{}: ↑ from the last child did not return to the parent",
        walk.name
    );
    back
}

#[test]
fn every_position_of_every_example_is_reachable_by_keyboard() {
    for (name, exp) in all_example_programs() {
        let root = AppState::new(exp.clone());
        let mut walk = Walk::start(name, &root);
        let end = walk_subtree(&root, &mut walk);

        assert_eq!(
            position(&end),
            position(&root),
            "{name}: the walk did not return to the root"
        );

        let visited: BTreeSet<Position> = walk.visited.iter().cloned().collect();
        let expected = expected_positions(&exp);
        assert_eq!(
            visited, expected,
            "{name}: the keyboard reached a different set of positions than the program has"
        );
        assert!(
            walk.keys >= expected.len() - 1,
            "{name}: {} keys cannot have visited {} positions",
            walk.keys,
            expected.len()
        );
    }
}

#[test]
fn left_reverses_right_everywhere() {
    for (name, exp) in all_example_programs() {
        let root = AppState::new(exp.clone());
        for start in walk_states(&root) {
            if let Some(next) = press(KeyCode::Right, &start) {
                let back = press(KeyCode::Left, &next)
                    .unwrap_or_else(|| panic!("{name}: ← declined after →"));
                assert_eq!(
                    position(&back),
                    position(&start),
                    "{name}: ← did not undo →"
                );
            }
        }
    }
}

fn walk_states(root: &AppState) -> Vec<AppState> {
    fn go(state: &AppState, out: &mut Vec<AppState>) -> AppState {
        let mut cur = match press(KeyCode::Down, state) {
            None => return state.clone(),
            Some(child) => child,
        };
        loop {
            out.push(cur.clone());
            cur = go(&cur, out);
            match press(KeyCode::Right, &cur) {
                Some(next) => cur = next,
                None => break,
            }
        }
        press(KeyCode::Up, &cur).expect("↑ must lead back out of a child")
    }

    let mut out = vec![root.clone()];
    go(root, &mut out);
    out
}

#[test]
fn tab_reaches_every_hole() {
    for (name, exp) in all_example_programs() {
        let holes: BTreeSet<Vec<usize>> = all_positions(&exp)
            .iter()
            .filter(|z| matches!(z.focus, Exp::EmptyHole(_) | Exp::NonEmptyHole(..)))
            .map(index_path)
            .collect();
        let mut state = AppState::new(exp.clone());
        let mut seen = BTreeSet::new();
        for _ in 0..holes.len() {
            state = handle_key(key(KeyCode::Tab), state);
            seen.insert(index_path(state.zipper()));
        }
        assert_eq!(seen, holes, "{name}: Tab missed a hole");
    }
}

#[test]
fn the_shell_opens_on_factorial_and_quits() {
    let state = AppState::factorial();
    let screen = render_to_string(&state, 80, 10);
    assert!(
        screen.contains("»λx0:Num. if x0 == 0 then 1 else x0 * ⦇⦈«"),
        "{screen}"
    );

    let quit = handle_key(
        crossterm::event::KeyEvent::new(
            KeyCode::Char('q'),
            crossterm::event::KeyModifiers::CONTROL,
        ),
        state,
    );
    assert!(quit.quit, "C-q must set the quit flag the loop reads");
}
