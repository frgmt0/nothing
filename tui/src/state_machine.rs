use nothing_action::act::Action;
use nothing_action::cursor_render::{CURSOR_CLOSE, CURSOR_OPEN};
use nothing_action::zipper::unzip;
use nothing_core::exp::{Exp, Id, Op};
use nothing_core::render::render;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{AppState, Slot, index_path};

#[derive(Clone, Debug)]
pub struct Row {
    pub cond: Option<(Vec<usize>, i64)>,
    pub result: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct Shape {
    pub var: Id,
    pub rows: Vec<Row>,
}

pub fn recognize(program: &Exp) -> Option<Shape> {
    let Exp::Lam(id, _, _) = program else {
        return None;
    };
    let var = *id;
    let mut rows = Vec::new();
    let mut z = unzip(program.clone()).move_child(0)?;
    loop {
        let Exp::If(cond, _, _) = z.focus.clone() else {
            break;
        };
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
            cond: Some((cond_path, n)),
            result: result_path,
        });
        z = z.move_child(2)?;
    }
    rows.push(Row {
        cond: None,
        result: index_path(&z),
    });

    if rows.len() < 3 {
        return None;
    }
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
            Some((_, n)) => format!("{var_name} == {n}"),
            None => "else".to_string(),
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
        let cond_marked = row.cond.as_ref().is_some_and(|(p, _)| *p == focus_path);
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
        if let Some((cond_path, _)) = &row.cond
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
        (Col::Cond, Some((path, _))) => path.clone(),
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
    if shape.rows[target_row].cond.is_none() && target_col == Col::Cond {
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
        assert_eq!(shape.rows[0].cond.as_ref().map(|(_, n)| *n), Some(0));
        assert_eq!(shape.rows[1].cond.as_ref().map(|(_, n)| *n), Some(1));
        assert!(shape.rows[2].cond.is_none());
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
    fn the_table_shows_every_row() {
        let state = state_machine_state();
        let text = marked_text(&state);
        assert!(text.contains("x0 == 0"));
        assert!(text.contains("x0 == 1"));
        assert!(text.contains("else"));
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
