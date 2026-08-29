use nothing_action::script::parse_ty;
use nothing_core::ty::Ty;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Accept {
    Append,
    Swallow,
    Exit,
    Ignore,
}

fn in_word(buffer: &str) -> bool {
    buffer.chars().last().is_some_and(char::is_alphabetic)
}

fn open_parens(buffer: &str) -> usize {
    let opens = buffer.chars().filter(|c| *c == '(').count();
    let closes = buffer.chars().filter(|c| *c == ')').count();
    opens.saturating_sub(closes)
}

pub fn accept(buffer: &str, c: char) -> Accept {
    match c {
        '?' | '*' | '>' | '(' | '[' => Accept::Append,
        ')' if open_parens(buffer) > 0 => Accept::Append,
        ')' => Accept::Ignore,
        c if c.is_alphabetic() && in_word(buffer) => Accept::Swallow,
        'n' | 'N' | 'b' | 'B' | 's' | 'S' => Accept::Append,
        _ => Accept::Exit,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tok {
    Base(&'static str),
    Prefix(&'static str),
    Op(&'static str),
    Open,
    Close,
}

fn tokens(buffer: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut prev_alpha = false;
    for c in buffer.chars() {
        let alpha = c.is_alphabetic();
        if !(alpha && prev_alpha) {
            match c {
                'n' | 'N' => out.push(Tok::Base("Num")),
                'b' | 'B' => out.push(Tok::Base("Bool")),
                's' | 'S' => out.push(Tok::Base("Str")),
                '?' => out.push(Tok::Base("?")),
                '[' => out.push(Tok::Prefix("List")),
                '>' => out.push(Tok::Op("->")),
                '*' => out.push(Tok::Op("*")),
                '(' => out.push(Tok::Open),
                ')' => out.push(Tok::Close),

                _ => {}
            }
        }
        prev_alpha = alpha;
    }
    out
}

fn repaired(buffer: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut want_operand = true;
    let mut depth = 0usize;
    for tok in tokens(buffer) {
        match tok {
            Tok::Base(name) => {
                out.push(name);
                want_operand = false;
            }
            Tok::Prefix(name) => {
                if want_operand {
                    out.push(name);
                }
            }
            Tok::Op(op) => {
                if want_operand {
                    out.push("?");
                }
                out.push(op);
                want_operand = true;
            }
            Tok::Open => {
                out.push("(");
                want_operand = true;
                depth += 1;
            }
            Tok::Close => {
                if depth == 0 {
                    continue;
                }
                if want_operand {
                    out.push("?");
                }
                out.push(")");
                want_operand = false;
                depth -= 1;
            }
        }
    }
    if want_operand {
        out.push("?");
    }
    out.extend(std::iter::repeat_n(")", depth));
    out.join(" ")
}

pub fn parse(buffer: &str) -> Ty {
    parse_ty(&repaired(buffer)).unwrap_or(Ty::Hole)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arrow(a: Ty, b: Ty) -> Ty {
        Ty::Arrow(Box::new(a), Box::new(b))
    }

    fn prod(a: Ty, b: Ty) -> Ty {
        Ty::Prod(Box::new(a), Box::new(b))
    }

    #[test]
    fn the_documented_prefixes_parse_as_documented() {
        assert_eq!(parse(""), Ty::Hole);
        assert_eq!(parse("n"), Ty::Num);
        assert_eq!(parse("n>"), arrow(Ty::Num, Ty::Hole));
        assert_eq!(parse("n>n"), arrow(Ty::Num, Ty::Num));
        assert_eq!(parse("n*n"), prod(Ty::Num, Ty::Num));
        assert_eq!(parse("b"), Ty::Bool);
        assert_eq!(parse("?"), Ty::Hole);
    }

    #[test]
    fn spelled_base_types_mean_the_same_as_their_initials() {
        assert_eq!(parse("num"), parse("n"));
        assert_eq!(parse("bool"), parse("b"));
        assert_eq!(parse("num>bool"), parse("n>b"));
        assert_eq!(parse("num*num>bool"), parse("n*n>b"));
    }

    #[test]
    fn precedence_and_grouping_follow_the_one_type_grammar() {
        assert_eq!(parse("n*n>n"), arrow(prod(Ty::Num, Ty::Num), Ty::Num));
        assert_eq!(parse("(n>n)>n"), arrow(arrow(Ty::Num, Ty::Num), Ty::Num));
        assert_eq!(parse("n>n>n"), arrow(Ty::Num, arrow(Ty::Num, Ty::Num)));
    }

    #[test]
    fn every_prefix_of_every_buffer_parses() {
        for buffer in [
            "n", "n>n", "n*n", "num>bool", "(n>n)>n", "n>(n*b)", "?>?", "n*n*n", "(((n", "n>>n",
            "*n", "()", "(n*)", "[n", "[[n", "[n>n", "([n)", "n>[b", "[n*[b", "[", "n[",
        ] {
            let mut prefix = String::new();
            for c in buffer.chars() {
                prefix.push(c);
                let ty = parse(&prefix);

                assert_eq!(
                    parse_ty(&ty.to_string()).unwrap(),
                    ty,
                    "`{prefix}` produced {ty}, which is not in the grammar"
                );
            }
        }
    }

    #[test]
    fn a_character_the_slot_does_not_understand_exits_rather_than_refusing() {
        assert_eq!(accept("", 'n'), Accept::Append);
        assert_eq!(accept("n", 'u'), Accept::Swallow);
        assert_eq!(accept("num", 'm'), Accept::Swallow);
        assert_eq!(accept("num>", 'b'), Accept::Append);
        assert_eq!(accept("", 'x'), Accept::Exit);
        assert_eq!(accept("n", '5'), Accept::Exit);
        assert_eq!(accept("n", '+'), Accept::Exit);
        assert_eq!(accept("n", '*'), Accept::Append);
        assert_eq!(accept("(n", ')'), Accept::Append);
        assert_eq!(accept("n", ')'), Accept::Ignore);
    }

    #[test]
    fn swallowed_letters_change_nothing_but_stay_visible() {
        assert_eq!(parse("nu"), parse("n"));
        assert_eq!(parse("boo"), parse("b"));
    }

    fn list(elem: Ty) -> Ty {
        Ty::List(Box::new(elem))
    }

    #[test]
    fn a_bracket_is_the_list_prefix_and_takes_the_next_type() {
        assert_eq!(parse("["), list(Ty::Hole));
        assert_eq!(parse("[n"), list(Ty::Num));
        assert_eq!(parse("[[n"), list(list(Ty::Num)));
        assert_eq!(parse("[n>n"), arrow(list(Ty::Num), Ty::Num));
        assert_eq!(parse("[(n>n)"), list(arrow(Ty::Num, Ty::Num)));
        assert_eq!(parse("[n*[b"), prod(list(Ty::Num), list(Ty::Bool)));
        assert_eq!(accept("", '['), Accept::Append);
        assert_eq!(accept("n", '['), Accept::Append);
        assert_eq!(parse("n["), parse("n"));
    }
}
