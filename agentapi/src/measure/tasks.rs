#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    FillHole,
    BuildFunction,
    FixQuarantine,
    ExtendProgram,
}

impl Family {
    pub fn label(self) -> &'static str {
        match self {
            Family::FillHole => "fill a hole",
            Family::BuildFunction => "build a small function",
            Family::FixQuarantine => "fix a quarantine",
            Family::ExtendProgram => "extend a program",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Task {
    pub name: &'static str,
    pub family: Family,
    pub goal: &'static str,
    pub setup: &'static str,
    pub target: &'static str,
}

pub fn tasks() -> Vec<Task> {
    vec![
        Task {
            name: "fill_double",
            family: Family::FillHole,
            goal: "The hole is the right operand of a multiplication. Make the function double its argument.",
            setup: "construct-lam\nmove-parent\nrename n\nset-ann Num\nmove-child 0\nconstruct-var n\nconstruct-binop mul\n",
            target: "λn:Num. n * 2",
        },
        Task {
            name: "fill_increment",
            family: Family::FillHole,
            goal: "The hole is the right operand of an addition. Make the function add one to its argument.",
            setup: "construct-lam\nmove-parent\nrename n\nset-ann Num\nmove-child 0\nconstruct-var n\nconstruct-binop add\n",
            target: "λn:Num. n + 1",
        },
        Task {
            name: "fill_condition",
            family: Family::FillHole,
            goal: "The hole is the condition of an if. It should test whether n is less than 0.",
            setup: "construct-lam\nmove-parent\nrename n\nset-ann Num\nmove-child 0\nconstruct-if\nmove-parent\nmove-child 1\nconstruct-num 1\nmove-next-sibling\nconstruct-num 0\nmove-parent\nmove-child 0\n",
            target: "λn:Num. if n < 0 then 1 else 0",
        },
        Task {
            name: "fill_then_branch",
            family: Family::FillHole,
            goal: "The hole is the then-branch. It should be the number 1.",
            setup: "construct-lam\nmove-parent\nrename n\nset-ann Num\nmove-child 0\nconstruct-if\nconstruct-var n\nconstruct-binop eq\nconstruct-num 0\nmove-parent\nmove-parent\nmove-child 1\n",
            target: "λn:Num. if n == 0 then 1 else ⦇⦈",
        },
        Task {
            name: "fill_pair_second",
            family: Family::FillHole,
            goal: "The hole is the second component of a pair. It should be the boolean true.",
            setup: "construct-num 1\nconstruct-pair\n",
            target: "(1, true)",
        },
        Task {
            name: "fill_constant_false",
            family: Family::FillHole,
            goal: "The hole is the body of a function taking a Bool. The function should always return false, ignoring its argument.",
            setup: "construct-lam\nmove-parent\nrename b\nset-ann Bool\nmove-child 0\n",
            target: "λb:Bool. false",
        },
        Task {
            name: "fill_projection_operand",
            family: Family::FillHole,
            goal: "The hole is the operand of a first-component projection. It should be the parameter p.",
            setup: "construct-lam\nmove-parent\nrename p\nset-ann Num * Num\nmove-child 0\nconstruct-proj l\n",
            target: "λp:Num * Num. fst p",
        },
        Task {
            name: "fill_application_argument",
            family: Family::FillHole,
            goal: "The hole is the argument of an application of f. Apply f to 10.",
            setup: "construct-lam\nmove-parent\nrename f\nset-ann Num -> Num\nmove-child 0\nconstruct-var f\nconstruct-ap\n",
            target: "λf:Num -> Num. f 10",
        },
        Task {
            name: "fill_let_body",
            family: Family::FillHole,
            goal: "The hole is the body of a let binding x to 5. The body should be x multiplied by itself.",
            setup: "construct-num 5\nconstruct-let\nmove-parent\nrename x\nmove-child 1\n",
            target: "let x = 5 in x * x",
        },
        Task {
            name: "fill_comparison_rhs",
            family: Family::FillHole,
            goal: "The hole is the right operand of a less-than comparison. Compare the parameter against 100.",
            setup: "construct-lam\nmove-parent\nrename n\nset-ann Num\nmove-child 0\nconstruct-var n\nconstruct-binop lt\n",
            target: "λn:Num. n < 100",
        },
        Task {
            name: "build_constant",
            family: Family::BuildFunction,
            goal: "Write the program that is just the number 42.",
            setup: "",
            target: "42",
        },
        Task {
            name: "build_sum",
            family: Family::BuildFunction,
            goal: "Write the program that adds 1 and 2, in that order.",
            setup: "",
            target: "1 + 2",
        },
        Task {
            name: "build_identity",
            family: Family::BuildFunction,
            goal: "Write the identity function on numbers: its parameter is named n and annotated Num, and its body is that parameter.",
            setup: "",
            target: "λn:Num. n",
        },
        Task {
            name: "build_double",
            family: Family::BuildFunction,
            goal: "Write a function whose parameter is named n and annotated Num, returning n multiplied by 2.",
            setup: "",
            target: "λn:Num. n * 2",
        },
        Task {
            name: "build_is_zero",
            family: Family::BuildFunction,
            goal: "Write a function whose parameter is named n and annotated Num, returning whether n equals 0.",
            setup: "",
            target: "λn:Num. n == 0",
        },
        Task {
            name: "build_clamp_to_one",
            family: Family::BuildFunction,
            goal: "Write a function whose parameter is named n and annotated Num. It returns 1 when n is less than 1, and otherwise returns n.",
            setup: "",
            target: "λn:Num. if n < 1 then 1 else n",
        },
        Task {
            name: "build_let_square",
            family: Family::BuildFunction,
            goal: "Write a let that binds the name x to 7 and whose body is x multiplied by x.",
            setup: "",
            target: "let x = 7 in x * x",
        },
        Task {
            name: "build_nested_conditional",
            family: Family::BuildFunction,
            goal: "Write a function whose parameter is named x and annotated Num. When 0 is less than x it looks further: when 10 is less than x it returns 2, otherwise 1. When 0 is not less than x it returns 0. Write every comparison with the literal on the left of <.",
            setup: "",
            target: "λx:Num. if 0 < x then (if 10 < x then 2 else 1) else 0",
        },
        Task {
            name: "build_pair_of_argument",
            family: Family::BuildFunction,
            goal: "Write a function whose parameter is named n and annotated Num, returning the pair whose two components are both n.",
            setup: "",
            target: "λn:Num. (n, n)",
        },
        Task {
            name: "build_apply_twice",
            family: Family::BuildFunction,
            goal: "Write a function whose parameter is named f and annotated Num -> Num, whose body applies f to 3.",
            setup: "",
            target: "λf:Num -> Num. f 3",
        },
        Task {
            name: "fix_bool_in_addition",
            family: Family::FixQuarantine,
            goal: "The right operand of the addition is quarantined: `true` is a Bool where a Num is needed. Replace the quarantined `true` with the number 2.",
            setup: "construct-num 1\nconstruct-binop add\nconstruct-bool true\n",
            target: "1 + 2",
        },
        Task {
            name: "fix_num_in_condition",
            family: Family::FixQuarantine,
            goal: "The condition of the if is quarantined: 1 is a Num where a Bool is needed. Replace it with the boolean true, then put 2 in the then-branch and 3 in the else-branch.",
            setup: "construct-if\nconstruct-num 1\n",
            target: "if true then 2 else 3",
        },
        Task {
            name: "fix_bool_operand_of_plus",
            family: Family::FixQuarantine,
            goal: "The left operand of the addition is quarantined: b is a Bool where a Num is needed. Put 6 into the empty right operand, then replace the quarantined b on the left with the number 5.",
            setup: "construct-lam\nmove-parent\nrename b\nset-ann Bool\nmove-child 0\nconstruct-var b\nconstruct-binop add\n",
            target: "λb:Bool. 5 + 6",
        },
        Task {
            name: "fix_bool_argument",
            family: Family::FixQuarantine,
            goal: "The argument of the application is quarantined: `true` is a Bool where f expects a Num. Replace it with the number 3.",
            setup: "construct-lam\nmove-parent\nrename f\nset-ann Num -> Num\nmove-child 0\nconstruct-var f\nconstruct-ap\nconstruct-bool true\n",
            target: "λf:Num -> Num. f 3",
        },
        Task {
            name: "fix_branch_mismatch",
            family: Family::FixQuarantine,
            goal: "The else-branch is quarantined: `false` is a Bool where the then-branch already fixed the type to Num. Replace it with the number 0.",
            setup: "construct-if\nconstruct-bool true\nmove-parent\nmove-child 1\nconstruct-num 1\nmove-next-sibling\nconstruct-bool false\n",
            target: "if true then 1 else 0",
        },
        Task {
            name: "fix_projection_of_a_number",
            family: Family::FixQuarantine,
            goal: "The operand of `fst` is quarantined: n is a Num where a pair is needed. Delete the quarantined operand and build the pair (n, 0) in its place.",
            setup: "construct-lam\nmove-parent\nrename n\nset-ann Num\nmove-child 0\nconstruct-var n\nconstruct-proj l\n",
            target: "λn:Num. fst (n, 0)",
        },
        Task {
            name: "extend_mul_into_sum",
            family: Family::ExtendProgram,
            goal: "The cursor is on the 2 in `1 + 2`. Extend it so that the 2 is multiplied by 3, keeping the 1 and the 2 where they are.",
            setup: "construct-num 1\nconstruct-binop add\nconstruct-num 2\n",
            target: "1 + 2 * 3",
        },
        Task {
            name: "extend_wrap_in_lambda",
            family: Family::ExtendProgram,
            goal: "The program is the number 5. Wrap it in a function whose parameter is named k and annotated Num, keeping the 5 as the body.",
            setup: "construct-num 5\n",
            target: "λk:Num. 5",
        },
        Task {
            name: "extend_bind_with_let",
            family: Family::ExtendProgram,
            goal: "The program is 7 * 7. Wrap it in a let that binds the name y to it, and make the body y plus 1.",
            setup: "construct-num 7\nconstruct-binop mul\nconstruct-num 7\nmove-parent\n",
            target: "let y = 7 * 7 in y + 1",
        },
        Task {
            name: "extend_apply_function",
            family: Family::ExtendProgram,
            goal: "The cursor is on the body `f` of the function. Extend it so that f is applied to 2.",
            setup: "construct-lam\nmove-parent\nrename f\nset-ann Num -> Num\nmove-child 0\nconstruct-var f\n",
            target: "λf:Num -> Num. f 2",
        },
        Task {
            name: "extend_pair_the_result",
            family: Family::ExtendProgram,
            goal: "The cursor is on the body `n` of the function. Extend it so the body becomes the pair (n, n).",
            setup: "construct-lam\nmove-parent\nrename n\nset-ann Num\nmove-child 0\nconstruct-var n\n",
            target: "λn:Num. (n, n)",
        },
        Task {
            name: "extend_flip_comparison",
            family: Family::ExtendProgram,
            goal: "The cursor is on the condition `n < 1`. Change it so the condition reads 1 < n instead, leaving the branches alone.",
            setup: "construct-lam\nmove-parent\nrename n\nset-ann Num\nmove-child 0\nconstruct-if\nconstruct-var n\nconstruct-binop lt\nconstruct-num 1\nmove-parent\nmove-parent\nmove-child 1\nconstruct-num 1\nmove-next-sibling\nconstruct-var n\nmove-parent\nmove-child 0\n",
            target: "λn:Num. if 1 < n then 1 else n",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::holes;
    use crate::session::AgentSession;
    use nothing_action::log::AuthorId;
    use nothing_core::typing::is_well_typed;

    fn start(task: &Task) -> AgentSession {
        let mut session = AgentSession::new(AuthorId::new(1));
        for line in task.setup.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            assert!(
                session.apply_text(line).unwrap_or_else(|e| panic!(
                    "{}: `{line}` did not parse: {e}",
                    task.name
                )),
                "{}: `{line}` did not apply",
                task.name
            );
        }
        session
    }

    #[test]
    fn there_are_at_least_thirty_tasks_with_distinct_names() {
        let all = tasks();
        assert!(all.len() >= 30, "only {} tasks", all.len());
        let mut names: Vec<&str> = all.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), all.len(), "task names must be distinct");
    }

    #[test]
    fn every_family_is_represented() {
        let all = tasks();
        for family in [
            Family::FillHole,
            Family::BuildFunction,
            Family::FixQuarantine,
            Family::ExtendProgram,
        ] {
            assert!(
                all.iter().filter(|t| t.family == family).count() >= 5,
                "{family:?} is under-represented"
            );
        }
    }

    #[test]
    fn every_setup_replays_to_a_well_typed_program() {
        for task in tasks() {
            let session = start(&task);
            assert!(
                is_well_typed(&session.exp()),
                "{}: setup produced an ill-typed program",
                task.name
            );
        }
    }

    #[test]
    fn every_quarantine_task_actually_starts_with_a_quarantine() {
        for task in tasks().into_iter().filter(|t| t.family == Family::FixQuarantine) {
            let session = start(&task);
            assert!(
                holes(&session.exp()).1 > 0,
                "{}: no non-empty hole in `{}`",
                task.name,
                session.state().render()
            );
        }
    }

    #[test]
    fn every_hole_filling_task_actually_starts_at_an_empty_hole() {
        for task in tasks().into_iter().filter(|t| t.family == Family::FillHole) {
            let session = start(&task);
            assert!(
                matches!(session.state().zipper.focus, nothing_core::exp::Exp::EmptyHole(_)),
                "{}: the cursor is not on an empty hole in `{}`",
                task.name,
                crate::holectx::hole_context(session.state()).focus_render
            );
        }
    }

    #[test]
    fn every_build_task_starts_from_the_empty_program() {
        for task in tasks().into_iter().filter(|t| t.family == Family::BuildFunction) {
            assert_eq!(task.setup, "", "{}", task.name);
        }
    }

    #[test]
    fn every_target_parses_and_is_well_typed_or_holed() {
        for task in tasks() {
            let parsed = crate::measure::text_parse::parse_program(task.target)
                .unwrap_or_else(|e| panic!("{}: target `{}` did not parse: {e}", task.name, task.target));
            assert!(
                is_well_typed(&parsed.exp),
                "{}: target `{}` is not well-typed",
                task.name,
                task.target
            );
        }
    }

    #[test]
    fn no_setup_already_reaches_its_target() {
        for task in tasks() {
            let session = start(&task);
            assert_ne!(
                session.state().render(),
                task.target,
                "{}: the setup already is the answer",
                task.name
            );
        }
    }
}
