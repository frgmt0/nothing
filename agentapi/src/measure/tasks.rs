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

pub fn post_b2_tasks() -> Vec<Task> {
    vec![
        Task {
            name: "fill_row_score_step",
            family: Family::FillHole,
            goal: "The hole is the body of the fold's step function, where r is the current row and acc is the running total. It should be acc plus the row's score field, with acc on the left of the +.",
            setup: "construct-let\nmove-parent\nrename mk\nmove-child 0\nconstruct-lam\nmove-parent\nrename n\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename s\nset-ann Num\nmove-child 0\nconstruct-record\nrename-field name\nconstruct-var n\nadd-field\nrename-field score\nconstruct-var s\nmove-parent\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-let\nmove-parent\nrename a\nmove-child 0\nconstruct-var mk\nconstruct-ap\nconstruct-str \"ada\"\nmove-parent\nconstruct-ap\nconstruct-num 90\nmove-parent\nmove-parent\nmove-child 1\nconstruct-let\nmove-parent\nrename b\nmove-child 0\nconstruct-var mk\nconstruct-ap\nconstruct-str \"bob\"\nmove-parent\nconstruct-ap\nconstruct-num 70\nmove-parent\nmove-parent\nmove-child 1\nconstruct-let\nmove-parent\nrename rows\nmove-child 0\nconstruct-var a\nconstruct-cons\nconstruct-cons\nconstruct-var b\nmove-parent\nmove-child 1\nconstruct-nil\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-fold\nconstruct-var rows\nmove-next-sibling\nconstruct-num 0\nmove-next-sibling\nconstruct-lam\nmove-parent\nrename r\nmove-child 0\nconstruct-lam\nmove-parent\nrename acc\nset-ann Num\nmove-child 0\n",
            target: "let mk = (λn:Str. λs:Num. {name = n, score = s}) in let a = mk \"ada\" 90 in let b = mk \"bob\" 70 in let rows = a :: b :: nil in fold rows 0 (λr:?. λacc:Num. acc + r.score)",
        },
        Task {
            name: "extend_row_score_double",
            family: Family::ExtendProgram,
            goal: "The cursor is on `r.score`, the right operand of the addition in the fold's step function. Extend it so that score is multiplied by 2 before it is added, leaving `acc +` and the rest of the program alone.",
            setup: "construct-let\nmove-parent\nrename mk\nmove-child 0\nconstruct-lam\nmove-parent\nrename n\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename s\nset-ann Num\nmove-child 0\nconstruct-record\nrename-field name\nconstruct-var n\nadd-field\nrename-field score\nconstruct-var s\nmove-parent\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-let\nmove-parent\nrename a\nmove-child 0\nconstruct-var mk\nconstruct-ap\nconstruct-str \"ada\"\nmove-parent\nconstruct-ap\nconstruct-num 90\nmove-parent\nmove-parent\nmove-child 1\nconstruct-let\nmove-parent\nrename b\nmove-child 0\nconstruct-var mk\nconstruct-ap\nconstruct-str \"bob\"\nmove-parent\nconstruct-ap\nconstruct-num 70\nmove-parent\nmove-parent\nmove-child 1\nconstruct-let\nmove-parent\nrename rows\nmove-child 0\nconstruct-var a\nconstruct-cons\nconstruct-cons\nconstruct-var b\nmove-parent\nmove-child 1\nconstruct-nil\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-fold\nconstruct-var rows\nmove-next-sibling\nconstruct-num 0\nmove-next-sibling\nconstruct-lam\nmove-parent\nrename r\nmove-child 0\nconstruct-lam\nmove-parent\nrename acc\nset-ann Num\nmove-child 0\nconstruct-var acc\nconstruct-binop add\nconstruct-field score\nconstruct-var r\nmove-parent\n",
            target: "let mk = (λn:Str. λs:Num. {name = n, score = s}) in let a = mk \"ada\" 90 in let b = mk \"bob\" 70 in let rows = a :: b :: nil in fold rows 0 (λr:?. λacc:Num. acc + r.score * 2)",
        },
        Task {
            name: "fill_row_join_seed",
            family: Family::FillHole,
            goal: "The hole is the fold's starting accumulator, between the list rows and the step function. The step function builds a Str, so the starting accumulator should be the empty string.",
            setup: "construct-let\nmove-parent\nrename mk\nmove-child 0\nconstruct-lam\nmove-parent\nrename n\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename s\nset-ann Num\nmove-child 0\nconstruct-record\nrename-field name\nconstruct-var n\nadd-field\nrename-field score\nconstruct-var s\nmove-parent\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-let\nmove-parent\nrename a\nmove-child 0\nconstruct-var mk\nconstruct-ap\nconstruct-str \"ada\"\nmove-parent\nconstruct-ap\nconstruct-num 90\nmove-parent\nmove-parent\nmove-child 1\nconstruct-let\nmove-parent\nrename b\nmove-child 0\nconstruct-var mk\nconstruct-ap\nconstruct-str \"bob\"\nmove-parent\nconstruct-ap\nconstruct-num 70\nmove-parent\nmove-parent\nmove-child 1\nconstruct-let\nmove-parent\nrename rows\nmove-child 0\nconstruct-var a\nconstruct-cons\nconstruct-cons\nconstruct-var b\nmove-parent\nmove-child 1\nconstruct-nil\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-fold\nconstruct-var rows\nmove-next-sibling\nmove-next-sibling\nconstruct-lam\nmove-parent\nrename r\nmove-child 0\nconstruct-lam\nmove-parent\nrename acc\nset-ann Str\nmove-child 0\nconstruct-var acc\nconstruct-binop concat\nconstruct-field name\nconstruct-var r\nmove-parent\nmove-parent\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\n",
            target: "let mk = (λn:Str. λs:Num. {name = n, score = s}) in let a = mk \"ada\" 90 in let b = mk \"bob\" 70 in let rows = a :: b :: nil in fold rows \"\" (λr:?. λacc:Str. acc ++ r.name)",
        },
        Task {
            name: "extend_row_name_comma",
            family: Family::ExtendProgram,
            goal: "The cursor is on `acc ++ r.name`, the body of the fold's step function. Extend it so a comma follows the name: the body becomes acc, then the row's name field, then the one-character string \",\", concatenated left to right.",
            setup: "construct-let\nmove-parent\nrename mk\nmove-child 0\nconstruct-lam\nmove-parent\nrename n\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename s\nset-ann Num\nmove-child 0\nconstruct-record\nrename-field name\nconstruct-var n\nadd-field\nrename-field score\nconstruct-var s\nmove-parent\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-let\nmove-parent\nrename a\nmove-child 0\nconstruct-var mk\nconstruct-ap\nconstruct-str \"ada\"\nmove-parent\nconstruct-ap\nconstruct-num 90\nmove-parent\nmove-parent\nmove-child 1\nconstruct-let\nmove-parent\nrename b\nmove-child 0\nconstruct-var mk\nconstruct-ap\nconstruct-str \"bob\"\nmove-parent\nconstruct-ap\nconstruct-num 70\nmove-parent\nmove-parent\nmove-child 1\nconstruct-let\nmove-parent\nrename rows\nmove-child 0\nconstruct-var a\nconstruct-cons\nconstruct-cons\nconstruct-var b\nmove-parent\nmove-child 1\nconstruct-nil\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-fold\nconstruct-var rows\nmove-next-sibling\nconstruct-str \"\"\nmove-next-sibling\nconstruct-lam\nmove-parent\nrename r\nmove-child 0\nconstruct-lam\nmove-parent\nrename acc\nset-ann Str\nmove-child 0\nconstruct-var acc\nconstruct-binop concat\nconstruct-field name\nconstruct-var r\nmove-parent\nmove-parent\n",
            target: "let mk = (λn:Str. λs:Num. {name = n, score = s}) in let a = mk \"ada\" 90 in let b = mk \"bob\" 70 in let rows = a :: b :: nil in fold rows \"\" (λr:?. λacc:Str. acc ++ r.name ++ \",\")",
        },
        Task {
            name: "extend_best_projection",
            family: Family::ExtendProgram,
            goal: "The cursor is on the final `best`, the body of the second let. Extend it so the program returns the score field of best rather than best itself.",
            setup: "construct-let\nmove-parent\nrename mk\nmove-child 0\nconstruct-lam\nmove-parent\nrename n\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename s\nset-ann Num\nmove-child 0\nconstruct-record\nrename-field name\nconstruct-var n\nadd-field\nrename-field score\nconstruct-var s\nmove-parent\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-let\nmove-parent\nrename best\nmove-child 0\nconstruct-var mk\nconstruct-ap\nconstruct-str \"ada\"\nmove-parent\nconstruct-ap\nconstruct-num 90\nmove-parent\nmove-parent\nmove-child 1\nconstruct-var best\n",
            target: "let mk = (λn:Str. λs:Num. {name = n, score = s}) in let best = mk \"ada\" 90 in best.score",
        },
        Task {
            name: "fill_greeting_formal_branch",
            family: Family::FillHole,
            goal: "The hole is the then-branch of the conditional. It should be the string \"Dear \", then the parameter who, then the one-character string \",\", concatenated left to right.",
            setup: "construct-lam\nmove-parent\nrename who\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename formal\nset-ann Bool\nmove-child 0\nconstruct-if\nconstruct-var formal\nmove-parent\nmove-child 2\nconstruct-str \"hey \"\nconstruct-binop concat\nconstruct-var who\nmove-parent\nconstruct-binop concat\nconstruct-str \"!\"\nmove-parent\nmove-parent\nmove-child 1\n",
            target: "λwho:Str. λformal:Bool. if formal then \"Dear \" ++ who ++ \",\" else \"hey \" ++ who ++ \"!\"",
        },
        Task {
            name: "fix_number_in_greeting",
            family: Family::FixQuarantine,
            goal: "The middle operand of the else-branch's concatenation is quarantined: 7 is a Num where a Str is needed. Replace the quarantined 7 with the parameter who, so the else-branch reads \"hello, \" ++ who ++ \".\".",
            setup: "construct-lam\nmove-parent\nrename who\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename loud\nset-ann Bool\nmove-child 0\nconstruct-if\nconstruct-var loud\nmove-parent\nmove-child 1\nconstruct-str \"HELLO, \"\nconstruct-binop concat\nconstruct-var who\nmove-parent\nconstruct-binop concat\nconstruct-str \"!\"\nmove-parent\nmove-parent\nmove-child 2\nconstruct-str \"hello, \"\nconstruct-binop concat\nconstruct-var who\nmove-parent\nconstruct-binop concat\nconstruct-str \".\"\nmove-parent\nmove-child 0\nmove-child 1\nconstruct-num 7\n",
            target: "λwho:Str. λloud:Bool. if loud then \"HELLO, \" ++ who ++ \"!\" else \"hello, \" ++ who ++ \".\"",
        },
        Task {
            name: "extend_name_with_last",
            family: Family::ExtendProgram,
            goal: "The cursor is on `\"name: \" ++ first`, the else-branch of the conditional. Extend it so the else-branch ends the way the then-branch does: after first, append the one-space string \" \" and then the parameter last, concatenated left to right.",
            setup: "construct-lam\nmove-parent\nrename first\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename last\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename shout\nset-ann Bool\nmove-child 0\nconstruct-if\nconstruct-var shout\nmove-parent\nmove-child 1\nconstruct-str \"NAME: \"\nconstruct-binop concat\nconstruct-var first\nmove-parent\nconstruct-binop concat\nconstruct-str \" \"\nmove-parent\nconstruct-binop concat\nconstruct-var last\nmove-parent\nmove-parent\nmove-child 2\nconstruct-str \"name: \"\nconstruct-binop concat\nconstruct-var first\nmove-parent\n",
            target: "λfirst:Str. λlast:Str. λshout:Bool. if shout then \"NAME: \" ++ first ++ \" \" ++ last else \"name: \" ++ first ++ \" \" ++ last",
        },
        Task {
            name: "fill_command_second_test",
            family: Family::FillHole,
            goal: "The hole is the condition of the inner if, the one sitting in the else-branch of the outer if. It should test whether cmd equals the string \"del\", with cmd on the left of the ==.",
            setup: "construct-lam\nmove-parent\nrename cmd\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename arg\nset-ann Str\nmove-child 0\nconstruct-if\nconstruct-binop eq\nconstruct-var cmd\nmove-parent\nmove-child 1\nconstruct-str \"add\"\nmove-parent\nmove-parent\nmove-child 1\nconstruct-var arg\nconstruct-binop concat\nconstruct-str \" added\"\nmove-parent\nmove-parent\nmove-child 2\nconstruct-if\nmove-parent\nmove-child 1\nconstruct-var arg\nconstruct-binop concat\nconstruct-str \" deleted\"\nmove-parent\nmove-parent\nmove-child 2\nconstruct-str \"unknown \"\nconstruct-binop concat\nconstruct-var cmd\nmove-parent\nmove-parent\nmove-child 0\n",
            target: "λcmd:Str. λarg:Str. if cmd == \"add\" then arg ++ \" added\" else if cmd == \"del\" then arg ++ \" deleted\" else \"unknown \" ++ cmd",
        },
        Task {
            name: "fill_settings_retries",
            family: Family::FillHole,
            goal: "The hole is the value of the record's retries field. It should be the number 3.",
            setup: "construct-let\nmove-parent\nrename settings\nmove-child 0\nconstruct-record\nrename-field host\nconstruct-str \"localhost\"\nadd-field\nrename-field port\nconstruct-num 8080\nadd-field\nrename-field retries\nmove-parent\nmove-parent\nmove-child 1\nconstruct-if\nconstruct-binop lt\nconstruct-num 0\nmove-parent\nmove-child 1\nconstruct-field retries\nconstruct-var settings\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-field host\nconstruct-var settings\nmove-parent\nmove-parent\nmove-child 2\nconstruct-str \"no host\"\nmove-parent\nmove-parent\nmove-child 0\nmove-child 2\n",
            target: "let settings = {host = \"localhost\", port = 8080, retries = 3} in if 0 < settings.retries then settings.host else \"no host\"",
        },
        Task {
            name: "fix_offline_branch",
            family: Family::FixQuarantine,
            goal: "The else-branch is quarantined: 0 is a Num where the then-branch already fixed the type to Str. Replace the quarantined 0 with the string \"offline\".",
            setup: "construct-let\nmove-parent\nrename settings\nmove-child 0\nconstruct-record\nrename-field host\nconstruct-str \"localhost\"\nadd-field\nrename-field port\nconstruct-num 8080\nadd-field\nrename-field verbose\nconstruct-bool false\nmove-parent\nmove-parent\nmove-child 1\nconstruct-if\nconstruct-field verbose\nconstruct-var settings\nmove-parent\nmove-parent\nmove-child 1\nconstruct-field host\nconstruct-var settings\nmove-parent\nmove-parent\nmove-child 2\nconstruct-num 0\n",
            target: "let settings = {host = \"localhost\", port = 8080, verbose = false} in if settings.verbose then settings.host else \"offline\"",
        },
        Task {
            name: "extend_settings_with_retries",
            family: Family::ExtendProgram,
            goal: "The cursor is on 8080, the value of the record's port field. Extend the record with one more field after port, named retries, whose value is the number 3. Leave the two existing fields and the let body as they are.",
            setup: "construct-let\nmove-parent\nrename settings\nmove-child 0\nconstruct-record\nrename-field host\nconstruct-str \"localhost\"\nadd-field\nrename-field port\nconstruct-num 8080\nmove-parent\nmove-parent\nmove-child 1\nconstruct-field host\nconstruct-var settings\nmove-parent\nmove-parent\nmove-child 0\nmove-child 1\n",
            target: "let settings = {host = \"localhost\", port = 8080, retries = 3} in settings.host",
        },
        Task {
            name: "fix_score_else_branch",
            family: Family::FixQuarantine,
            goal: "The else-branch is quarantined: `false` is a Bool where the then-branch already fixed the type to Str. Delete it and build in its place the row's name field followed by the string \" failed\", concatenated in that order.",
            setup: "construct-let\nmove-parent\nrename row\nmove-child 0\nconstruct-record\nrename-field name\nconstruct-str \"ada\"\nadd-field\nrename-field score\nconstruct-num 90\nmove-parent\nmove-parent\nmove-child 1\nconstruct-if\nconstruct-binop lt\nconstruct-num 0\nmove-parent\nmove-child 1\nconstruct-field score\nconstruct-var row\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-field name\nconstruct-var row\nmove-parent\nconstruct-binop concat\nconstruct-str \" passed\"\nmove-parent\nmove-parent\nmove-child 2\nconstruct-bool false\n",
            target: "let row = {name = \"ada\", score = 90} in if 0 < row.score then row.name ++ \" passed\" else row.name ++ \" failed\"",
        },
        Task {
            name: "fill_match_stopped_arm",
            family: Family::FillHole,
            goal: "The hole is the body of the match's third arm, the one for Stopped. It should be the string \"done\".",
            setup: "construct-lam\nmove-parent\nrename s\nmove-child 0\nconstruct-var s\nconstruct-match\nadd-arm\nrename-constructor Idle\nadd-arm\nrename-constructor Running\nadd-arm\nrename-constructor Stopped\nmove-parent\nmove-child 1\nconstruct-str \"waiting\"\nmove-next-sibling\nconstruct-str \"in flight\"\nmove-next-sibling\n",
            target: "λs:?. match s { Idle x0 -> \"waiting\" | Running x1 -> \"in flight\" | Stopped x2 -> \"done\" }",
        },
        Task {
            name: "fix_match_busy_arm",
            family: Family::FixQuarantine,
            goal: "The body of the Busy arm is quarantined: 3 is a Num where the other two arms already fixed the type to Str. Replace the quarantined 3 with the string \"busy\".",
            setup: "construct-lam\nmove-parent\nrename s\nmove-child 0\nconstruct-var s\nconstruct-match\nadd-arm\nrename-constructor Idle\nadd-arm\nrename-constructor Busy\nadd-arm\nrename-constructor Done\nmove-parent\nmove-child 1\nconstruct-str \"idle\"\nmove-next-sibling\nmove-next-sibling\nconstruct-str \"done\"\nmove-prev-sibling\nconstruct-num 3\n",
            target: "λs:?. match s { Idle x0 -> \"idle\" | Busy x1 -> \"busy\" | Done x2 -> \"done\" }",
        },
        Task {
            name: "extend_match_with_stopped_arm",
            family: Family::ExtendProgram,
            goal: "The cursor is on the match, which has two arms. Add one more arm after them, its constructor named Stopped and its body the string \"stopped\". Leave the new arm's payload binder at the name the editor gives it, and leave the first two arms alone.",
            setup: "construct-lam\nmove-parent\nrename s\nmove-child 0\nconstruct-var s\nconstruct-match\nadd-arm\nrename-constructor Idle\nadd-arm\nrename-constructor Running\nmove-parent\nmove-child 1\nconstruct-str \"idle\"\nmove-next-sibling\nconstruct-str \"running\"\nmove-parent\n",
            target: "λs:?. match s { Idle x0 -> \"idle\" | Running x1 -> \"running\" | Stopped x2 -> \"stopped\" }",
        },
        Task {
            name: "fill_filter_threshold",
            family: Family::FillHole,
            goal: "The hole is the condition of the if inside the fold's step function. It should test whether 10 is less than h, with the literal 10 on the left of the <.",
            setup: "construct-lam\nmove-parent\nrename xs\nset-ann List Num\nmove-child 0\nconstruct-fold\nconstruct-var xs\nmove-next-sibling\nconstruct-nil\nmove-next-sibling\nconstruct-lam\nmove-parent\nrename h\nset-ann Num\nmove-child 0\nconstruct-lam\nmove-parent\nrename acc\nset-ann List Num\nmove-child 0\nconstruct-if\nmove-parent\nmove-child 1\nconstruct-cons\nconstruct-var h\nmove-parent\nmove-child 1\nconstruct-var acc\nmove-parent\nmove-parent\nmove-child 2\nconstruct-var acc\nmove-parent\nmove-child 0\n",
            target: "λxs:List Num. fold xs nil (λh:Num. λacc:List Num. if 10 < h then h :: acc else acc)",
        },
        Task {
            name: "fix_filter_else_branch",
            family: Family::FixQuarantine,
            goal: "The else-branch of the fold's step function is quarantined: 0 is a Num where a List Num is needed. Replace the quarantined 0 with the accumulator acc, so an element that fails the test leaves the list unchanged.",
            setup: "construct-lam\nmove-parent\nrename xs\nset-ann List Num\nmove-child 0\nconstruct-fold\nconstruct-var xs\nmove-next-sibling\nconstruct-nil\nmove-next-sibling\nconstruct-lam\nmove-parent\nrename h\nset-ann Num\nmove-child 0\nconstruct-lam\nmove-parent\nrename acc\nset-ann List Num\nmove-child 0\nconstruct-if\nconstruct-binop lt\nconstruct-num 5\nmove-parent\nmove-child 1\nconstruct-var h\nmove-parent\nmove-parent\nmove-child 1\nconstruct-cons\nconstruct-var h\nmove-parent\nmove-child 1\nconstruct-var acc\nmove-parent\nmove-parent\nmove-child 2\nconstruct-num 0\n",
            target: "λxs:List Num. fold xs nil (λh:Num. λacc:List Num. if 5 < h then h :: acc else acc)",
        },
        Task {
            name: "fill_join_separator",
            family: Family::FillHole,
            goal: "The hole is the right operand of the last concatenation in the fold's step function. It should be the one-character string holding a single space.",
            setup: "construct-let\nmove-parent\nrename names\nmove-child 0\nconstruct-str \"ada\"\nconstruct-cons\nconstruct-cons\nconstruct-str \"bob\"\nmove-parent\nmove-child 1\nconstruct-cons\nconstruct-str \"cy\"\nmove-parent\nmove-child 1\nconstruct-nil\nmove-parent\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-fold\nconstruct-var names\nmove-next-sibling\nconstruct-str \"\"\nmove-next-sibling\nconstruct-lam\nmove-parent\nrename n\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename acc\nset-ann Str\nmove-child 0\nconstruct-var acc\nconstruct-binop concat\nconstruct-var n\nmove-parent\nconstruct-binop concat\n",
            target: "let names = \"ada\" :: \"bob\" :: \"cy\" :: nil in fold names \"\" (λn:Str. λacc:Str. acc ++ n ++ \" \")",
        },
        Task {
            name: "fix_number_in_name_list",
            family: Family::FixQuarantine,
            goal: "The second element of the list is quarantined: 7 is a Num where the other elements are Str. Replace the quarantined 7 with the string \"bob\".",
            setup: "construct-let\nmove-parent\nrename names\nmove-child 0\nconstruct-str \"ada\"\nconstruct-cons\nconstruct-cons\nconstruct-num 7\nmove-parent\nmove-child 1\nconstruct-cons\nconstruct-str \"cy\"\nmove-parent\nmove-child 1\nconstruct-nil\nmove-parent\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-fold\nconstruct-var names\nmove-next-sibling\nconstruct-str \"\"\nmove-next-sibling\nconstruct-lam\nmove-parent\nrename n\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename acc\nset-ann Str\nmove-child 0\nconstruct-var acc\nconstruct-binop concat\nconstruct-var n\nmove-parent\nmove-parent\nmove-parent\nmove-parent\nmove-parent\nmove-child 0\nmove-child 1\nmove-child 0\n",
            target: "let names = \"ada\" :: \"bob\" :: \"cy\" :: nil in fold names \"\" (λn:Str. λacc:Str. acc ++ n)",
        },
        Task {
            name: "fix_list_tail",
            family: Family::FixQuarantine,
            goal: "The tail of the list after \"bob\" is quarantined: 3 is a Num where a list is needed. Replace the quarantined 3 with the empty list, so the list holds exactly \"ada\" and \"bob\".",
            setup: "construct-let\nmove-parent\nrename names\nmove-child 0\nconstruct-str \"ada\"\nconstruct-cons\nconstruct-cons\nconstruct-str \"bob\"\nmove-parent\nmove-child 1\nconstruct-num 3\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-fold\nconstruct-var names\nmove-next-sibling\nconstruct-str \"\"\nmove-next-sibling\nconstruct-lam\nmove-parent\nrename n\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename acc\nset-ann Str\nmove-child 0\nconstruct-var acc\nconstruct-binop concat\nconstruct-var n\nmove-parent\nmove-parent\nmove-parent\nmove-parent\nmove-parent\nmove-child 0\nmove-child 1\nmove-child 1\n",
            target: "let names = \"ada\" :: \"bob\" :: nil in fold names \"\" (λn:Str. λacc:Str. acc ++ n)",
        },
        Task {
            name: "fix_string_seed_in_total",
            family: Family::FixQuarantine,
            goal: "The fold's starting accumulator is quarantined: \"none\" is a Str where the step function needs a Num. Replace the quarantined string with the number 0.",
            setup: "construct-lam\nmove-parent\nrename xs\nset-ann List Num\nmove-child 0\nconstruct-fold\nconstruct-var xs\nmove-next-sibling\nmove-next-sibling\nconstruct-lam\nmove-parent\nrename h\nset-ann Num\nmove-child 0\nconstruct-lam\nmove-parent\nrename acc\nset-ann Num\nmove-child 0\nconstruct-var acc\nconstruct-binop add\nconstruct-var h\nmove-parent\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-str \"none\"\n",
            target: "λxs:List Num. fold xs 0 (λh:Num. λacc:Num. acc + h)",
        },
        Task {
            name: "extend_join_into_let",
            family: Family::ExtendProgram,
            goal: "The cursor is on the fold, the body of the let that binds names. Wrap that fold in a let that binds the name joined to it, and make the new let's body joined followed by the string \"!\", concatenated in that order.",
            setup: "construct-let\nmove-parent\nrename names\nmove-child 0\nconstruct-str \"ada\"\nconstruct-cons\nconstruct-cons\nconstruct-str \"bob\"\nmove-parent\nmove-child 1\nconstruct-nil\nmove-parent\nmove-parent\nmove-parent\nmove-child 1\nconstruct-fold\nconstruct-var names\nmove-next-sibling\nconstruct-str \"\"\nmove-next-sibling\nconstruct-lam\nmove-parent\nrename n\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename acc\nset-ann Str\nmove-child 0\nconstruct-var acc\nconstruct-binop concat\nconstruct-var n\nmove-parent\nmove-parent\nmove-parent\nmove-parent\n",
            target: "let names = \"ada\" :: \"bob\" :: nil in let joined = fold names \"\" (λn:Str. λacc:Str. acc ++ n) in joined ++ \"!\"",
        },
        Task {
            name: "extend_item_count",
            family: Family::ExtendProgram,
            goal: "The cursor is on n, the value of the record's count field. Extend it so the field's value is n plus 1, with n on the left of the +. Leave the label and ok fields as they are.",
            setup: "construct-lam\nmove-parent\nrename tag\nset-ann Str\nmove-child 0\nconstruct-lam\nmove-parent\nrename n\nset-ann Num\nmove-child 0\nconstruct-record\nrename-field label\nconstruct-str \"item: \"\nconstruct-binop concat\nconstruct-var tag\nmove-parent\nmove-parent\nadd-field\nrename-field count\nconstruct-var n\nadd-field\nrename-field ok\nconstruct-binop lt\nconstruct-num 0\nmove-parent\nmove-child 1\nconstruct-var n\nmove-parent\nmove-parent\nmove-child 1\n",
            target: "λtag:Str. λn:Num. {label = \"item: \" ++ tag, count = n + 1, ok = 0 < n}",
        },
        Task {
            name: "build_exclaim",
            family: Family::BuildFunction,
            goal: "Write a function whose parameter is named s and annotated Str. Its body is the string \"hello, \", then s, then the one-character string \"!\", concatenated left to right.",
            setup: "",
            target: "λs:Str. \"hello, \" ++ s ++ \"!\"",
        },
        Task {
            name: "build_total_of_list",
            family: Family::BuildFunction,
            goal: "Write a function whose parameter is named ns and annotated List Num. Its body folds over ns starting from 0, with a step function whose first parameter is named h and annotated Num and whose second is named acc and annotated Num, returning acc plus h in that order.",
            setup: "",
            target: "λns:List Num. fold ns 0 (λh:Num. λacc:Num. acc + h)",
        },
        Task {
            name: "build_point_record",
            family: Family::BuildFunction,
            goal: "Write a function of two parameters, the first named a and the second named b, both annotated Num. Its body is a record with two fields in this order: x, whose value is a, then y, whose value is b.",
            setup: "",
            target: "λa:Num. λb:Num. {x = a, y = b}",
        },
        Task {
            name: "build_name_list",
            family: Family::BuildFunction,
            goal: "Write a function whose parameter is named first and annotated Str. Its body is a list of three elements in this order: first, the string \"bob\", the string \"cy\", ending in the empty list.",
            setup: "",
            target: "λfirst:Str. first :: \"bob\" :: \"cy\" :: nil",
        },
        Task {
            name: "build_labelled_count",
            family: Family::BuildFunction,
            goal: "Write a function of two parameters, the first named tag and annotated Str, the second named n and annotated Num. Its body is a record with two fields in this order: label, whose value is tag followed by the one-character string \"!\", then count, whose value is n plus 1.",
            setup: "",
            target: "λtag:Str. λn:Num. {label = tag ++ \"!\", count = n + 1}",
        },
        Task {
            name: "build_choose_greeting",
            family: Family::BuildFunction,
            goal: "Write a function whose parameter is named formal and annotated Bool. When formal is true it returns the string \"good evening\", and otherwise the string \"hi\".",
            setup: "",
            target: "λformal:Bool. if formal then \"good evening\" else \"hi\"",
        },
        Task {
            name: "build_city_lookup",
            family: Family::BuildFunction,
            goal: "Write a let that binds the name row to a record with two fields in this order: city, whose value is the string \"oslo\", then temp, whose value is 4. The body of the let projects the city field out of row.",
            setup: "",
            target: "let row = {city = \"oslo\", temp = 4} in row.city",
        },
        Task {
            name: "build_status_word",
            family: Family::BuildFunction,
            goal: "Write a function whose parameter is named s and left without a type annotation. Its body matches on s with two arms in this order: one whose constructor is named Ok returning the string \"fine\", then one whose constructor is named Bad returning the string \"broken\". Leave both payload binders at the names the editor gives them.",
            setup: "",
            target: "λs:?. match s { Ok x0 -> \"fine\" | Bad x1 -> \"broken\" }",
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
                session
                    .apply_text(line)
                    .unwrap_or_else(|e| panic!("{}: `{line}` did not parse: {e}", task.name)),
                "{}: `{line}` did not apply",
                task.name
            );
        }
        session
    }

    fn task_sets() -> [(&'static str, Vec<Task>); 2] {
        [("tasks", tasks()), ("post_b2_tasks", post_b2_tasks())]
    }

    fn least_per_family(set: &str) -> usize {
        if set == "post_b2_tasks" { 6 } else { 5 }
    }

    fn mean_target_length(all: &[Task]) -> usize {
        all.iter().map(|t| t.target.chars().count()).sum::<usize>() / all.len()
    }

    #[test]
    fn there_are_at_least_thirty_tasks_with_distinct_names() {
        for (set, all) in task_sets() {
            assert!(all.len() >= 30, "{set}: only {} tasks", all.len());
            let mut names: Vec<&str> = all.iter().map(|t| t.name).collect();
            names.sort();
            names.dedup();
            assert_eq!(names.len(), all.len(), "{set}: task names must be distinct");
        }
    }

    #[test]
    fn the_two_sets_never_share_a_name() {
        let later = post_b2_tasks();
        for original in tasks() {
            assert!(
                later.iter().all(|t| t.name != original.name),
                "`{}` names a task in both sets",
                original.name
            );
        }
    }

    #[test]
    fn every_family_is_represented() {
        for (set, all) in task_sets() {
            let least = least_per_family(set);
            for family in [
                Family::FillHole,
                Family::BuildFunction,
                Family::FixQuarantine,
                Family::ExtendProgram,
            ] {
                assert!(
                    all.iter().filter(|t| t.family == family).count() >= least,
                    "{set}: {family:?} is under-represented"
                );
            }
        }
    }

    #[test]
    fn every_setup_replays_to_a_well_typed_program() {
        for (set, all) in task_sets() {
            for task in all {
                let session = start(&task);
                assert!(
                    is_well_typed(&session.exp()),
                    "{set}/{}: setup produced an ill-typed program",
                    task.name
                );
            }
        }
    }

    #[test]
    fn every_quarantine_task_actually_starts_with_a_quarantine() {
        for (set, all) in task_sets() {
            for task in all
                .into_iter()
                .filter(|t| t.family == Family::FixQuarantine)
            {
                let session = start(&task);
                assert!(
                    holes(&session.exp()).1 > 0,
                    "{set}/{}: no non-empty hole in `{}`",
                    task.name,
                    session.state().render()
                );
            }
        }
    }

    #[test]
    fn every_hole_filling_task_actually_starts_at_an_empty_hole() {
        for (set, all) in task_sets() {
            for task in all.into_iter().filter(|t| t.family == Family::FillHole) {
                let session = start(&task);
                assert!(
                    matches!(
                        session.state().zipper.focus,
                        nothing_core::exp::Exp::EmptyHole(_)
                    ),
                    "{set}/{}: the cursor is not on an empty hole in `{}`",
                    task.name,
                    crate::holectx::hole_context(session.state()).focus_render
                );
            }
        }
    }

    #[test]
    fn every_build_task_starts_from_the_empty_program() {
        for (set, all) in task_sets() {
            for task in all
                .into_iter()
                .filter(|t| t.family == Family::BuildFunction)
            {
                assert_eq!(task.setup, "", "{set}/{}", task.name);
            }
        }
    }

    #[test]
    fn every_target_parses_and_is_well_typed_or_holed() {
        for (set, all) in task_sets() {
            for task in all {
                let parsed =
                    crate::measure::text_parse::parse_program(task.target).unwrap_or_else(|e| {
                        panic!(
                            "{set}/{}: target `{}` did not parse: {e}",
                            task.name, task.target
                        )
                    });
                assert!(
                    is_well_typed(&parsed.exp),
                    "{set}/{}: target `{}` is not well-typed",
                    task.name,
                    task.target
                );
            }
        }
    }

    #[test]
    fn no_setup_already_reaches_its_target() {
        for (set, all) in task_sets() {
            for task in all {
                let session = start(&task);
                assert_ne!(
                    session.state().render(),
                    task.target,
                    "{set}/{}: the setup already is the answer",
                    task.name
                );
            }
        }
    }

    #[test]
    fn the_post_b2_targets_exercise_the_post_b2_grammar() {
        let all = post_b2_tasks();
        let with_strings = all.iter().filter(|t| t.target.contains('"')).count();
        let with_lists = all
            .iter()
            .filter(|t| {
                t.target.contains("::") || t.target.contains("nil") || t.target.contains("fold")
            })
            .count();
        let with_records = all
            .iter()
            .filter(|t| t.target.contains('{') && !t.target.contains("match"))
            .count();
        let with_matches = all.iter().filter(|t| t.target.contains("match")).count();
        assert!(
            with_strings >= 20,
            "only {with_strings} post-B2 targets hold a string literal"
        );
        assert!(
            with_lists >= 12,
            "only {with_lists} post-B2 targets hold a list"
        );
        assert!(
            with_records >= 10,
            "only {with_records} post-B2 targets hold a record"
        );
        assert!(
            with_matches >= 4,
            "only {with_matches} post-B2 targets hold a match"
        );
    }

    #[test]
    fn post_b2_targets_are_much_larger_than_the_originals() {
        let original = mean_target_length(&tasks());
        let later = mean_target_length(&post_b2_tasks());
        assert!(
            later >= original * 3,
            "post-B2 targets average {later} characters against {original} for the original set"
        );
    }
}
