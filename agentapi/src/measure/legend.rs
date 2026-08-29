pub struct SyntaxEntry {
    pub label: &'static str,
    pub example: &'static str,
    pub note: &'static str,
}

pub struct ActionEntry {
    pub example: &'static str,
    pub effect: &'static str,
}

pub const ORIGINAL_SYNTAX: &str = "\
The program is written in this syntax:
  numbers        0   1   -3
  booleans       true   false
  variable       a name bound by an enclosing λ or let
  function       λx:T. body        T is a type: Num, Bool, ?, T -> T, T * T
  application    f a               left associative, each argument is an atom
  operators      + - * < ==        * binds tightest, then + and -, then < and ==
  conditional    if c then a else b
  binding        let x = e in body
  pair           (a, b)
  projection     fst e     snd e
  empty hole     ⦇⦈
  grouping       ( e )
Every name you use must be bound by an enclosing λ or let. There are no other
built-in names, no strings, no lists and no recursion.";

pub const POST_B2_SYNTAX_ENTRIES: &[SyntaxEntry] = &[
    SyntaxEntry {
        label: "numbers",
        example: "1 + 2 * 3",
        note: "* binds tightest, then + - ++, then :: , then < ==",
    },
    SyntaxEntry {
        label: "booleans",
        example: "if true then 1 else 0",
        note: "",
    },
    SyntaxEntry {
        label: "strings",
        example: "\"hello, \" ++ \"world\"",
        note: "++ joins two strings; only \\\" and \\\\ are escaped",
    },
    SyntaxEntry {
        label: "comparison",
        example: "λn:Num. n * 2 + 1 < 10",
        note: "< and == are the only comparisons; there is no >",
    },
    SyntaxEntry {
        label: "function",
        example: "λn:Num. n + 1",
        note: "λ, or \\ if you prefer; the parameter is always annotated",
    },
    SyntaxEntry {
        label: "application",
        example: "λf:Num -> Num -> Num. f 1 2",
        note: "left associative, each argument is an atom",
    },
    SyntaxEntry {
        label: "binding",
        example: "let x = 5 in x * x",
        note: "",
    },
    SyntaxEntry {
        label: "pair",
        example: "(1, true)",
        note: "",
    },
    SyntaxEntry {
        label: "projection",
        example: "fst (1, true)",
        note: "fst e and snd e take apart a pair",
    },
    SyntaxEntry {
        label: "empty list",
        example: "nil",
        note: "",
    },
    SyntaxEntry {
        label: "cons",
        example: "1 :: 2 :: nil",
        note: ":: is right associative and every list ends in nil",
    },
    SyntaxEntry {
        label: "fold",
        example: "fold (1 :: 2 :: nil) 0 (λh:Num. λa:Num. h + a)",
        note: "fold LIST INIT STEP; the step takes the head, then the accumulator",
    },
    SyntaxEntry {
        label: "record",
        example: "{name = \"ada\", score = 90}",
        note: "field order is part of the value",
    },
    SyntaxEntry {
        label: "field",
        example: "{name = \"ada\", score = 90}.score",
        note: "a field access binds tighter than an application",
    },
    SyntaxEntry {
        label: "injection",
        example: "`Some 1",
        note: "`CASE payload builds one case of a variant",
    },
    SyntaxEntry {
        label: "match",
        example: "λs:?. match s { Idle u -> 0 | Busy n -> n }",
        note: "one arm per case, separated by |, each arm naming its payload",
    },
    SyntaxEntry {
        label: "command",
        example: "bind line <- readline in print line",
        note: "print e, readline, pure e, and bind x <- c in body",
    },
    SyntaxEntry {
        label: "pure",
        example: "pure 1",
        note: "",
    },
    SyntaxEntry {
        label: "empty hole",
        example: "1 + ⦇⦈",
        note: "an unfinished spot; ? is accepted for it as well",
    },
    SyntaxEntry {
        label: "quarantine",
        example: "1 + ⦇true⦈",
        note: "an expression the types refused, kept inside the brackets",
    },
    SyntaxEntry {
        label: "grouping",
        example: "(1 + 2) * 3",
        note: "",
    },
    SyntaxEntry {
        label: "base types",
        example: "λs:Str. s ++ \"!\"",
        note: "Num, Bool and Str",
    },
    SyntaxEntry {
        label: "unknown type",
        example: "λx:?. x",
        note: "? fits anything, and is how record and variant parameters are written",
    },
    SyntaxEntry {
        label: "function type",
        example: "λf:Num -> Bool. f 1",
        note: "",
    },
    SyntaxEntry {
        label: "product type",
        example: "λp:Num * Bool. fst p",
        note: "",
    },
    SyntaxEntry {
        label: "list type",
        example: "λxs:List Num. fold xs 0 (λh:Num. λa:Num. h + a)",
        note: "",
    },
    SyntaxEntry {
        label: "record type",
        example: "λp:{a: Num, b: Str}. p.a",
        note: "",
    },
    SyntaxEntry {
        label: "variant type",
        example: "λs:[A: Num | B: Str]. 1",
        note: "",
    },
    SyntaxEntry {
        label: "command type",
        example: "λc:Cmd Num. c",
        note: "",
    },
];

pub const POST_B2_ACTION_GRAMMAR: &[ActionEntry] = &[
    ActionEntry {
        example: "construct-num 42",
        effect: "write a number",
    },
    ActionEntry {
        example: "construct-bool true",
        effect: "write true or false",
    },
    ActionEntry {
        example: "construct-str \"text\"",
        effect: "write a string literal, in double quotes",
    },
    ActionEntry {
        example: "construct-var NAME",
        effect: "refer to an in-scope name",
    },
    ActionEntry {
        example: "construct-nil",
        effect: "write the empty list nil",
    },
    ActionEntry {
        example: "construct-cons",
        effect: "e becomes e :: ⦇⦈",
    },
    ActionEntry {
        example: "construct-lam",
        effect: "e becomes λx:?. e",
    },
    ActionEntry {
        example: "construct-ap",
        effect: "e becomes e ⦇⦈",
    },
    ActionEntry {
        example: "construct-binop add",
        effect: "e becomes e OP ⦇⦈   (add sub mul lt eq concat)",
    },
    ActionEntry {
        example: "construct-if",
        effect: "e becomes if e then ⦇⦈ else ⦇⦈",
    },
    ActionEntry {
        example: "construct-let",
        effect: "e becomes let x = e in ⦇⦈",
    },
    ActionEntry {
        example: "construct-pair",
        effect: "e becomes (e, ⦇⦈)",
    },
    ActionEntry {
        example: "construct-proj l",
        effect: "e becomes fst e, or snd e with r",
    },
    ActionEntry {
        example: "construct-fold",
        effect: "e becomes fold ⦇⦈ ⦇⦈ ⦇⦈, cursor on the list",
    },
    ActionEntry {
        example: "construct-record",
        effect: "e becomes a record with one field, cursor on its value",
    },
    ActionEntry {
        example: "add-field",
        effect: "add a field after this one (cursor inside a record)",
    },
    ActionEntry {
        example: "remove-field",
        effect: "drop this field (cursor inside a record)",
    },
    ActionEntry {
        example: "rename-field NAME",
        effect: "name the field the cursor is inside",
    },
    ActionEntry {
        example: "construct-field NAME",
        effect: "e becomes e.NAME; NAME must already exist in the program",
    },
    ActionEntry {
        example: "set-field NAME",
        effect: "point an existing field access at another field",
    },
    ActionEntry {
        example: "construct-inj",
        effect: "e becomes `Case e, cursor on the payload",
    },
    ActionEntry {
        example: "construct-match",
        effect: "e becomes match e {}, with e as the scrutinee",
    },
    ActionEntry {
        example: "add-arm",
        effect: "add an arm to the match, minting a new case",
    },
    ActionEntry {
        example: "remove-arm",
        effect: "drop the arm the cursor is inside",
    },
    ActionEntry {
        example: "set-constructor NAME",
        effect: "aim an injection or an arm at an existing case",
    },
    ActionEntry {
        example: "rename-constructor NAME",
        effect: "name the case of the arm or injection at the cursor",
    },
    ActionEntry {
        example: "delete",
        effect: "the focus becomes an empty hole",
    },
    ActionEntry {
        example: "finish",
        effect: "unwrap a quarantine ⦇e⦈ whose contents now fit",
    },
    ActionEntry {
        example: "set-ann Num",
        effect: "set the λ parameter's type (cursor on the λ); types are Num Bool Str ? List T Cmd T A -> B A * B",
    },
    ActionEntry {
        example: "rename NAME",
        effect: "rename the binder (cursor on the λ or the let)",
    },
    ActionEntry {
        example: "move-child 0",
        effect: "descend to child N, 0-based, source order",
    },
    ActionEntry {
        example: "move-parent",
        effect: "ascend",
    },
    ActionEntry {
        example: "move-next-sibling",
        effect: "next child of the same parent",
    },
    ActionEntry {
        example: "move-prev-sibling",
        effect: "previous child of the same parent",
    },
];

pub fn post_b2_syntax() -> String {
    let mut out = String::new();
    out.push_str("The program is written in this syntax. Every line below is a real program:\n");
    for entry in POST_B2_SYNTAX_ENTRIES {
        out.push_str(&format!("  {:<15}{}\n", entry.label, entry.example));
        if !entry.note.is_empty() {
            out.push_str(&format!("                 ({})\n", entry.note));
        }
    }
    out.push_str(
        "Every name you use must be bound by an enclosing λ, let, match arm, fold step\n\
         or bind. There are no built-in names and no recursion. Field names and case\n\
         names belong to the program: reuse the ones it already has.",
    );
    out
}

pub fn post_b2_action_grammar() -> String {
    let mut out = String::new();
    out.push_str("The action grammar:\n");
    for entry in POST_B2_ACTION_GRAMMAR {
        out.push_str(&format!("  {:<24} {}\n", entry.example, entry.effect));
    }
    out.push_str(
        "At an empty hole a construction fills the hole. On anything else the\n\
         construction wraps it, as shown above. After a construction the cursor lands\n\
         on the first new empty hole if the form has one.",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::text_parse::parse_program;
    use nothing_action::script::parse_step;
    use nothing_core::render::render;

    #[test]
    fn every_syntax_example_round_trips_through_the_renderer() {
        for entry in POST_B2_SYNTAX_ENTRIES {
            let parsed = parse_program(entry.example).unwrap_or_else(|e| {
                panic!("{}: `{}` did not parse: {e}", entry.label, entry.example)
            });
            assert_eq!(
                render(&parsed.exp, &parsed.names),
                entry.example,
                "{}: the legend does not match the renderer",
                entry.label
            );
        }
    }

    #[test]
    fn the_legend_covers_every_post_b2_construct() {
        let text = post_b2_syntax();
        for construct in [
            "\"hello, \"",
            "::",
            "nil",
            "fold",
            "{name",
            ".score",
            "`Some",
            "match",
            "List Num",
            "Cmd Num",
            "⦇⦈",
        ] {
            assert!(
                text.contains(construct),
                "the baseline legend never mentions `{construct}`"
            );
        }
    }

    #[test]
    fn every_advertised_action_parses_as_a_step() {
        for entry in POST_B2_ACTION_GRAMMAR {
            assert!(
                parse_step(entry.example).is_ok(),
                "the prompt advertises `{}`, which is not a step",
                entry.example
            );
        }
    }

    #[test]
    fn the_action_grammar_covers_every_post_b2_construction() {
        let text = post_b2_action_grammar();
        for step in [
            "construct-str",
            "construct-nil",
            "construct-cons",
            "construct-fold",
            "construct-record",
            "construct-field",
            "construct-inj",
            "construct-match",
            "add-arm",
            "rename-constructor",
            "rename-field",
            "construct-binop add",
        ] {
            assert!(
                text.contains(step),
                "the action grammar never mentions `{step}`"
            );
        }
    }

    #[test]
    fn the_original_legend_still_describes_the_original_surface() {
        assert!(ORIGINAL_SYNTAX.contains("no strings, no lists"));
    }
}
