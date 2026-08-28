use nothing_action::cursor_render::{CURSOR_CLOSE, CURSOR_OPEN};
use nothing_action::zipper::{Frame, Zipper};
use nothing_core::exp::{Exp, Op, Side};
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;

use crate::app::{AppState, Slot};

pub fn phrase(exp: &Exp, names: &NameTable) -> String {
    match exp {
        Exp::Var(id) => names.display(*id),
        Exp::Num(n) => n.to_string(),
        Exp::Bool(b) => if *b { "yes" } else { "no" }.to_string(),
        Exp::EmptyHole(_) => "(blank)".to_string(),
        Exp::NonEmptyHole(_, e) => format!("(not yet fitting: {})", phrase(e, names)),
        Exp::Pair(a, b) => format!("the pair of {} and {}", phrase(a, names), phrase(b, names)),
        Exp::Proj(side, e) => format!("the {} part of {}", side_word(*side), phrase(e, names)),
        Exp::Ap(f, a) => format!("{} applied to {}", phrase(f, names), phrase(a, names)),
        Exp::BinOp(op, l, r) => binop_phrase(*op, &phrase(l, names), &phrase(r, names)),
        Exp::If(c, t, e) => format!(
            "if {} then {} otherwise {}",
            phrase(c, names),
            phrase(t, names),
            phrase(e, names)
        ),
        Exp::Let(id, bound, body) => format!(
            "let {} be {}, then {}",
            names.display(*id),
            phrase(bound, names),
            phrase(body, names)
        ),
        Exp::Lam(id, ty, body) => format!(
            "a function taking {} ({}) and returning {}",
            names.display(*id),
            ty_phrase(ty),
            phrase(body, names)
        ),
    }
}

fn side_word(side: Side) -> &'static str {
    match side {
        Side::L => "first",
        Side::R => "second",
    }
}

fn binop_phrase(op: Op, l: &str, r: &str) -> String {
    match op {
        Op::Add => format!("the sum of {l} and {r}"),
        Op::Sub => format!("the difference between {l} and {r}"),
        Op::Mul => format!("the product of {l} and {r}"),
        Op::Lt => format!("whether {l} is less than {r}"),
        Op::Eq => format!("whether {l} equals {r}"),
    }
}

pub fn ty_phrase(ty: &Ty) -> String {
    match ty {
        Ty::Num => "a number".to_string(),
        Ty::Bool => "a yes-or-no value".to_string(),
        Ty::Hole => "an unknown type".to_string(),
        Ty::Arrow(a, b) => format!("a function from {} to {}", ty_phrase(a), ty_phrase(b)),
        Ty::Prod(a, b) => format!("a pair of {} and {}", ty_phrase(a), ty_phrase(b)),
    }
}

fn assemble(frame: &Frame, child: &str, names: &NameTable) -> String {
    match frame {
        Frame::LamBody(id, ty) => format!(
            "a function taking {} ({}) and returning {child}",
            names.display(*id),
            ty_phrase(ty)
        ),
        Frame::ApFun(arg) => format!("{child} applied to {}", phrase(arg, names)),
        Frame::ApArg(fun) => format!("{} applied to {child}", phrase(fun, names)),
        Frame::BinOpLeft(op, rhs) => binop_phrase(*op, child, &phrase(rhs, names)),
        Frame::BinOpRight(op, lhs) => binop_phrase(*op, &phrase(lhs, names), child),
        Frame::IfCond(then_, else_) => format!(
            "if {child} then {} otherwise {}",
            phrase(then_, names),
            phrase(else_, names)
        ),
        Frame::IfThen(cond, else_) => format!(
            "if {} then {child} otherwise {}",
            phrase(cond, names),
            phrase(else_, names)
        ),
        Frame::IfElse(cond, then_) => format!(
            "if {} then {} otherwise {child}",
            phrase(cond, names),
            phrase(then_, names)
        ),
        Frame::LetBound(id, body) => format!(
            "let {} be {child}, then {}",
            names.display(*id),
            phrase(body, names)
        ),
        Frame::LetBody(id, bound) => format!(
            "let {} be {}, then {child}",
            names.display(*id),
            phrase(bound, names)
        ),
        Frame::PairFst(snd) => format!("the pair of {child} and {}", phrase(snd, names)),
        Frame::PairSnd(fst) => format!("the pair of {} and {child}", phrase(fst, names)),
        Frame::ProjBody(side) => format!("the {} part of {child}", side_word(*side)),
        Frame::NonEmptyHoleBody(_) => format!("(not yet fitting: {child})"),
    }
}

pub fn render_with_cursor(z: &Zipper, names: &NameTable) -> String {
    let mut content = format!("{CURSOR_OPEN}{}{CURSOR_CLOSE}", phrase(&z.focus, names));
    for frame in z.path.iter().rev() {
        content = assemble(frame, &content, names);
    }
    content
}

pub fn marked_text(state: &AppState) -> String {
    if state.slot != Slot::Node {
        return crate::render::program_line(state);
    }
    render_with_cursor(state.zipper(), state.names())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_action::zipper::unzip;
    use nothing_core::examples;

    fn names() -> NameTable {
        examples::names()
    }

    #[test]
    fn no_operator_symbols_leak_into_the_prose() {
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
        for e in &examples {
            let text = phrase(e, &names());
            for symbol in ["+", "-", "*", "<", "==", "->"] {
                assert!(
                    !text.contains(symbol),
                    "beginner rendering of {e:?} still contains `{symbol}`: {text}"
                );
            }
        }
    }

    #[test]
    fn snapshot_let_identity() {
        assert_eq!(
            phrase(&examples::let_identity(), &names()),
            "let x0 be 1, then x0"
        );
    }

    #[test]
    fn snapshot_increment_applied() {
        assert_eq!(
            phrase(&examples::increment_applied(), &names()),
            "a function taking x0 (a number) and returning the sum of x0 and 1 applied to 41"
        );
    }

    #[test]
    fn snapshot_clamp_to_one() {
        assert_eq!(
            phrase(&examples::clamp_to_one(), &names()),
            "a function taking x0 (a number) and returning if whether x0 is less than 1 then 1 otherwise x0"
        );
    }

    #[test]
    fn snapshot_pair_and_project() {
        assert_eq!(
            phrase(&examples::pair_and_project(), &names()),
            "let x0 be the pair of 1 and yes, then the first part of x0"
        );
    }

    #[test]
    fn snapshot_pair_with_empty_hole() {
        assert_eq!(
            phrase(&examples::pair_with_empty_hole(), &names()),
            "the pair of (blank) and 2"
        );
    }

    #[test]
    fn snapshot_square_and_compare() {
        assert_eq!(
            phrase(&examples::square_and_compare(), &names()),
            "let x0 be a function taking x1 (a number) and returning the product of x1 and x1, then whether x0 applied to 5 equals 25"
        );
    }

    #[test]
    fn snapshot_identity_hole_annotated_applied() {
        assert_eq!(
            phrase(&examples::identity_hole_annotated_applied(), &names()),
            "a function taking x0 (an unknown type) and returning x0 applied to yes"
        );
    }

    #[test]
    fn snapshot_add_with_empty_hole() {
        assert_eq!(
            phrase(&examples::add_with_empty_hole(), &names()),
            "the sum of 1 and (blank)"
        );
    }

    #[test]
    fn snapshot_add_with_non_empty_hole() {
        assert_eq!(
            phrase(&examples::add_with_non_empty_hole(), &names()),
            "the sum of 1 and (not yet fitting: yes)"
        );
    }

    #[test]
    fn snapshot_if_over_pairs_with_hole() {
        assert_eq!(
            phrase(&examples::if_over_pairs_with_hole(), &names()),
            "if yes then the pair of 1 and 2 otherwise the pair of (blank) and 4"
        );
    }

    #[test]
    fn stripping_markers_reproduces_the_plain_beginner_projection() {
        for e in [
            examples::let_identity(),
            examples::increment_applied(),
            examples::clamp_to_one(),
            examples::pair_and_project(),
            examples::square_and_compare(),
            examples::if_over_pairs_with_hole(),
        ] {
            let expected = phrase(&e, &names());
            for z in nothing_action::zipper::all_positions(&e) {
                let marked = render_with_cursor(&z, &names());
                let stripped = marked.replace(CURSOR_OPEN, "").replace(CURSOR_CLOSE, "");
                assert_eq!(stripped, expected, "mismatch at {:?}", z.path);
            }
        }
    }

    #[test]
    fn snapshot_factorial_fixture() {
        let state = AppState::factorial();
        assert_eq!(
            phrase(&state.program(), state.names()),
            "a function taking x0 (a number) and returning if whether x0 equals 0 then 1 otherwise the product of x0 and (blank)"
        );
    }

    #[test]
    fn snapshot_state_machine_fixture() {
        use nothing_action::script::replay_script;

        const STATE_MACHINE_FIXTURE: &str =
            include_str!("../../bench/fixtures/state_machine.actions");
        let replayed = replay_script(STATE_MACHINE_FIXTURE)
            .expect("the embedded state machine fixture must replay cleanly");
        assert_eq!(
            phrase(&replayed.exp(), &replayed.names),
            "a function taking x0 (a number) and returning if whether x0 equals 0 then 1 otherwise if whether x0 equals 1 then 2 otherwise 0"
        );
    }

    #[test]
    fn the_root_is_delimited_once() {
        let e = examples::square_and_compare();
        let z = unzip(e);
        let marked = render_with_cursor(&z, &names());
        assert_eq!(marked.matches(CURSOR_OPEN).count(), 1);
        assert_eq!(marked.matches(CURSOR_CLOSE).count(), 1);
    }
}
