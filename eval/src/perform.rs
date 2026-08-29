use std::collections::VecDeque;

use nothing_core::doc::Doc;
use nothing_core::exp::Id;
use nothing_core::ty::Ty;
use nothing_core::typing::{join, syn};

use crate::dynamic::{Dyn, elaborate, subst};
use crate::step::{Defs, Outcome, blocked_holes, defs_of, run_in_counted};

pub trait Io {
    fn write_line(&mut self, text: &str);
    fn read_line(&mut self) -> Option<String>;
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Recorded {
    pub written: Vec<String>,
    pub to_read: VecDeque<String>,
}

impl Recorded {
    pub fn with_input(lines: impl IntoIterator<Item = String>) -> Recorded {
        Recorded {
            written: Vec::new(),
            to_read: lines.into_iter().collect(),
        }
    }
}

impl Io for Recorded {
    fn write_line(&mut self, text: &str) {
        self.written.push(text.to_string());
    }

    fn read_line(&mut self) -> Option<String> {
        self.to_read.pop_front()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Performance {
    pub outcome: Outcome,
    pub commands: usize,
    pub steps: usize,
}

pub fn is_command_type(ty: &Ty) -> bool {
    matches!(ty, Ty::Cmd(_))
}

pub fn main_type(doc: &Doc, main: Id) -> Ty {
    let Some(def) = doc.get(main) else {
        return Ty::Hole;
    };
    let synthesised = syn(&doc.ctx(), &def.body).unwrap_or(Ty::Hole);
    join(&def.ann, &synthesised).unwrap_or(synthesised)
}

pub fn runs_as_a_command(doc: &Doc, main: Id) -> bool {
    is_command_type(&main_type(doc, main))
}

pub fn perform_doc(doc: &Doc, main: Id, fuel: usize, io: &mut dyn Io) -> Performance {
    let defs = defs_of(doc);
    let start = match doc.get(main) {
        Some(def) => elaborate(&def.body),
        None => Dyn::Var(main),
    };
    perform_in(&defs, start, fuel, io)
}

pub fn perform_in(defs: &Defs, start: Dyn, fuel: usize, io: &mut dyn Io) -> Performance {
    let mut pending: Vec<(Id, Dyn)> = Vec::new();
    let mut current = start;
    let mut left = fuel;
    let mut steps = 0usize;
    let mut commands = 0usize;

    loop {
        let (outcome, used) = run_in_counted(defs, current, left);
        left -= used;
        steps += used;

        let value = match outcome {
            Outcome::Value(value) => value,
            Outcome::Indeterminate { result, blocked } => {
                let result = rebuild(result, pending);
                return Performance {
                    outcome: Outcome::Indeterminate { result, blocked },
                    commands,
                    steps,
                };
            }
            Outcome::OutOfFuel { partial, .. } => {
                return Performance {
                    outcome: Outcome::OutOfFuel {
                        partial: rebuild(partial, pending),
                        steps,
                    },
                    commands,
                    steps,
                };
            }
        };

        if left == 0 {
            return Performance {
                outcome: Outcome::OutOfFuel {
                    partial: rebuild(value, pending),
                    steps,
                },
                commands,
                steps,
            };
        }

        let yielded = match value {
            Dyn::CmdBind(command, id, body) => {
                left -= 1;
                commands += 1;
                pending.push((id, *body));
                current = *command;
                continue;
            }
            Dyn::Print(text) => {
                left -= 1;
                commands += 1;
                match *text {
                    Dyn::Str(line) => {
                        io.write_line(&line);
                        Dyn::Record(Vec::new())
                    }
                    other => {
                        return stuck(Dyn::Print(Box::new(other)), pending, commands, steps);
                    }
                }
            }
            Dyn::Readline => {
                left -= 1;
                commands += 1;
                match io.read_line() {
                    Some(line) => Dyn::Str(line),
                    None => return stuck(Dyn::Readline, pending, commands, steps),
                }
            }
            Dyn::CmdPure(inner) => {
                left -= 1;
                commands += 1;
                *inner
            }
            other => return stuck(other, pending, commands, steps),
        };

        match pending.pop() {
            None => {
                return Performance {
                    outcome: Outcome::Value(yielded),
                    commands,
                    steps,
                };
            }
            Some((id, body)) => current = subst(id, &yielded, &body),
        }
    }
}

fn stuck(residual: Dyn, pending: Vec<(Id, Dyn)>, commands: usize, steps: usize) -> Performance {
    let result = rebuild(residual, pending);
    let blocked = blocked_holes(&result);
    Performance {
        outcome: Outcome::Indeterminate { result, blocked },
        commands,
        steps,
    }
}

fn rebuild(residual: Dyn, pending: Vec<(Id, Dyn)>) -> Dyn {
    let mut out = residual;
    for (id, body) in pending.into_iter().rev() {
        out = Dyn::CmdBind(Box::new(out), id, Box::new(body));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::exp::{Exp, HoleId, Op};
    use nothing_core::names::NameTable;
    use nothing_core::ty::{cmd, unit};

    use crate::dynamic::render;
    use crate::step::DEFAULT_FUEL;

    fn x(n: u128) -> Id {
        Id::from_u128(n)
    }

    fn h(n: u128) -> HoleId {
        HoleId::from_u128(n)
    }

    fn perform(exp: Exp, io: &mut dyn Io) -> Performance {
        perform_in(&Defs::new(), elaborate(&exp), DEFAULT_FUEL, io)
    }

    #[test]
    fn a_lone_print_writes_one_line_and_yields_nothing() {
        let mut io = Recorded::default();
        let out = perform(Exp::print(Exp::str_("hello, world")), &mut io);
        assert_eq!(io.written, vec!["hello, world".to_string()]);
        assert_eq!(out.outcome, Outcome::Value(Dyn::Record(Vec::new())));
        assert_eq!(out.commands, 1);
    }

    #[test]
    fn a_bind_chain_performs_its_commands_in_order() {
        let a = x(1);
        let program = Exp::cmd_bind(
            Exp::print(Exp::str_("first")),
            a,
            Exp::cmd_bind(
                Exp::print(Exp::str_("second")),
                x(2),
                Exp::print(Exp::str_("third")),
            ),
        );
        let mut io = Recorded::default();
        let out = perform(program, &mut io);
        assert_eq!(io.written, vec!["first", "second", "third"]);
        assert!(out.outcome.is_value());
    }

    #[test]
    fn readline_is_substituted_into_the_rest_of_the_chain() {
        let line = x(1);
        let program = Exp::cmd_bind(
            Exp::readline(),
            line,
            Exp::print(Exp::bin_op(
                Op::Concat,
                Exp::str_("hello, "),
                Exp::var(line),
            )),
        );
        let mut io = Recorded::with_input(["Ada".to_string()]);
        let out = perform(program, &mut io);
        assert_eq!(io.written, vec!["hello, Ada".to_string()]);
        assert!(out.outcome.is_value());
    }

    #[test]
    fn pure_hands_its_value_back_without_writing_anything() {
        let mut io = Recorded::default();
        let out = perform(Exp::cmd_pure(Exp::num(7)), &mut io);
        assert!(io.written.is_empty());
        assert_eq!(out.outcome, Outcome::Value(Dyn::Num(7)));
    }

    #[test]
    fn a_hole_between_two_prints_stops_after_the_first_one() {
        let program = Exp::cmd_bind(
            Exp::print(Exp::str_("first")),
            x(1),
            Exp::cmd_bind(Exp::empty_hole(h(9)), x(2), Exp::print(Exp::str_("second"))),
        );
        let mut io = Recorded::default();
        let out = perform(program, &mut io);
        assert_eq!(
            io.written,
            vec!["first".to_string()],
            "the print before the hole happened and the one after it did not"
        );
        let Outcome::Indeterminate { result, blocked } = &out.outcome else {
            panic!("expected an indeterminate report, got {:?}", out.outcome);
        };
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].hole, h(9));
        let mut names = NameTable::new();
        names.set(x(2), "b");
        assert_eq!(
            render(result, &names),
            "bind b <- ⦇⦈ in print \"second\"",
            "the residual is the part of the program that has not run"
        );
    }

    #[test]
    fn a_readline_after_a_hole_never_asks_for_input() {
        let program = Exp::cmd_bind(
            Exp::empty_hole(h(3)),
            x(1),
            Exp::cmd_bind(Exp::readline(), x(2), Exp::print(Exp::var(x(2)))),
        );
        let mut io = Recorded::with_input(["never read".to_string()]);
        let out = perform(program, &mut io);
        assert!(io.written.is_empty());
        assert_eq!(
            io.to_read.len(),
            1,
            "the line waiting on standard input was not consumed"
        );
        assert!(out.outcome.is_indeterminate());
        assert_eq!(out.outcome.blocked()[0].hole, h(3));
    }

    #[test]
    fn a_readline_with_nothing_left_to_read_stops_rather_than_inventing_a_line() {
        let mut io = Recorded::default();
        let out = perform(Exp::readline(), &mut io);
        assert!(out.outcome.is_stuck());
    }

    #[test]
    fn an_endless_bind_loop_runs_out_of_fuel_instead_of_running_forever() {
        let loop_id = x(1);
        let body = Exp::cmd_bind(Exp::print(Exp::str_("x")), x(2), Exp::var(loop_id));
        let defs: Defs = [(loop_id, elaborate(&body))].into_iter().collect();
        let mut io = Recorded::default();
        let out = perform_in(&defs, elaborate(&body), 40, &mut io);
        assert!(
            out.outcome.is_out_of_fuel(),
            "expected out-of-fuel, got {:?}",
            out.outcome
        );
        assert!(
            !io.written.is_empty(),
            "the loop printed what it printed before the budget ran out"
        );
        assert!(io.written.len() < 40, "the budget bounded the output");
        assert!(io.written.iter().all(|line| line == "x"));
    }

    #[test]
    fn a_command_is_recognised_by_the_type_of_main_however_it_is_written() {
        assert!(is_command_type(&cmd(unit())));
        assert!(is_command_type(&cmd(Ty::Str)));
        assert!(!is_command_type(&Ty::Str));
        assert!(!is_command_type(&Ty::Hole));

        let main = x(1);
        let doc = Doc::new(vec![nothing_core::doc::Def::new(
            main,
            Ty::Hole,
            Exp::print(Exp::str_("hi")),
        )])
        .expect("one definition");
        assert!(
            runs_as_a_command(&doc, main),
            "an unannotated main whose body is a command still runs"
        );

        let empty = Doc::new(vec![nothing_core::doc::Def::new(
            main,
            cmd(unit()),
            Exp::empty_hole(h(1)),
        )])
        .expect("one definition");
        assert!(
            runs_as_a_command(&empty, main),
            "a main annotated Cmd runs even while its body is still a hole"
        );

        let number = Doc::new(vec![nothing_core::doc::Def::new(
            main,
            Ty::Hole,
            Exp::num(1),
        )])
        .expect("one definition");
        assert!(!runs_as_a_command(&number, main));
    }
}
