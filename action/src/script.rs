
use std::fmt;

use nothing_core::exp::{Id, Op, Side};
use nothing_core::ty::Ty;

use crate::act::{Action, EditState};

#[derive(Clone, PartialEq, Debug)]
pub enum Command {
    Act(Action),
    Quit,
    Show,
    Reset,
    Help,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScriptError {
    pub line: usize,
    pub text: String,
    pub message: String,
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}: {}", self.line, self.message, self.text)
    }
}

impl std::error::Error for ScriptError {}

pub const HELP: &str = "\
movement:
  move-child N            descend into child N (0-based, source order)
  move-parent             ascend to the parent
  move-next-sibling       next child of the same parent   (alias: move-next)
  move-prev-sibling       previous child of the same parent (alias: move-prev)
editing:
  delete                  replace the focus with an empty hole
  construct-num N         write a numeric literal
  construct-bool BOOL     write true or false
  construct-var N         reference the in-scope binder with id N
  construct-lam           e becomes λx:?. e
  construct-ap            e becomes e ⦇⦈
  construct-binop OP      e becomes e OP ⦇⦈   (add|sub|mul|lt|eq, or + - * < ==)
  construct-if            e becomes if e then ⦇⦈ else ⦇⦈
  construct-let           e becomes let x = e in ⦇⦈
  construct-pair          e becomes (e, ⦇⦈)
  construct-proj SIDE     e becomes fst e / snd e   (l|r, fst|snd, left|right)
  construct-non-empty-hole  e becomes ⦇e⦈
  set-ann TY              set the focused lambda's annotation
                          TY := Num | Bool | ? | TY -> TY | TY * TY | ( TY )
  set-binder-id N         re-identify the focused lambda or let binder
  finish                  unwrap a non-empty hole whose contents now fit
harness:
  show                    re-print the current program
  reset                   start again from ⦇⦈
  help                    print this
  quit                    stop reading input
  # ...                   comment (ignored); blank lines are ignored too";

pub fn parse_command(line: &str) -> Result<Option<Command>, ParseError> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    match line {
        "quit" | "q" | "exit" => return Ok(Some(Command::Quit)),
        "show" => return Ok(Some(Command::Show)),
        "reset" => return Ok(Some(Command::Reset)),
        "help" | "?" => return Ok(Some(Command::Help)),
        _ => {}
    }
    parse_action(line).map(|a| Some(Command::Act(a)))
}

pub fn parse_action(line: &str) -> Result<Action, ParseError> {
    let line = line.trim();
    let (head, rest) = match line.find(char::is_whitespace) {
        Some(i) => (&line[..i], line[i..].trim()),
        None => (line, ""),
    };

    let no_arg = |action: Action| -> Result<Action, ParseError> {
        if rest.is_empty() {
            Ok(action)
        } else {
            Err(ParseError(format!(
                "`{head}` takes no argument, got `{rest}`"
            )))
        }
    };

    match head {
        "move-child" => Ok(Action::MoveChild(parse_usize(head, rest)?)),
        "move-parent" => no_arg(Action::MoveParent),
        "move-next-sibling" | "move-next" => no_arg(Action::MoveNextSibling),
        "move-prev-sibling" | "move-prev" => no_arg(Action::MovePrevSibling),

        "delete" => no_arg(Action::Delete),

        "construct-num" => Ok(Action::ConstructNum(parse_i64(head, rest)?)),
        "construct-bool" => Ok(Action::ConstructBool(parse_bool(head, rest)?)),
        "construct-var" => Ok(Action::ConstructVar(Id::new(parse_u64(head, rest)?))),
        "construct-lam" => no_arg(Action::ConstructLam),
        "construct-ap" => no_arg(Action::ConstructAp),
        "construct-binop" => Ok(Action::ConstructBinOp(parse_op(rest)?)),
        "construct-if" => no_arg(Action::ConstructIf),
        "construct-let" => no_arg(Action::ConstructLet),
        "construct-pair" => no_arg(Action::ConstructPair),
        "construct-proj" => Ok(Action::ConstructProj(parse_side(rest)?)),
        "construct-non-empty-hole" => no_arg(Action::ConstructNonEmptyHole),

        "set-ann" => Ok(Action::SetAnn(parse_ty(rest)?)),
        "set-binder-id" => Ok(Action::SetBinderId(Id::new(parse_u64(head, rest)?))),
        "finish" => no_arg(Action::Finish),

        "" => Err(ParseError("empty command".to_string())),
        other => Err(ParseError(format!("unknown action `{other}`"))),
    }
}

pub fn action_name(action: &Action) -> String {
    match action {
        Action::MoveChild(n) => format!("move-child {n}"),
        Action::MoveParent => "move-parent".to_string(),
        Action::MoveNextSibling => "move-next-sibling".to_string(),
        Action::MovePrevSibling => "move-prev-sibling".to_string(),
        Action::Delete => "delete".to_string(),
        Action::ConstructNum(n) => format!("construct-num {n}"),
        Action::ConstructBool(b) => format!("construct-bool {b}"),
        Action::ConstructVar(id) => format!("construct-var {}", id.0),
        Action::ConstructLam => "construct-lam".to_string(),
        Action::ConstructAp => "construct-ap".to_string(),
        Action::ConstructBinOp(op) => format!("construct-binop {}", op_name(*op)),
        Action::ConstructIf => "construct-if".to_string(),
        Action::ConstructLet => "construct-let".to_string(),
        Action::ConstructPair => "construct-pair".to_string(),
        Action::ConstructProj(side) => format!("construct-proj {}", side_name(*side)),
        Action::ConstructNonEmptyHole => "construct-non-empty-hole".to_string(),
        Action::SetAnn(ty) => format!("set-ann {ty}"),
        Action::SetBinderId(id) => format!("set-binder-id {}", id.0),
        Action::Finish => "finish".to_string(),
    }
}

fn op_name(op: Op) -> &'static str {
    match op {
        Op::Add => "add",
        Op::Sub => "sub",
        Op::Mul => "mul",
        Op::Lt => "lt",
        Op::Eq => "eq",
    }
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::L => "l",
        Side::R => "r",
    }
}

fn parse_usize(head: &str, rest: &str) -> Result<usize, ParseError> {
    rest.parse::<usize>()
        .map_err(|_| ParseError(format!("`{head}` expects a child index, got `{rest}`")))
}

fn parse_u64(head: &str, rest: &str) -> Result<u64, ParseError> {
    rest.parse::<u64>()
        .map_err(|_| ParseError(format!("`{head}` expects a binder id, got `{rest}`")))
}

fn parse_i64(head: &str, rest: &str) -> Result<i64, ParseError> {
    rest.parse::<i64>()
        .map_err(|_| ParseError(format!("`{head}` expects an integer, got `{rest}`")))
}

fn parse_bool(head: &str, rest: &str) -> Result<bool, ParseError> {
    match rest {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(ParseError(format!(
            "`{head}` expects `true` or `false`, got `{other}`"
        ))),
    }
}

fn parse_op(rest: &str) -> Result<Op, ParseError> {
    match rest {
        "add" | "+" => Ok(Op::Add),
        "sub" | "-" => Ok(Op::Sub),
        "mul" | "*" => Ok(Op::Mul),
        "lt" | "<" => Ok(Op::Lt),
        "eq" | "==" => Ok(Op::Eq),
        other => Err(ParseError(format!(
            "unknown operator `{other}` (expected add|sub|mul|lt|eq)"
        ))),
    }
}

fn parse_side(rest: &str) -> Result<Side, ParseError> {
    match rest {
        "l" | "left" | "fst" | "0" => Ok(Side::L),
        "r" | "right" | "snd" | "1" => Ok(Side::R),
        other => Err(ParseError(format!(
            "unknown projection side `{other}` (expected l|r)"
        ))),
    }
}


pub fn parse_ty(text: &str) -> Result<Ty, ParseError> {
    let tokens = lex_ty(text)?;
    let mut pos = 0;
    let ty = ty_arrow(&tokens, &mut pos)?;
    if pos != tokens.len() {
        return Err(ParseError(format!(
            "trailing input in type `{text}` at `{}`",
            tokens[pos]
        )));
    }
    Ok(ty)
}

fn lex_ty(text: &str) -> Result<Vec<String>, ParseError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '(' || c == ')' || c == '*' || c == '?' {
            tokens.push(c.to_string());
            i += 1;
        } else if c == '-' && chars.get(i + 1) == Some(&'>') {
            tokens.push("->".to_string());
            i += 2;
        } else if c.is_alphanumeric() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(chars[start..i].iter().collect());
        } else {
            return Err(ParseError(format!("unexpected character `{c}` in type")));
        }
    }
    if tokens.is_empty() {
        return Err(ParseError("expected a type, got nothing".to_string()));
    }
    Ok(tokens)
}

fn ty_arrow(tokens: &[String], pos: &mut usize) -> Result<Ty, ParseError> {
    let left = ty_prod(tokens, pos)?;
    if tokens.get(*pos).map(String::as_str) == Some("->") {
        *pos += 1;
        let right = ty_arrow(tokens, pos)?;
        Ok(Ty::Arrow(Box::new(left), Box::new(right)))
    } else {
        Ok(left)
    }
}

fn ty_prod(tokens: &[String], pos: &mut usize) -> Result<Ty, ParseError> {
    let mut left = ty_atom(tokens, pos)?;
    while tokens.get(*pos).map(String::as_str) == Some("*") {
        *pos += 1;
        let right = ty_atom(tokens, pos)?;
        left = Ty::Prod(Box::new(left), Box::new(right));
    }
    Ok(left)
}

fn ty_atom(tokens: &[String], pos: &mut usize) -> Result<Ty, ParseError> {
    let Some(token) = tokens.get(*pos) else {
        return Err(ParseError("unexpected end of type".to_string()));
    };
    *pos += 1;
    match token.to_ascii_lowercase().as_str() {
        "num" => Ok(Ty::Num),
        "bool" => Ok(Ty::Bool),
        "?" | "hole" => Ok(Ty::Hole),
        "(" => {
            let inner = ty_arrow(tokens, pos)?;
            if tokens.get(*pos).map(String::as_str) != Some(")") {
                return Err(ParseError("unclosed `(` in type".to_string()));
            }
            *pos += 1;
            Ok(inner)
        }
        other => Err(ParseError(format!("unknown type `{other}`"))),
    }
}


pub fn parse_script(text: &str) -> Result<Vec<Action>, ScriptError> {
    Ok(parse_numbered_script(text)?
        .into_iter()
        .map(|(_, action)| action)
        .collect())
}

pub fn parse_numbered_script(text: &str) -> Result<Vec<(usize, Action)>, ScriptError> {
    let mut actions = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let at = |message: String| ScriptError {
            line: i + 1,
            text: line.to_string(),
            message,
        };
        match parse_command(line).map_err(|e| at(e.0))? {
            None => {}
            Some(Command::Act(action)) => actions.push((i + 1, action)),
            Some(Command::Quit) => break,
            Some(_) => {
                return Err(at(
                    "harness commands (show/reset/help) are not allowed in a script".to_string(),
                ));
            }
        }
    }
    Ok(actions)
}

pub fn replay_script(text: &str) -> Result<EditState, ScriptError> {
    let lines: Vec<&str> = text.lines().collect();
    let mut state = EditState::empty();
    for (line_no, action) in parse_numbered_script(text)? {
        if !state.apply_mut(action.clone()) {
            return Err(ScriptError {
                line: line_no,
                text: lines[line_no - 1].to_string(),
                message: format!("action did not apply: {}", action_name(&action)),
            });
        }
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::render::render;
    use nothing_core::typing::is_well_typed;

    fn every_action() -> Vec<Action> {
        vec![
            Action::MoveChild(0),
            Action::MoveChild(7),
            Action::MoveParent,
            Action::MoveNextSibling,
            Action::MovePrevSibling,
            Action::Delete,
            Action::ConstructNum(0),
            Action::ConstructNum(-12),
            Action::ConstructBool(true),
            Action::ConstructBool(false),
            Action::ConstructVar(Id::new(3)),
            Action::ConstructLam,
            Action::ConstructAp,
            Action::ConstructBinOp(Op::Add),
            Action::ConstructBinOp(Op::Sub),
            Action::ConstructBinOp(Op::Mul),
            Action::ConstructBinOp(Op::Lt),
            Action::ConstructBinOp(Op::Eq),
            Action::ConstructIf,
            Action::ConstructLet,
            Action::ConstructPair,
            Action::ConstructProj(Side::L),
            Action::ConstructProj(Side::R),
            Action::ConstructNonEmptyHole,
            Action::SetAnn(Ty::Num),
            Action::SetAnn(Ty::Bool),
            Action::SetAnn(Ty::Hole),
            Action::SetAnn(Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Bool))),
            Action::SetAnn(Ty::Prod(Box::new(Ty::Num), Box::new(Ty::Num))),
            Action::SetBinderId(Id::new(9)),
            Action::Finish,
        ]
    }

    #[test]
    fn action_name_and_parse_action_are_inverse() {
        for action in every_action() {
            let name = action_name(&action);
            let parsed = parse_action(&name)
                .unwrap_or_else(|e| panic!("`{name}` did not parse back: {e}"));
            assert_eq!(parsed, action, "round trip failed for `{name}`");
        }
    }

    #[test]
    fn every_action_is_documented_in_the_help_text() {
        let documented: Vec<&str> = HELP
            .lines()
            .filter_map(|line| line.strip_prefix("  "))
            .filter(|line| !line.starts_with(' '))
            .filter_map(|line| line.split_whitespace().next())
            .collect();
        for action in every_action() {
            let name = action_name(&action);
            let head = name.split_whitespace().next().unwrap();
            assert!(
                documented.contains(&head),
                "`{head}` is a real action but `help` does not list it"
            );
        }
    }

    #[test]
    fn blank_lines_and_comments_are_ignored() {
        assert_eq!(parse_command("").unwrap(), None);
        assert_eq!(parse_command("   ").unwrap(), None);
        assert_eq!(parse_command("# a comment").unwrap(), None);
        assert_eq!(parse_command("   # indented comment").unwrap(), None);
    }

    #[test]
    fn harness_commands_parse() {
        assert_eq!(parse_command("quit").unwrap(), Some(Command::Quit));
        assert_eq!(parse_command("show").unwrap(), Some(Command::Show));
        assert_eq!(parse_command("reset").unwrap(), Some(Command::Reset));
        assert_eq!(parse_command("help").unwrap(), Some(Command::Help));
    }

    #[test]
    fn unknown_and_malformed_input_is_an_error_not_a_panic() {
        assert!(parse_action("frobnicate").is_err());
        assert!(parse_action("move-child").is_err());
        assert!(parse_action("move-child x").is_err());
        assert!(parse_action("move-child -1").is_err());
        assert!(parse_action("move-parent 3").is_err());
        assert!(parse_action("construct-num").is_err());
        assert!(parse_action("construct-num 1.5").is_err());
        assert!(parse_action("construct-bool yes").is_err());
        assert!(parse_action("construct-binop pow").is_err());
        assert!(parse_action("construct-proj middle").is_err());
        assert!(parse_action("set-ann").is_err());
        assert!(parse_action("set-ann Str").is_err());
        assert!(parse_action("set-ann (Num").is_err());
        assert!(parse_action("set-ann Num ->").is_err());
        assert!(parse_action("set-ann Num Num").is_err());
        assert!(parse_action("set-binder-id nine").is_err());
    }

    #[test]
    fn negative_numbers_are_accepted_but_negative_indices_are_not() {
        assert_eq!(parse_action("construct-num -7").unwrap(), Action::ConstructNum(-7));
        assert!(parse_action("construct-var -1").is_err());
    }

    #[test]
    fn operator_and_side_aliases_work() {
        assert_eq!(
            parse_action("construct-binop +").unwrap(),
            Action::ConstructBinOp(Op::Add)
        );
        assert_eq!(
            parse_action("construct-binop ==").unwrap(),
            Action::ConstructBinOp(Op::Eq)
        );
        assert_eq!(
            parse_action("construct-proj fst").unwrap(),
            Action::ConstructProj(Side::L)
        );
        assert_eq!(
            parse_action("construct-proj snd").unwrap(),
            Action::ConstructProj(Side::R)
        );
        assert_eq!(
            parse_action("move-next").unwrap(),
            Action::MoveNextSibling
        );
        assert_eq!(
            parse_action("move-prev").unwrap(),
            Action::MovePrevSibling
        );
    }

    #[test]
    fn the_type_grammar_binds_as_documented() {
        assert_eq!(parse_ty("Num").unwrap(), Ty::Num);
        assert_eq!(parse_ty("num").unwrap(), Ty::Num);
        assert_eq!(parse_ty("BOOL").unwrap(), Ty::Bool);
        assert_eq!(parse_ty("?").unwrap(), Ty::Hole);
        assert_eq!(parse_ty("hole").unwrap(), Ty::Hole);

        assert_eq!(
            parse_ty("Num * Num -> Num").unwrap(),
            Ty::Arrow(
                Box::new(Ty::Prod(Box::new(Ty::Num), Box::new(Ty::Num))),
                Box::new(Ty::Num)
            )
        );

        assert_eq!(
            parse_ty("Num -> Num -> Bool").unwrap(),
            Ty::Arrow(
                Box::new(Ty::Num),
                Box::new(Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Bool)))
            )
        );

        assert_eq!(
            parse_ty("(Num -> Num) -> Num").unwrap(),
            Ty::Arrow(
                Box::new(Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Num))),
                Box::new(Ty::Num)
            )
        );
        assert_eq!(
            parse_ty("Num * (Num * Num)").unwrap(),
            Ty::Prod(
                Box::new(Ty::Num),
                Box::new(Ty::Prod(Box::new(Ty::Num), Box::new(Ty::Num)))
            )
        );

        assert_eq!(parse_ty("Num->Num").unwrap(), parse_ty("Num -> Num").unwrap());
    }

    #[test]
    fn every_type_the_grammar_produces_round_trips_through_display() {
        for text in [
            "Num",
            "Bool",
            "?",
            "Num -> Num",
            "Num -> Num -> Num",
            "(Num -> Num) -> Num",
            "Num * Num",
            "Num * Num -> Bool",
            "(Num * Num) * ?",
            "? -> ? * Bool",
        ] {
            let ty = parse_ty(text).unwrap();
            let displayed = ty.to_string();
            assert_eq!(
                parse_ty(&displayed).unwrap(),
                ty,
                "`{text}` displayed as `{displayed}` and did not parse back"
            );
        }
    }

    #[test]
    fn a_script_replays_from_the_empty_program() {

        let state = replay_script("construct-num 1\nconstruct-binop add\nconstruct-num 2\n")
            .expect("script replays");
        assert_eq!(render(&state.exp()), "1 + 2");
        assert!(is_well_typed(&state.exp()));
        assert_eq!(
            parse_script("construct-num 1\nconstruct-binop add\nconstruct-num 2\n")
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn a_script_reports_the_line_of_a_bad_action() {
        let err = replay_script("construct-num 1\nfrobnicate\n").unwrap_err();
        assert_eq!(err.line, 2);
        assert!(err.message.contains("unknown action"));
    }

    #[test]
    fn a_script_reports_the_line_of_an_action_that_does_not_apply() {
        let err = replay_script("construct-num 1\nmove-child 0\n").unwrap_err();
        assert_eq!(err.line, 2);
        assert!(err.message.contains("did not apply"), "{}", err.message);
    }

    #[test]
    fn quit_ends_a_script_and_comments_do_not_count_as_actions() {
        let actions = parse_script("# build 1\nconstruct-num 1\n\nquit\nconstruct-num 2\n").unwrap();
        assert_eq!(actions, vec![Action::ConstructNum(1)]);
    }

    #[test]
    fn harness_commands_are_rejected_inside_a_script() {
        assert!(parse_script("construct-num 1\nreset\n").is_err());
    }
}