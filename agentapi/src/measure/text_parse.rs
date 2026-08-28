use nothing_action::script::parse_ty;
use nothing_core::exp::{Exp, HoleId, Id, Op, Side};
use nothing_core::names::NameTable;

const KEYWORDS: &[&str] = &["if", "then", "else", "let", "in", "true", "false", "fst", "snd"];

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TextError(pub String);

impl std::fmt::Display for TextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TextError {}

#[derive(Clone, PartialEq, Debug)]
pub struct Parsed {
    pub exp: Exp,
    pub names: NameTable,
}

pub fn strip_fences(text: &str) -> String {
    if text.lines().any(|line| line.trim().starts_with("```")) {
        let inner: Vec<&str> = text
            .lines()
            .skip_while(|line| !line.trim().starts_with("```"))
            .skip(1)
            .take_while(|line| !line.trim().starts_with("```"))
            .collect();
        return inner.join("\n").trim().to_string();
    }
    text.trim().to_string()
}

pub fn parse_program(text: &str) -> Result<Parsed, TextError> {
    let mut names = NameTable::new();
    let mut parser = Parser {
        chars: text.chars().collect(),
        pos: 0,
        scope: Vec::new(),
        next_id: 0,
        next_hole: 0,
        names: &mut names,
    };
    parser.skip_ws();
    let exp = parser.expr()?;
    parser.skip_ws();
    if parser.pos != parser.chars.len() {
        let rest: String = parser.chars[parser.pos..].iter().collect();
        return Err(TextError(format!("trailing input `{rest}`")));
    }
    Ok(Parsed { exp, names })
}

struct Parser<'a> {
    chars: Vec<char>,
    pos: usize,
    scope: Vec<(String, Id)>,
    next_id: u128,
    next_hole: u128,
    names: &'a mut NameTable,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, text: &str) -> bool {
        let save = self.pos;
        for expected in text.chars() {
            if self.peek() != Some(expected) {
                self.pos = save;
                return false;
            }
            self.pos += 1;
        }
        true
    }

    fn eat_symbol(&mut self, text: &str) -> bool {
        self.skip_ws();
        self.eat(text)
    }

    fn peek_word(&mut self) -> Option<String> {
        self.skip_ws();
        let start = self.pos;
        let mut end = start;
        while matches!(self.chars.get(end), Some(c) if c.is_alphanumeric() || *c == '_') {
            end += 1;
        }
        if end == start {
            return None;
        }
        Some(self.chars[start..end].iter().collect())
    }

    fn eat_word(&mut self, word: &str) -> bool {
        match self.peek_word() {
            Some(found) if found == word => {
                self.pos += word.chars().count();
                true
            }
            _ => false,
        }
    }

    fn take_word(&mut self) -> Option<String> {
        let word = self.peek_word()?;
        self.pos += word.chars().count();
        Some(word)
    }

    fn fresh_id(&mut self) -> Id {
        self.next_id += 1;
        Id::from_u128(0x7465_7874_0000_0000_0000_0000_0000_0000 | self.next_id)
    }

    fn fresh_hole(&mut self) -> HoleId {
        self.next_hole += 1;
        HoleId::from_u128(0x686f_6c65_0000_0000_0000_0000_0000_0000 | self.next_hole)
    }

    fn lookup(&self, name: &str) -> Option<Id> {
        self.scope
            .iter()
            .rev()
            .find(|(bound, _)| bound == name)
            .map(|(_, id)| *id)
    }

    fn expr(&mut self) -> Result<Exp, TextError> {
        self.skip_ws();
        if self.eat("λ") || self.eat("\\") {
            return self.lambda();
        }
        if self.eat_word("let") {
            return self.let_();
        }
        if self.eat_word("if") {
            return self.if_();
        }
        self.cmp()
    }

    fn lambda(&mut self) -> Result<Exp, TextError> {
        self.skip_ws();
        let name = self
            .take_word()
            .ok_or_else(|| TextError("a lambda needs a parameter name".to_string()))?;
        if KEYWORDS.contains(&name.as_str()) {
            return Err(TextError(format!("`{name}` is a keyword, not a name")));
        }
        if !self.eat_symbol(":") {
            return Err(TextError("a lambda parameter needs `:` and a type".to_string()));
        }
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c != '.') {
            self.pos += 1;
        }
        if self.peek() != Some('.') {
            return Err(TextError("a lambda annotation must end with `.`".to_string()));
        }
        let ty_text: String = self.chars[start..self.pos].iter().collect();
        self.pos += 1;
        let ty = parse_ty(ty_text.trim()).map_err(|e| TextError(e.to_string()))?;

        let id = self.fresh_id();
        self.names.set(id, name.clone());
        self.scope.push((name, id));
        let body = self.expr();
        self.scope.pop();
        Ok(Exp::lam(id, ty, body?))
    }

    fn let_(&mut self) -> Result<Exp, TextError> {
        self.skip_ws();
        let name = self
            .take_word()
            .ok_or_else(|| TextError("a let needs a name".to_string()))?;
        if KEYWORDS.contains(&name.as_str()) {
            return Err(TextError(format!("`{name}` is a keyword, not a name")));
        }
        if !self.eat_symbol("=") {
            return Err(TextError("a let needs `=`".to_string()));
        }
        let bound = self.expr()?;
        if !self.eat_word("in") {
            return Err(TextError("a let needs `in`".to_string()));
        }
        let id = self.fresh_id();
        self.names.set(id, name.clone());
        self.scope.push((name, id));
        let body = self.expr();
        self.scope.pop();
        Ok(Exp::let_(id, bound, body?))
    }

    fn if_(&mut self) -> Result<Exp, TextError> {
        let cond = self.expr()?;
        if !self.eat_word("then") {
            return Err(TextError("an if needs `then`".to_string()));
        }
        let then = self.expr()?;
        if !self.eat_word("else") {
            return Err(TextError("an if needs `else`".to_string()));
        }
        let else_ = self.expr()?;
        Ok(Exp::if_(cond, then, else_))
    }

    fn cmp(&mut self) -> Result<Exp, TextError> {
        let mut left = self.additive()?;
        loop {
            self.skip_ws();
            let op = if self.eat("==") {
                Op::Eq
            } else if self.peek() == Some('<') {
                self.pos += 1;
                Op::Lt
            } else {
                return Ok(left);
            };
            let right = self.additive()?;
            left = Exp::bin_op(op, left, right);
        }
    }

    fn additive(&mut self) -> Result<Exp, TextError> {
        let mut left = self.multiplicative()?;
        loop {
            self.skip_ws();
            let op = match self.peek() {
                Some('+') => Op::Add,
                Some('-') if self.chars.get(self.pos + 1) != Some(&'>') => Op::Sub,
                _ => return Ok(left),
            };
            self.pos += 1;
            let right = self.multiplicative()?;
            left = Exp::bin_op(op, left, right);
        }
    }

    fn multiplicative(&mut self) -> Result<Exp, TextError> {
        let mut left = self.application()?;
        loop {
            self.skip_ws();
            if self.peek() != Some('*') {
                return Ok(left);
            }
            self.pos += 1;
            let right = self.application()?;
            left = Exp::bin_op(Op::Mul, left, right);
        }
    }

    fn application(&mut self) -> Result<Exp, TextError> {
        let mut head = if self.eat_word("fst") {
            Exp::proj(Side::L, self.atom()?)
        } else if self.eat_word("snd") {
            Exp::proj(Side::R, self.atom()?)
        } else {
            self.atom()?
        };
        loop {
            let save = self.pos;
            self.skip_ws();
            if !self.starts_atom() {
                self.pos = save;
                return Ok(head);
            }
            match self.atom() {
                Ok(arg) => head = Exp::ap(head, arg),
                Err(_) => {
                    self.pos = save;
                    return Ok(head);
                }
            }
        }
    }

    fn starts_atom(&mut self) -> bool {
        match self.peek() {
            Some('(') | Some('⦇') | Some('?') => true,
            Some(c) if c.is_ascii_digit() => true,
            Some('-') => matches!(self.chars.get(self.pos + 1), Some(c) if c.is_ascii_digit()),
            Some(c) if c.is_alphabetic() || c == '_' => match self.peek_word() {
                Some(word) => !["then", "else", "in"].contains(&word.as_str()),
                None => false,
            },
            _ => false,
        }
    }

    fn atom(&mut self) -> Result<Exp, TextError> {
        self.skip_ws();
        match self.peek() {
            None => Err(TextError("unexpected end of program".to_string())),
            Some('?') => {
                self.pos += 1;
                Ok(Exp::empty_hole(self.fresh_hole()))
            }
            Some('⦇') => {
                self.pos += 1;
                if self.peek() == Some('⦈') {
                    self.pos += 1;
                    return Ok(Exp::empty_hole(self.fresh_hole()));
                }
                let inner = self.expr()?;
                self.skip_ws();
                if self.peek() != Some('⦈') {
                    return Err(TextError("unclosed `⦇`".to_string()));
                }
                self.pos += 1;
                Ok(Exp::non_empty_hole(self.fresh_hole(), inner))
            }
            Some('(') => {
                self.pos += 1;
                let first = self.expr()?;
                self.skip_ws();
                if self.peek() == Some(',') {
                    self.pos += 1;
                    let second = self.expr()?;
                    self.skip_ws();
                    if self.peek() != Some(')') {
                        return Err(TextError("unclosed `(` in a pair".to_string()));
                    }
                    self.pos += 1;
                    return Ok(Exp::pair(first, second));
                }
                if self.peek() != Some(')') {
                    return Err(TextError("unclosed `(`".to_string()));
                }
                self.pos += 1;
                Ok(first)
            }
            Some(c) if c.is_ascii_digit() || c == '-' => self.number(),
            Some(c) if c.is_alphabetic() || c == '_' => {
                let word = self
                    .take_word()
                    .ok_or_else(|| TextError("expected a name".to_string()))?;
                match word.as_str() {
                    "true" => Ok(Exp::bool_(true)),
                    "false" => Ok(Exp::bool_(false)),
                    other if KEYWORDS.contains(&other) => {
                        Err(TextError(format!("`{other}` cannot start an expression")))
                    }
                    other => match self.lookup(other) {
                        Some(id) => Ok(Exp::var(id)),
                        None => Err(TextError(format!("`{other}` is not in scope"))),
                    },
                }
            }
            Some(c) => Err(TextError(format!("unexpected character `{c}`"))),
        }
    }

    fn number(&mut self) -> Result<Exp, TextError> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        text.parse::<i64>()
            .map(Exp::num)
            .map_err(|e| TextError(format!("bad number `{text}`: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_action::generate;
    use nothing_core::examples;
    use nothing_core::render::render;
    use nothing_core::typing::is_well_typed;

    fn round_trips(exp: &nothing_core::exp::Exp, names: &NameTable) {
        let text = render(exp, names);
        let parsed = parse_program(&text)
            .unwrap_or_else(|e| panic!("`{text}` did not parse: {e}"));
        assert_eq!(
            render(&parsed.exp, &parsed.names),
            text,
            "reparsing `{text}` produced a different program"
        );
    }

    #[test]
    fn every_example_program_round_trips_through_the_renderer() {
        let names = examples::names();
        for exp in [
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
        ] {
            round_trips(&exp, &names);
        }
    }

    #[test]
    fn generated_closed_programs_round_trip_through_the_renderer() {
        let names = examples::names();
        let mut checked = 0;
        for seed in 0..400u64 {
            let exp = generate::well_typed_exp(seed);
            round_trips(&exp, &names);
            checked += 1;
        }
        assert!(checked >= 400);
    }

    #[test]
    fn a_parsed_program_is_well_typed_when_the_text_describes_one() {
        let parsed = parse_program("λn:Num. if n == 0 then 1 else n * 2").unwrap();
        assert!(is_well_typed(&parsed.exp));
        assert_eq!(render(&parsed.exp, &parsed.names), "λn:Num. if n == 0 then 1 else n * 2");
    }

    #[test]
    fn an_ill_typed_program_parses_but_does_not_typecheck() {
        let parsed = parse_program("1 + true").unwrap();
        assert!(!is_well_typed(&parsed.exp));
    }

    #[test]
    fn a_free_name_is_a_parse_error() {
        assert!(parse_program("n + 1").is_err());
        assert!(parse_program("λn:Num. m").is_err());
    }

    #[test]
    fn malformed_text_is_an_error_not_a_panic() {
        for text in [
            "",
            "(",
            "1 +",
            "if 1 then",
            "let x = 1",
            "λx. x",
            "λx:Str. x",
            "λ:Num. 1",
            "(1, 2",
            "⦇1",
            "then",
        ] {
            assert!(parse_program(text).is_err(), "`{text}` should not parse");
        }
    }

    #[test]
    fn a_backslash_is_accepted_for_the_lambda() {
        let parsed = parse_program("\\n:Num. n + 1").unwrap();
        assert_eq!(render(&parsed.exp, &parsed.names), "λn:Num. n + 1");
    }

    #[test]
    fn a_question_mark_is_accepted_for_an_empty_hole() {
        let parsed = parse_program("1 + ?").unwrap();
        assert_eq!(render(&parsed.exp, &parsed.names), "1 + ⦇⦈");
    }

    #[test]
    fn shadowing_resolves_to_the_innermost_binder() {
        let parsed = parse_program("λx:Num. λx:Bool. x").unwrap();
        match &parsed.exp {
            Exp::Lam(_, _, body) => match &**body {
                Exp::Lam(inner, _, used) => match &**used {
                    Exp::Var(id) => assert_eq!(id, inner),
                    other => panic!("expected a variable, got {other:?}"),
                },
                other => panic!("expected a lambda, got {other:?}"),
            },
            other => panic!("expected a lambda, got {other:?}"),
        }
    }

    #[test]
    fn application_is_left_associative_and_arguments_are_atoms() {
        let parsed = parse_program("λf:Num -> Num -> Num. f 1 2").unwrap();
        assert!(is_well_typed(&parsed.exp));
        assert_eq!(render(&parsed.exp, &parsed.names), "λf:Num -> Num -> Num. f 1 2");
    }

    #[test]
    fn newlines_and_extra_spacing_do_not_matter() {
        let parsed = parse_program("λn:Num.\n  if n == 0\n  then 1\n  else n * 2").unwrap();
        assert_eq!(render(&parsed.exp, &parsed.names), "λn:Num. if n == 0 then 1 else n * 2");
    }

    #[test]
    fn fences_and_prose_are_stripped_before_parsing() {
        let reply = "Here you go:\n\n```\nλn:Num. n + 1\n```\n\nHope that helps.";
        assert_eq!(strip_fences(reply), "λn:Num. n + 1");
        assert!(parse_program(&strip_fences(reply)).is_ok());
    }
}
