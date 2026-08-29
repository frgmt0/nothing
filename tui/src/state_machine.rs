use nothing_action::act::Action;
use nothing_action::cursor_render::{CURSOR_CLOSE, CURSOR_OPEN};
use nothing_action::zipper::unzip;
use nothing_core::exp::{Exp, Id, Op};
use nothing_core::render::render;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{AppState, Slot, index_path};

#[derive(Clone, Debug)]
pub enum Cond {
    Eq(Vec<usize>, i64),
    Case(Id),
    Else,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub cond: Cond,
    pub result: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct Shape {
    pub var: Id,
    pub rows: Vec<Row>,
}

pub fn recognize(program: &Exp) -> Option<Shape> {
    let Exp::Lam(id, _, body) = program else {
        return None;
    };
    let var = *id;
    let shape = match body.as_ref() {
        Exp::Match(..) => recognize_match(program, var)?,
        _ => recognize_chain(program, var)?,
    };
    if shape.rows.len() < 3 {
        return None;
    }
    Some(shape)
}

fn recognize_match(program: &Exp, var: Id) -> Option<Shape> {
    let z = unzip(program.clone()).move_child(0)?;
    let Exp::Match(scrutinee, arms) = z.focus.clone() else {
        return None;
    };
    match scrutinee.as_ref() {
        Exp::Var(v) if *v == var => {}
        _ => return None,
    }
    let mut rows = Vec::with_capacity(arms.len());
    for (index, (ctor, _, _)) in arms.iter().enumerate() {
        rows.push(Row {
            cond: Cond::Case(*ctor),
            result: index_path(&z.clone().move_child(index + 1)?),
        });
    }
    Some(Shape { var, rows })
}

fn recognize_chain(program: &Exp, var: Id) -> Option<Shape> {
    let mut rows = Vec::new();
    let mut z = unzip(program.clone()).move_child(0)?;
    while let Exp::If(cond, _, _) = z.focus.clone() {
        let Exp::BinOp(Op::Eq, l, r) = *cond else {
            break;
        };
        let Exp::Var(v) = *l else {
            break;
        };
        if v != var {
            break;
        }
        let Exp::Num(n) = *r else {
            break;
        };

        let cond_path = index_path(&z.clone().move_child(0)?.move_child(1)?);
        let result_path = index_path(&z.clone().move_child(1)?);
        rows.push(Row {
            cond: Cond::Eq(cond_path, n),
            result: result_path,
        });
        z = z.move_child(2)?;
    }
    rows.push(Row {
        cond: Cond::Else,
        result: index_path(&z),
    });
    Some(Shape { var, rows })
}

fn exp_at(program: &Exp, path: &[usize]) -> Exp {
    let mut z = unzip(program.clone());
    for &i in path {
        z = z
            .move_child(i)
            .expect("a path produced by recognize is always walkable");
    }
    z.focus
}

fn pad(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len >= width {
        text.to_string()
    } else {
        format!("{text}{}", " ".repeat(width - len))
    }
}

fn mark(text: &str, marked: bool) -> String {
    if marked {
        format!("{CURSOR_OPEN}{text}{CURSOR_CLOSE}")
    } else {
        text.to_string()
    }
}

pub fn marked_text(state: &AppState) -> String {
    if state.slot != Slot::Node {
        return crate::render::program_line(state);
    }
    let program = state.program();
    let Some(shape) = recognize(&program) else {
        return crate::render::program_line(state);
    };
    let names = state.names();
    let focus_path = index_path(state.zipper());
    let var_name = names.display(shape.var);

    let cond_texts: Vec<String> = shape
        .rows
        .iter()
        .map(|row| match &row.cond {
            Cond::Eq(_, n) => format!("{var_name} == {n}"),
            Cond::Case(ctor) => names.display(*ctor),
            Cond::Else => "else".to_string(),
        })
        .collect();
    let result_texts: Vec<String> = shape
        .rows
        .iter()
        .map(|row| render(&exp_at(&program, &row.result), names))
        .collect();
    let cond_width = cond_texts
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(0);

    let mut out = format!("state machine on {var_name}\n");
    for (i, row) in shape.rows.iter().enumerate() {
        let cond_marked = matches!(&row.cond, Cond::Eq(path, _) if *path == focus_path);
        let result_marked = row.result == focus_path;
        let cond_field = mark(&pad(&cond_texts[i], cond_width), cond_marked);
        let result_field = mark(&result_texts[i], result_marked);
        out.push_str(&format!("  {cond_field}  ->  {result_field}\n"));
    }
    out.trim_end_matches('\n').to_string()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Col {
    Cond,
    Result,
}

fn locate(shape: &Shape, focus_path: &[usize]) -> Option<(usize, Col)> {
    for (i, row) in shape.rows.iter().enumerate() {
        if let Cond::Eq(cond_path, _) = &row.cond
            && focus_path.starts_with(cond_path.as_slice())
        {
            return Some((i, Col::Cond));
        }
        if focus_path.starts_with(row.result.as_slice()) {
            return Some((i, Col::Result));
        }
    }
    None
}

fn target_path(shape: &Shape, row: usize, col: Col) -> Vec<usize> {
    let row = &shape.rows[row];
    match (col, &row.cond) {
        (Col::Cond, Cond::Eq(path, _)) => path.clone(),
        _ => row.result.clone(),
    }
}

fn goto(state: &AppState, target: &[usize]) -> Option<AppState> {
    let mut actions = vec![Action::MoveParent; state.zipper().depth()];
    actions.extend(target.iter().map(|&i| Action::MoveChild(i)));
    state.apply_actions(&actions)
}

pub fn handle_key(key: KeyEvent, state: AppState) -> Option<AppState> {
    if key.modifiers.contains(KeyModifiers::CONTROL) || state.slot != Slot::Node {
        return None;
    }
    let shape = recognize(&state.program())?;
    let focus_path = index_path(state.zipper());
    let (row, col) = locate(&shape, &focus_path).unwrap_or((0, Col::Cond));

    let (target_row, mut target_col) = match key.code {
        KeyCode::Down => ((row + 1).min(shape.rows.len() - 1), col),
        KeyCode::Up => (row.saturating_sub(1), col),
        KeyCode::Left => (row, Col::Cond),
        KeyCode::Right => (row, Col::Result),
        _ => return None,
    };
    if !matches!(shape.rows[target_row].cond, Cond::Eq(..)) && target_col == Col::Cond {
        target_col = Col::Result;
    }

    let path = target_path(&shape, target_row, target_col);
    goto(&state, &path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_action::script::replay_script;
    use nothing_core::examples;

    const STATE_MACHINE_FIXTURE: &str = include_str!("../../bench/fixtures/state_machine.actions");

    fn state_machine_state() -> AppState {
        let replayed = replay_script(STATE_MACHINE_FIXTURE)
            .expect("the embedded state machine fixture must replay cleanly");
        AppState::with_names(replayed.exp(), replayed.names.clone())
    }

    #[test]
    fn recognizes_the_reference_state_machine() {
        let state = state_machine_state();
        let shape = recognize(&state.program()).expect("the fixture is state-machine shaped");
        assert_eq!(shape.rows.len(), 3);
        let cases: Vec<String> = shape
            .rows
            .iter()
            .map(|row| match &row.cond {
                Cond::Case(ctor) => state.display_name(*ctor),
                other => panic!("the reference is a match now, not {other:?}"),
            })
            .collect();
        assert_eq!(cases, vec!["Idle", "Running", "Stopped"]);
    }

    #[test]
    fn the_chain_of_equality_tests_is_still_a_state_machine() {
        let program = replay_script(
            "construct-lam\nmove-parent\nrename s\nset-ann Num\nmove-child 0\n\
             construct-if\nconstruct-binop eq\nconstruct-var s\nmove-next-sibling\n\
             construct-num 0\nmove-parent\nmove-next-sibling\nconstruct-num 1\n\
             move-next-sibling\nconstruct-if\nconstruct-binop eq\nconstruct-var s\n\
             move-next-sibling\nconstruct-num 1\nmove-parent\nmove-next-sibling\n\
             construct-num 2\nmove-next-sibling\nconstruct-num 0\n",
        )
        .expect("the pre-variant encoding still replays");
        let shape = recognize(&program.exp()).expect("an if-chain is still a state machine");
        assert_eq!(shape.rows.len(), 3);
        assert!(matches!(shape.rows[0].cond, Cond::Eq(_, 0)));
        assert!(matches!(shape.rows[1].cond, Cond::Eq(_, 1)));
        assert!(matches!(shape.rows[2].cond, Cond::Else));
    }

    #[test]
    fn none_of_the_ten_core_examples_are_mistaken_for_a_state_machine() {
        let examples: Vec<Exp> = vec![
            examples::let_identity(),
            examples::increment_applied(),
            examples::clamp_to_one(),
            examples::pair_and_project(),
            examples::pair_with_empty_hole(),
            examples::add_with_empty_hole(),
            examples::square_and_compare(),
            examples::identity_hole_annotated_applied(),
            examples::add_with_non_empty_hole(),
            examples::if_over_pairs_with_hole(),
        ];
        for e in examples {
            assert!(recognize(&e).is_none());
        }
    }

    #[test]
    fn factorial_does_not_match_only_one_comparison() {
        assert!(recognize(&AppState::factorial().program()).is_none());
    }

    #[test]
    fn a_match_of_fewer_than_three_cases_is_not_a_state_machine() {
        let program = replay_script(
            "construct-lam\nmove-parent\nrename s\nmove-child 0\nconstruct-var s\n\
             construct-match\nadd-arm\nadd-arm\n",
        )
        .expect("two arms replay");
        assert!(recognize(&program.exp()).is_none());
    }

    #[test]
    fn the_table_shows_every_row() {
        let state = state_machine_state();
        let text = marked_text(&state);
        assert!(text.contains("state machine on s"), "{text}");
        assert!(text.contains("Idle"), "{text}");
        assert!(text.contains("Running"), "{text}");
        assert!(text.contains("Stopped"), "{text}");
    }

    #[test]
    fn moving_onto_a_cell_marks_it_in_the_table() {
        let state = state_machine_state()
            .apply_actions(&[Action::MoveChild(0), Action::MoveChild(1)])
            .expect("row 0's result is reachable");
        let text = marked_text(&state);
        assert!(text.contains(CURSOR_OPEN));
        assert!(text.contains(CURSOR_CLOSE));
    }
}
