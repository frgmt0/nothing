use std::fmt;

use nothing_core::exp::{Id, Op, Side};
use nothing_core::ty::Ty;

use crate::act::{Action, EditState};

#[derive(Clone, PartialEq, Debug)]
pub enum Step {
    Act(Action),
    Var(String),
    Rename(String),
    RenameDef(String),
    Def(String),
}

impl Step {
    pub fn resolve(&self, state: &EditState) -> Result<Action, ParseError> {
        match self {
            Step::Act(action) => Ok(action.clone()),
            Step::Var(name) => match lookup_in_scope(state, name) {
                Some(id) => Ok(Action::ConstructVar(id)),
                None => Err(ParseError(format!("no binder named `{name}` is in scope"))),
            },
            Step::Rename(name) => match state.zipper.binder_id() {
                Some(id) => Ok(Action::Rename(id, name.clone())),
                None => Err(ParseError(
                    "`rename` needs the cursor on a lambda or a let".to_string(),
                )),
            },
            Step::RenameDef(name) => Ok(Action::Rename(state.def_id(), name.clone())),
            Step::Def(name) => match lookup_definition(state, name) {
                Some(id) => Ok(Action::MoveToDef(id)),
                None => Err(ParseError(format!("no definition named `{name}`"))),
            },
        }
    }
}

fn lookup_in_scope(state: &EditState, name: &str) -> Option<Id> {
    state
        .zipper
        .binders()
        .into_iter()
        .rev()
        .find(|id| state.names.get(*id) == Some(name))
        .or_else(|| lookup_definition(state, name))
}

fn lookup_definition(state: &EditState, name: &str) -> Option<Id> {
    state
        .definition_ids()
        .into_iter()
        .find(|id| state.names.get(*id) == Some(name))
}

#[derive(Clone, PartialEq, Debug)]
pub enum Command {
    Act(Step),
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
  construct-var NAME      reference the in-scope binder or definition NAME
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
  set-binder-id UUID      re-identify the focused lambda or let binder
  rename NAME             give the focused binder the display name NAME
  finish                  unwrap a non-empty hole whose contents now fit
definitions:
  create-definition       add a definition after this one and move to it
  delete-definition       remove this definition; references to it become holes
  set-def-ann TY          set this definition's type annotation
  move-next-def           move the cursor to the next definition
  move-prev-def           move the cursor to the previous definition
  move-to-def NAME        move the cursor to the definition displayed as NAME
  rename-def NAME         give this definition the display name NAME
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
    parse_step(line).map(|step| Some(Command::Act(step)))
}

pub fn parse_step(line: &str) -> Result<Step, ParseError> {
    let line = line.trim();
    let (head, rest) = match line.find(char::is_whitespace) {
        Some(i) => (&line[..i], line[i..].trim()),
        None => (line, ""),
    };

    let no_arg = |action: Action| -> Result<Step, ParseError> {
        if rest.is_empty() {
            Ok(Step::Act(action))
        } else {
            Err(ParseError(format!(
                "`{head}` takes no argument, got `{rest}`"
            )))
        }
    };

    let act = |action: Action| -> Result<Step, ParseError> { Ok(Step::Act(action)) };

    match head {
        "move-child" => act(Action::MoveChild(parse_usize(head, rest)?)),
        "move-parent" => no_arg(Action::MoveParent),
        "move-next-sibling" | "move-next" => no_arg(Action::MoveNextSibling),
        "move-prev-sibling" | "move-prev" => no_arg(Action::MovePrevSibling),

        "delete" => no_arg(Action::Delete),

        "construct-num" => act(Action::ConstructNum(parse_i64(head, rest)?)),
        "construct-bool" => act(Action::ConstructBool(parse_bool(head, rest)?)),
        "construct-var" => Ok(Step::Var(parse_name(head, rest)?)),
        "construct-lam" => no_arg(Action::ConstructLam),
        "construct-ap" => no_arg(Action::ConstructAp),
        "construct-binop" => act(Action::ConstructBinOp(parse_op(rest)?)),
        "construct-if" => no_arg(Action::ConstructIf),
        "construct-let" => no_arg(Action::ConstructLet),
        "construct-pair" => no_arg(Action::ConstructPair),
        "construct-proj" => act(Action::ConstructProj(parse_side(rest)?)),
        "construct-non-empty-hole" => no_arg(Action::ConstructNonEmptyHole),

        "set-ann" => act(Action::SetAnn(parse_ty(rest)?)),
        "set-binder-id" => act(Action::SetBinderId(parse_id(head, rest)?)),
        "rename" => Ok(Step::Rename(parse_name(head, rest)?)),
        "finish" => no_arg(Action::Finish),

        "create-definition" => no_arg(Action::CreateDefinition),
        "delete-definition" => no_arg(Action::DeleteDefinition),
        "set-def-ann" => act(Action::SetDefAnn(parse_ty(rest)?)),
        "move-next-def" => no_arg(Action::MoveNextDef),
        "move-prev-def" => no_arg(Action::MovePrevDef),
        "move-to-def" => Ok(Step::Def(parse_name(head, rest)?)),
        "rename-def" => Ok(Step::RenameDef(parse_name(head, rest)?)),

        "" => Err(ParseError("empty command".to_string())),
        other => Err(ParseError(format!("unknown action `{other}`"))),
    }
}

pub fn step_name(step: &Step) -> String {
    match step {
        Step::Act(action) => action_name(action),
        Step::Var(name) => format!("construct-var {name}"),
        Step::Rename(name) => format!("rename {name}"),
        Step::RenameDef(name) => format!("rename-def {name}"),
        Step::Def(name) => format!("move-to-def {name}"),
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
        Action::ConstructVar(id) => format!("construct-var {id}"),
        Action::ConstructLam => "construct-lam".to_string(),
        Action::ConstructAp => "construct-ap".to_string(),
        Action::ConstructBinOp(op) => format!("construct-binop {}", op_name(*op)),
        Action::ConstructIf => "construct-if".to_string(),
        Action::ConstructLet => "construct-let".to_string(),
        Action::ConstructPair => "construct-pair".to_string(),
        Action::ConstructProj(side) => format!("construct-proj {}", side_name(*side)),
        Action::ConstructNonEmptyHole => "construct-non-empty-hole".to_string(),
        Action::SetAnn(ty) => format!("set-ann {ty}"),
        Action::SetBinderId(id) => format!("set-binder-id {id}"),
        Action::Rename(id, name) => format!("rename {name} {id}"),
        Action::Finish => "finish".to_string(),
        Action::CreateDefinition => "create-definition".to_string(),
        Action::DeleteDefinition => "delete-definition".to_string(),
        Action::SetDefAnn(ty) => format!("set-def-ann {ty}"),
        Action::MoveNextDef => "move-next-def".to_string(),
        Action::MovePrevDef => "move-prev-def".to_string(),
        Action::MoveToDef(id) => format!("move-to-def {id}"),
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

fn parse_id(head: &str, rest: &str) -> Result<Id, ParseError> {
    Id::parse(rest)
        .ok_or_else(|| ParseError(format!("`{head}` expects a binder uuid, got `{rest}`")))
}

fn parse_name(head: &str, rest: &str) -> Result<String, ParseError> {
    let name = rest.split_whitespace().next().unwrap_or("");
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(ParseError(format!("`{head}` expects a name, got `{rest}`")));
    }
    Ok(name.to_string())
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

pub fn parse_script(text: &str) -> Result<Vec<Step>, ScriptError> {
    Ok(parse_numbered_script(text)?
        .into_iter()
        .map(|(_, step)| step)
        .collect())
}

pub fn parse_numbered_script(text: &str) -> Result<Vec<(usize, Step)>, ScriptError> {
    let mut actions = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let at = |message: String| ScriptError {
            line: i + 1,
            text: line.to_string(),
            message,
        };
        match parse_command(line).map_err(|e| at(e.0))? {
            None => {}
            Some(Command::Act(step)) => actions.push((i + 1, step)),
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
    replay_script_from(text, EditState::empty())
}

pub fn replay_script_from(text: &str, start: EditState) -> Result<EditState, ScriptError> {
    let lines: Vec<&str> = text.lines().collect();
    let mut state = start;
    for (line_no, step) in parse_numbered_script(text)? {
        let at = |message: String| ScriptError {
            line: line_no,
            text: lines[line_no - 1].to_string(),
            message,
        };
        let action = step.resolve(&state).map_err(|e| at(e.0))?;
        if !state.apply_mut(action) {
            return Err(at(format!("action did not apply: {}", step_name(&step))));
        }
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::typing::is_well_typed;

    fn every_step() -> Vec<Step> {
        let mut steps: Vec<Step> = vec![
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
            Action::SetBinderId(Id::from_u128(9)),
            Action::Rename(Id::from_u128(9), "xs".to_string()),
            Action::Finish,
        ]
        .into_iter()
        .map(Step::Act)
        .collect();
        steps.push(Step::Var("x0".to_string()));
        steps.push(Step::Rename("total".to_string()));
        steps
    }

    #[test]
    fn step_name_and_parse_step_are_inverse() {
        for step in every_step() {
            if matches!(step, Step::Act(Action::Rename(..))) {
                continue;
            }
            let name = step_name(&step);
            let parsed =
                parse_step(&name).unwrap_or_else(|e| panic!("`{name}` did not parse back: {e}"));
            assert_eq!(parsed, step, "round trip failed for `{name}`");
        }
    }

    #[test]
    fn a_rename_step_resolves_against_the_focused_binder() {
        let state = replay_script("construct-lam\nmove-parent\nrename total\n").expect("replays");
        assert_eq!(state.render(), "λtotal:?. ⦇⦈");
    }

    #[test]
    fn construct_var_names_the_binder_rather_than_its_identity() {
        let state = replay_script(
            "construct-lam\nmove-parent\nrename n\nset-ann Num\nmove-child 0\nconstruct-var n\n",
        )
        .expect("replays");
        assert_eq!(state.render(), "λn:Num. n");

        let err = replay_script("construct-lam\nconstruct-var nope\n").unwrap_err();
        assert_eq!(err.line, 2);
        assert!(err.message.contains("no binder named"), "{}", err.message);
    }

    #[test]
    fn an_inner_binder_wins_a_shared_display_name() {
        let state = replay_script(
            "construct-lam\nmove-parent\nrename x\nset-ann Num\nmove-child 0\n\
             construct-lam\nmove-parent\nrename x\nset-ann Bool\nmove-child 0\n\
             construct-var x\n",
        )
        .expect("replays");
        assert_eq!(state.render(), "λx:Num. λx:Bool. x");
        let inner = match state.exp() {
            nothing_core::exp::Exp::Lam(_, _, body) => match *body {
                nothing_core::exp::Exp::Lam(id, _, body) => match *body {
                    nothing_core::exp::Exp::Var(used) => (id, used),
                    other => panic!("expected a variable, got {other:?}"),
                },
                other => panic!("expected a lambda, got {other:?}"),
            },
            other => panic!("expected a lambda, got {other:?}"),
        };
        assert_eq!(inner.0, inner.1, "`x` resolved to the innermost binder");
    }

    #[test]
    fn every_action_is_documented_in_the_help_text() {
        let documented: Vec<&str> = HELP
            .lines()
            .filter_map(|line| line.strip_prefix("  "))
            .filter(|line| !line.starts_with(' '))
            .filter_map(|line| line.split_whitespace().next())
            .collect();
        for step in every_step() {
            let name = step_name(&step);
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
        assert!(parse_step("frobnicate").is_err());
        assert!(parse_step("move-child").is_err());
        assert!(parse_step("move-child x").is_err());
        assert!(parse_step("move-child -1").is_err());
        assert!(parse_step("move-parent 3").is_err());
        assert!(parse_step("construct-num").is_err());
        assert!(parse_step("construct-num 1.5").is_err());
        assert!(parse_step("construct-bool yes").is_err());
        assert!(parse_step("construct-binop pow").is_err());
        assert!(parse_step("construct-proj middle").is_err());
        assert!(parse_step("set-ann").is_err());
        assert!(parse_step("set-ann Str").is_err());
        assert!(parse_step("set-ann (Num").is_err());
        assert!(parse_step("set-ann Num ->").is_err());
        assert!(parse_step("set-ann Num Num").is_err());
        assert!(parse_step("set-binder-id nine").is_err());
    }

    #[test]
    fn negative_numbers_are_accepted_but_negative_indices_are_not() {
        assert_eq!(
            parse_step("construct-num -7").unwrap(),
            Step::Act(Action::ConstructNum(-7))
        );
        assert!(parse_step("construct-var -").is_err());
    }

    #[test]
    fn operator_and_side_aliases_work() {
        assert_eq!(
            parse_step("construct-binop +").unwrap(),
            Step::Act(Action::ConstructBinOp(Op::Add))
        );
        assert_eq!(
            parse_step("construct-binop ==").unwrap(),
            Step::Act(Action::ConstructBinOp(Op::Eq))
        );
        assert_eq!(
            parse_step("construct-proj fst").unwrap(),
            Step::Act(Action::ConstructProj(Side::L))
        );
        assert_eq!(
            parse_step("construct-proj snd").unwrap(),
            Step::Act(Action::ConstructProj(Side::R))
        );
        assert_eq!(
            parse_step("move-next").unwrap(),
            Step::Act(Action::MoveNextSibling)
        );
        assert_eq!(
            parse_step("move-prev").unwrap(),
            Step::Act(Action::MovePrevSibling)
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

        assert_eq!(
            parse_ty("Num->Num").unwrap(),
            parse_ty("Num -> Num").unwrap()
        );
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
        assert_eq!(state.render(), "1 + 2");
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
        let actions =
            parse_script("# build 1\nconstruct-num 1\n\nquit\nconstruct-num 2\n").unwrap();
        assert_eq!(actions, vec![Step::Act(Action::ConstructNum(1))]);
    }

    #[test]
    fn harness_commands_are_rejected_inside_a_script() {
        assert!(parse_script("construct-num 1\nreset\n").is_err());
    }
}
