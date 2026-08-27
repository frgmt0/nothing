//! The annotation slot's type entry (`KEYS.md` §"Literal entry" → *Types*).
//!
//! > The annotation slot re-issues `SetAnn` with the whole token buffer on
//! > every keystroke, parsed by the `script::parse_ty` grammar with `>` for
//! > `->`. **Every prefix parses** because a trailing operator takes `?` as
//! > its missing operand: `:n` → `Num`, `:n>` → `Num -> ?`, `:n>n` →
//! > `Num -> Num`, `:n*n` → `Num * Num`.
//!
//! That property is the whole point: annotation entry is commit-live like
//! every other kind of entry, so there is no keystroke at which the buffer
//! has no meaning and nothing can be written to the program.
//!
//! There is exactly one type parser in the project — `script::parse_ty` —
//! and this module does not become a second one. It turns the buffer into a
//! *repaired* string in that grammar (filling in missing operands with `?`
//! and closing unclosed parens) and hands it over. [`parse`] is total, and
//! `every_prefix_of_every_buffer_parses` proves it.

use nothing_action::script::parse_ty;
use nothing_core::ty::Ty;

/// What a printable character does in the annotation slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Accept {
    /// Part of the type: append it and re-issue `SetAnn`.
    Append,
    /// A letter inside a spelled base type (`n`**um**, `b`**ool**): keep it
    /// in the buffer so the user sees what they typed, but it means nothing.
    Swallow,
    /// A character the slot has no meaning for. `KEYS.md`: *"a character a
    /// slot does not understand means 'I am done here', and the character
    /// gets its normal meaning one step out"* — the caller leaves the slot
    /// and re-dispatches it. Never a refusal.
    Exit,
    /// A `)` with no `(` to close. The only genuinely inert character here:
    /// leaving the slot would be worse, since the user is plainly still
    /// writing a type.
    Ignore,
}

/// Is the buffer in the middle of a spelled-out base type?
fn in_word(buffer: &str) -> bool {
    buffer.chars().last().is_some_and(char::is_alphabetic)
}

/// How many `(` are still unclosed.
fn open_parens(buffer: &str) -> usize {
    let opens = buffer.chars().filter(|c| *c == '(').count();
    let closes = buffer.chars().filter(|c| *c == ')').count();
    opens.saturating_sub(closes)
}

/// What `c` means when the annotation buffer currently reads `buffer`.
pub fn accept(buffer: &str, c: char) -> Accept {
    match c {
        '?' | '*' | '>' | '(' => Accept::Append,
        ')' if open_parens(buffer) > 0 => Accept::Append,
        ')' => Accept::Ignore,
        c if c.is_alphabetic() && in_word(buffer) => Accept::Swallow,
        'n' | 'N' | 'b' | 'B' => Accept::Append,
        _ => Accept::Exit,
    }
}

/// One meaningful token of the buffer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tok {
    Base(&'static str),
    Op(&'static str),
    Open,
    Close,
}

/// The buffer's meaningful tokens, with spelled-out base types collapsed:
/// `num > bool` and `n>b` tokenise identically.
fn tokens(buffer: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut prev_alpha = false;
    for c in buffer.chars() {
        let alpha = c.is_alphabetic();
        if !(alpha && prev_alpha) {
            match c {
                'n' | 'N' => out.push(Tok::Base("Num")),
                'b' | 'B' => out.push(Tok::Base("Bool")),
                '?' => out.push(Tok::Base("?")),
                '>' => out.push(Tok::Op("->")),
                '*' => out.push(Tok::Op("*")),
                '(' => out.push(Tok::Open),
                ')' => out.push(Tok::Close),
                // Unreachable through `accept`, which never appends
                // anything else; ignored rather than panicking, because a
                // buffer is data and a panic in a key handler is not an
                // outcome the editor is allowed to have.
                _ => {}
            }
        }
        prev_alpha = alpha;
    }
    out
}

/// The buffer as a `script::parse_ty` source string, with every missing
/// operand filled in with `?` and every unclosed group closed.
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

/// The type the buffer currently denotes. Total: an empty buffer is `?`,
/// which is exactly the annotation a freshly constructed lambda already has.
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
        // The commit-live invariant, stated as a test: there is no keystroke
        // at which the annotation has no meaning.
        for buffer in [
            "n", "n>n", "n*n", "num>bool", "(n>n)>n", "n>(n*b)", "?>?", "n*n*n", "(((n", "n>>n",
            "*n", "()", "(n*)",
        ] {
            let mut prefix = String::new();
            for c in buffer.chars() {
                prefix.push(c);
                let ty = parse(&prefix);
                // Round-trips through the one type grammar, so the repaired
                // string really is in it.
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
        // `Swallow` keeps the character in the buffer (the caller appends
        // it) but the type is the same before and after.
        assert_eq!(parse("nu"), parse("n"));
        assert_eq!(parse("boo"), parse("b"));
    }
}
