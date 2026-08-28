
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use nothing_core::exp::Exp;
use nothing_core::names::NameTable;
use nothing_core::typing::is_well_typed;
use nothing_eval::dynamic;
use nothing_eval::incr::IncrEngine;
use nothing_eval::step::{Blocked, HoleKind, Outcome};

use crate::app::AppState;

pub const LIVE_FUEL: usize = 4_000;

#[derive(Clone)]
pub struct EngineHandle(Rc<RefCell<IncrEngine>>);

impl EngineHandle {
    pub fn new() -> EngineHandle {
        EngineHandle(Rc::new(RefCell::new(IncrEngine::new())))
    }

    pub fn eval(&self, exp: &Exp, fuel: usize) -> Outcome {
        self.0.borrow_mut().eval_with_fuel(exp, fuel)
    }

    pub fn node_evals(&self) -> usize {
        self.0.borrow().node_evals
    }
}

impl Default for EngineHandle {
    fn default() -> EngineHandle {
        EngineHandle::new()
    }
}

impl PartialEq for EngineHandle {
    fn eq(&self, _other: &EngineHandle) -> bool {
        true
    }
}

impl fmt::Debug for EngineHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EngineHandle(node_evals={})", self.node_evals())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Subject {
    Focus,
    Program,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Live {
    pub subject: Subject,
    pub outcome: Outcome,
}

pub fn live(state: &AppState) -> Live {
    let (subject, exp) = subject_of(state);
    Live {
        subject,
        outcome: state.engine.eval(&exp, LIVE_FUEL),
    }
}

fn subject_of(state: &AppState) -> (Subject, Exp) {
    let focus = state.focus();
    if !matches!(focus, Exp::EmptyHole(_)) && is_well_typed(focus) {
        (Subject::Focus, focus.clone())
    } else {
        (Subject::Program, state.program())
    }
}

pub fn live_line(state: &AppState) -> String {
    let live = live(state);
    let lead = match live.subject {
        Subject::Focus => "⇒ ",
        Subject::Program => "program ⇒ ",
    };
    format!("{lead}{}", describe(&live.outcome, state.names()))
}

pub fn describe(outcome: &Outcome, names: &NameTable) -> String {
    match outcome {
        Outcome::Value(result) => dynamic::render(result, names),

        Outcome::Indeterminate { result, blocked } => {
            let mut line = dynamic::render(result, names);
            match blocked.split_first() {
                None => line.push_str(" · stuck"),
                Some((first, rest)) => {
                    line.push_str(&format!(" · blocked on {}", hole_label(first)));
                    if !rest.is_empty() {
                        line.push_str(&format!(" and {} more", rest.len()));
                    }
                    if let Some(scope) = scope_of(first, names) {
                        line.push_str(&format!(" · {scope}"));
                    }
                }
            }
            line
        }

        Outcome::OutOfFuel { steps, .. } => {
            format!("… still running after {steps} steps")
        }
    }
}

pub fn hole_label(blocked: &Blocked) -> String {
    let shape = match blocked.kind {
        HoleKind::Empty => "⦇⦈",
        HoleKind::NonEmpty => "⦇e⦈",
    };
    format!("{shape}#{}", blocked.hole.short())
}

pub fn scope_of(blocked: &Blocked, names: &NameTable) -> Option<String> {
    let known = blocked.known();
    if known.is_empty() {
        return None;
    }
    Some(
        known
            .iter()
            .map(|(id, value)| format!("{} = {}", names.display(*id), dynamic::render(value, names)))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{handle_key, key};
    use nothing_core::examples;
    use nothing_core::exp::{HoleId, Id, Op};
    use nothing_core::ty::Ty;
    use ratatui::crossterm::event::KeyCode;

    fn typed(text: &str) -> AppState {
        text.chars().fold(AppState::empty(), |state, c| {
            handle_key(key(KeyCode::Char(c)), state)
        })
    }

    #[test]
    fn a_finished_expression_shows_its_value() {
        let sum = handle_key(key(KeyCode::Up), typed("1+2"));
        assert_eq!(live_line(&sum), "⇒ 3");
        assert_eq!(live_line(&AppState::new(examples::let_identity())), "⇒ 1");
        assert_eq!(
            live_line(&AppState::new(examples::pair_and_project())),
            "⇒ 1"
        );
        assert_eq!(
            live_line(&AppState::new(examples::square_and_compare())),
            "⇒ true"
        );
        assert_eq!(
            live_line(&AppState::new(examples::increment_applied())),
            "⇒ 42"
        );
    }

    #[test]
    fn editing_an_expression_updates_its_displayed_value_with_no_run_command() {
        let one = typed("1");
        assert_eq!(live_line(&one), "⇒ 1");

        let open = handle_key(key(KeyCode::Char('+')), one);
        let line = live_line(&open);
        assert!(
            line.starts_with("program ⇒ 1 + ⦇⦈ · blocked on ⦇⦈#"),
            "an unwritten operand has no value of its own, so the program answers: {line}"
        );

        let two = handle_key(key(KeyCode::Char('2')), open);
        assert_eq!(live_line(&two), "⇒ 2", "the cursor is on the operand");
        assert_eq!(
            live_line(&handle_key(key(KeyCode::Up), two.clone())),
            "⇒ 3",
            "and on the sum, the sum"
        );

        let extended = handle_key(key(KeyCode::Char('3')), two);
        assert_eq!(live_line(&extended), "⇒ 23");
        assert_eq!(
            live_line(&handle_key(key(KeyCode::Up), extended)),
            "⇒ 24",
            "one keystroke changed the answer without any run command"
        );
    }

    #[test]
    fn the_value_of_the_whole_program_shows_when_the_focus_is_open() {
        let body = handle_key(key(KeyCode::Up), typed("\\x0:n.x0+1"));
        assert!(matches!(body.focus(), Exp::BinOp(..)));
        assert_eq!(
            live_line(&body),
            "program ⇒ λx0:Num. x0 + 1",
            "`x0 + 1` has a free variable, so the program answers instead"
        );
    }

    #[test]
    fn a_hole_makes_the_result_indeterminate_and_names_the_hole() {
        let state = AppState::with_names(examples::add_with_empty_hole(), examples::names());
        let line = live_line(&state);
        assert!(line.starts_with("⇒ 1 + ⦇⦈ · blocked on ⦇⦈#"), "{line}");
    }

    #[test]
    fn an_indeterminate_result_shows_what_was_in_scope_where_it_stopped() {
        let x = Id::from_u128(0x5a);
        let mut names = NameTable::new();
        names.set(x, "n");

        let program = Exp::ap(
            Exp::lam(x, Ty::Num, Exp::empty_hole(HoleId::from_u128(1))),
            Exp::num(5),
        );
        let state = AppState::with_names(program, names);
        let line = live_line(&state);
        assert!(line.contains("blocked on ⦇⦈#"), "{line}");
        assert!(line.ends_with(" · n = 5"), "{line}");
    }

    #[test]
    fn a_quarantined_expression_is_reported_as_the_other_kind_of_hole() {
        let state = AppState::with_names(examples::add_with_non_empty_hole(), examples::names());
        let line = live_line(&state);
        assert!(line.starts_with("⇒ 1 + ⦇true⦈ · blocked on ⦇e⦈#"), "{line}");
    }

    #[test]
    fn a_non_terminating_program_says_so_instead_of_hanging() {
        let x = Id::from_u128(0x77);
        let omega = Exp::lam(x, Ty::Hole, Exp::ap(Exp::var(x), Exp::var(x)));
        let state = AppState::new(Exp::ap(omega.clone(), omega));

        let line = live_line(&state);
        assert_eq!(line, format!("⇒ … still running after {LIVE_FUEL} steps"));
        assert!(!line.contains("blocked"), "distinct from indeterminate");
    }

    #[test]
    fn a_recursive_program_computes_live() {
        use nothing_eval::fixpoint::{Binders, fix};

        let binders = Binders {
            f: Id::from_u128(0x10),
            x: Id::from_u128(0x11),
            v: Id::from_u128(0x12),
        };
        let fac = Id::from_u128(0x20);
        let n = Id::from_u128(0x21);

        let generator = Exp::lam(
            fac,
            Ty::Hole,
            Exp::lam(
                n,
                Ty::Num,
                Exp::if_(
                    Exp::bin_op(Op::Eq, Exp::var(n), Exp::num(0)),
                    Exp::num(1),
                    Exp::bin_op(
                        Op::Mul,
                        Exp::var(n),
                        Exp::ap(
                            Exp::var(fac),
                            Exp::bin_op(Op::Sub, Exp::var(n), Exp::num(1)),
                        ),
                    ),
                ),
            ),
        );
        let program = Exp::ap(fix(binders, generator), Exp::num(5));
        let state = AppState::new(program);
        assert_eq!(live_line(&state), "⇒ 120");
    }
}
