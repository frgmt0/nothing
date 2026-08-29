use nothing_action::cursor_render::{CURSOR_CLOSE, CURSOR_OPEN};
use nothing_action::zipper::{Frame, Zipper};
use nothing_core::exp::{Exp, Id, Op, Side};
use nothing_core::names::NameTable;
use nothing_core::render::quote_str;
use nothing_core::ty::Ty;

use crate::app::{AppState, Slot};

pub fn phrase(exp: &Exp, names: &NameTable) -> String {
    match exp {
        Exp::Var(id) => names.display(*id),
        Exp::Num(n) => n.to_string(),
        Exp::Bool(b) => if *b { "yes" } else { "no" }.to_string(),
        Exp::Str(text) => format!("the text {}", quote_str(text)),
        Exp::EmptyHole(_) => "(blank)".to_string(),
        Exp::NonEmptyHole(_, e) => format!("(not yet fitting: {})", phrase(e, names)),
        Exp::Pair(a, b) => format!("the pair of {} and {}", phrase(a, names), phrase(b, names)),
        Exp::Nil => "an empty list".to_string(),
        Exp::Cons(head, tail) => format!(
            "{} in front of {}",
            phrase(head, names),
            phrase(tail, names)
        ),
        Exp::Fold(list, init, step) => format!(
            "combining {}, starting from {}, with {}",
            phrase(list, names),
            phrase(init, names),
            phrase(step, names)
        ),
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
        Exp::Record(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(id, value)| field_phrase(*id, &phrase(value, names), names))
                .collect();
            record_phrase(&parts)
        }
        Exp::Field(subject, id) => {
            format!("the {} of {}", names.display(*id), phrase(subject, names))
        }
        Exp::Print(text) => print_phrase(&phrase(text, names)),
        Exp::Readline => readline_phrase(),
        Exp::CmdPure(value) => pure_phrase(&phrase(value, names)),
        Exp::CmdBind(command, id, body) => bind_phrase(
            &phrase(command, names),
            *id,
            nothing_core::doc::references(body, *id),
            &phrase(body, names),
            names,
        ),
        Exp::Inj(ctor, payload) => inj_phrase(*ctor, &phrase(payload, names), names),
        Exp::Match(scrutinee, arms) => {
            let parts: Vec<String> = arms
                .iter()
                .map(|(ctor, binder, body)| arm_phrase(*ctor, *binder, &phrase(body, names), names))
                .collect();
            match_phrase(&phrase(scrutinee, names), &parts)
        }
    }
}

fn print_phrase(text: &str) -> String {
    format!("print {text}")
}

fn readline_phrase() -> String {
    "read a line of text from whoever runs this".to_string()
}

fn pure_phrase(value: &str) -> String {
    format!("do nothing and hand back {value}")
}

fn bind_phrase(
    command: &str,
    id: Id,
    uses_the_result: bool,
    body: &str,
    names: &NameTable,
) -> String {
    if uses_the_result {
        format!(
            "{command}, then with the result named {}, {body}",
            names.display(id)
        )
    } else {
        format!("{command}, then, ignoring the result, {body}")
    }
}

fn inj_phrase(ctor: Id, payload: &str, names: &NameTable) -> String {
    if payload == record_phrase(&[]) {
        return format!("{} carrying nothing", names.display(ctor));
    }
    format!("{} carrying {payload}", names.display(ctor))
}

fn arm_phrase(ctor: Id, binder: Id, body: &str, names: &NameTable) -> String {
    format!(
        "when it is {} {}: {body}",
        names.display(ctor),
        names.display(binder)
    )
}

fn match_phrase(scrutinee: &str, arms: &[String]) -> String {
    match arms.split_last() {
        None => format!("looking at {scrutinee}, which has no cases yet"),
        Some((last, [])) => format!("looking at {scrutinee}, {last}"),
        Some((last, rest)) => format!("looking at {scrutinee}, {}, and {last}", rest.join(", ")),
    }
}

fn field_phrase(id: Id, value: &str, names: &NameTable) -> String {
    format!("{} set to {value}", names.display(id))
}

fn record_phrase(parts: &[String]) -> String {
    match parts.split_last() {
        None => "a record with no fields".to_string(),
        Some((last, [])) => format!("a record with {last}"),
        Some((last, rest)) => format!("a record with {} and {last}", rest.join(", ")),
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
        Op::Concat => format!("{l} followed by {r}"),
    }
}

pub fn ty_phrase(ty: &Ty) -> String {
    match ty {
        Ty::Num => "a number".to_string(),
        Ty::Bool => "a yes-or-no value".to_string(),
        Ty::Str => "a piece of text".to_string(),
        Ty::Hole => "an unknown type".to_string(),
        Ty::Arrow(a, b) => format!("a function from {} to {}", ty_phrase(a), ty_phrase(b)),
        Ty::Prod(a, b) => format!("a pair of {} and {}", ty_phrase(a), ty_phrase(b)),
        Ty::List(elem) => format!("a list of {}", ty_phrase(elem)),
        Ty::Record(fields) => match fields.len() {
            0 => "a record with no fields".to_string(),
            1 => "a record with 1 field".to_string(),
            n => format!("a record with {n} fields"),
        },
        Ty::Variant(ctors) => match ctors.len() {
            0 => "a choice with no cases".to_string(),
            1 => "a choice with 1 case".to_string(),
            n => format!("a choice between {n} cases"),
        },
        Ty::Cmd(result) => format!("a command that produces {}", ty_phrase(result)),
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
        Frame::ConsHead(tail) => format!("{child} in front of {}", phrase(tail, names)),
        Frame::ConsTail(head) => format!("{} in front of {child}", phrase(head, names)),
        Frame::FoldList(init, step) => format!(
            "combining {child}, starting from {}, with {}",
            phrase(init, names),
            phrase(step, names)
        ),
        Frame::FoldInit(list, step) => format!(
            "combining {}, starting from {child}, with {}",
            phrase(list, names),
            phrase(step, names)
        ),
        Frame::FoldStep(list, init) => format!(
            "combining {}, starting from {}, with {child}",
            phrase(list, names),
            phrase(init, names)
        ),
        Frame::PrintText => print_phrase(child),
        Frame::PureValue => pure_phrase(child),
        Frame::BindCommand(id, body) => bind_phrase(
            child,
            *id,
            nothing_core::doc::references(body, *id),
            &phrase(body, names),
            names,
        ),
        Frame::BindBody(id, command) => {
            bind_phrase(&phrase(command, names), *id, true, child, names)
        }
        Frame::NonEmptyHoleBody(_) => format!("(not yet fitting: {child})"),
        Frame::RecordField(others, index, id) => {
            let mut parts: Vec<String> = others
                .iter()
                .map(|(other, value)| field_phrase(*other, &phrase(value, names), names))
                .collect();
            let here = field_phrase(*id, child, names);
            parts.insert((*index).min(parts.len()), here);
            record_phrase(&parts)
        }
        Frame::FieldSubject(id) => format!("the {} of {child}", names.display(*id)),
        Frame::InjPayload(ctor) => inj_phrase(*ctor, child, names),
        Frame::MatchScrutinee(arms) => {
            let parts: Vec<String> = arms
                .iter()
                .map(|(ctor, binder, body)| arm_phrase(*ctor, *binder, &phrase(body, names), names))
                .collect();
            match_phrase(child, &parts)
        }
        Frame::MatchArm(scrutinee, others, index, ctor, binder) => {
            let mut parts: Vec<String> = others
                .iter()
                .map(|(other, other_binder, body)| {
                    arm_phrase(*other, *other_binder, &phrase(body, names), names)
                })
                .collect();
            let here = arm_phrase(*ctor, *binder, child, names);
            parts.insert((*index).min(parts.len()), here);
            match_phrase(&phrase(scrutinee, names), &parts)
        }
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
            "a function taking x0 (a number) and returning if whether x0 equals 0 then 1 otherwise the product of x0 and main applied to the difference between x0 and 1"
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
            "a function taking s (an unknown type) and returning looking at s, \
             when it is Idle x0: Running carrying nothing, \
             when it is Running x1: Stopped carrying nothing, \
             and when it is Stopped x2: Idle carrying nothing"
        );
    }

    #[test]
    fn a_list_reads_as_a_list() {
        let e = Exp::list([Exp::num(1), Exp::num(2), Exp::num(3)]);
        assert_eq!(
            phrase(&e, &names()),
            "1 in front of 2 in front of 3 in front of an empty list"
        );
        assert_eq!(phrase(&Exp::nil(), &names()), "an empty list");
        assert_eq!(
            ty_phrase(&Ty::List(Box::new(Ty::Num))),
            "a list of a number"
        );
        assert_eq!(
            ty_phrase(&Ty::List(Box::new(Ty::List(Box::new(Ty::Bool))))),
            "a list of a list of a yes-or-no value"
        );
        assert_eq!(
            phrase(
                &Exp::fold(
                    e,
                    Exp::num(0),
                    Exp::var(nothing_core::exp::Id::from_u128(1))
                ),
                &names()
            ),
            "combining 1 in front of 2 in front of 3 in front of an empty list, \
             starting from 0, with _00000000"
        );
    }

    #[test]
    fn a_command_reads_as_something_to_do_and_a_bind_reads_as_then() {
        let mut names = names();
        let line = nothing_core::exp::Id::from_u128(0x11e);
        names.set(line, "line");

        assert_eq!(
            phrase(&Exp::print(Exp::str_("hi")), &names),
            "print the text \"hi\""
        );
        assert_eq!(
            phrase(&Exp::readline(), &names),
            "read a line of text from whoever runs this"
        );
        assert_eq!(
            phrase(&Exp::cmd_pure(Exp::num(1)), &names),
            "do nothing and hand back 1"
        );
        assert_eq!(
            ty_phrase(&Ty::Cmd(Box::new(Ty::Str))),
            "a command that produces a piece of text"
        );

        assert_eq!(
            phrase(
                &Exp::cmd_bind(Exp::readline(), line, Exp::print(Exp::var(line))),
                &names
            ),
            "read a line of text from whoever runs this, then with the result named line, \
             print line"
        );
        assert_eq!(
            phrase(
                &Exp::cmd_bind(
                    Exp::print(Exp::str_("first")),
                    line,
                    Exp::print(Exp::str_("second"))
                ),
                &names
            ),
            "print the text \"first\", then, ignoring the result, print the text \"second\"",
            "a bind whose binder is never mentioned is what another language would call seq"
        );
    }

    #[test]
    fn a_record_reads_by_name() {
        let mut names = names();
        let x = nothing_core::exp::Id::from_u128(0x1a);
        let y = nothing_core::exp::Id::from_u128(0x1b);
        names.set(x, "x");
        names.set(y, "y");

        let point = Exp::record([(x, Exp::num(1)), (y, Exp::num(2))]);
        assert_eq!(
            phrase(&point, &names),
            "a record with x set to 1 and y set to 2"
        );
        assert_eq!(
            phrase(&Exp::record([(x, Exp::num(1))]), &names),
            "a record with x set to 1"
        );
        assert_eq!(phrase(&Exp::record([]), &names), "a record with no fields");
        assert_eq!(
            phrase(&Exp::field(point.clone(), x), &names),
            "the x of a record with x set to 1 and y set to 2"
        );
        assert_eq!(
            ty_phrase(&Ty::Record(vec![(x, Ty::Num), (y, Ty::Bool)])),
            "a record with 2 fields"
        );
        assert_eq!(
            ty_phrase(&Ty::Record(Vec::new())),
            "a record with no fields"
        );

        let z = unzip(Exp::field(point, x));
        let z = z.move_child(0).expect("into the subject");
        let z = z.move_child(1).expect("into the second field");
        let marked = render_with_cursor(&z, &names);
        assert_eq!(
            marked.replace(CURSOR_OPEN, "").replace(CURSOR_CLOSE, ""),
            "the x of a record with x set to 1 and y set to 2",
            "the surrounding prose is rebuilt around whichever field the cursor is in"
        );
        assert_eq!(
            marked,
            format!("the x of a record with x set to 1 and y set to {CURSOR_OPEN}2{CURSOR_CLOSE}"),
            "and the cursor is on the field's value, not on the record"
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
