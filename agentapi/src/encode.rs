use nothing_action::act::Action;
use nothing_action::log::{AuthorId, LogEntry};
use nothing_action::script::parse_ty;
use nothing_core::exp::{Exp, HoleId, Id, Op, Side};
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;

use crate::json::Json;

pub fn op_str(op: Op) -> &'static str {
    match op {
        Op::Add => "Add",
        Op::Sub => "Sub",
        Op::Mul => "Mul",
        Op::Lt => "Lt",
        Op::Eq => "Eq",
    }
}

pub fn op_from_str(text: &str) -> Option<Op> {
    match text.to_ascii_lowercase().as_str() {
        "add" | "+" => Some(Op::Add),
        "sub" | "-" => Some(Op::Sub),
        "mul" | "*" => Some(Op::Mul),
        "lt" | "<" => Some(Op::Lt),
        "eq" | "==" => Some(Op::Eq),
        _ => None,
    }
}

pub fn side_str(side: Side) -> &'static str {
    match side {
        Side::L => "L",
        Side::R => "R",
    }
}

pub fn side_from_str(text: &str) -> Option<Side> {
    match text.to_ascii_lowercase().as_str() {
        "l" | "left" | "fst" | "0" => Some(Side::L),
        "r" | "right" | "snd" | "1" => Some(Side::R),
        _ => None,
    }
}

pub fn ty_json(ty: &Ty) -> Json {
    match ty {
        Ty::Num => Json::obj(vec![("ty", Json::str("Num"))]),
        Ty::Bool => Json::obj(vec![("ty", Json::str("Bool"))]),
        Ty::Hole => Json::obj(vec![("ty", Json::str("Hole"))]),
        Ty::Arrow(a, b) => Json::obj(vec![
            ("ty", Json::str("Arrow")),
            ("from", ty_json(a)),
            ("to", ty_json(b)),
        ]),
        Ty::Prod(a, b) => Json::obj(vec![
            ("ty", Json::str("Prod")),
            ("fst", ty_json(a)),
            ("snd", ty_json(b)),
        ]),
    }
}

pub fn ty_from_json(value: &Json) -> Result<Ty, String> {
    if let Some(text) = value.as_str() {
        return parse_ty(text).map_err(|e| e.to_string());
    }
    let tag = value
        .get("ty")
        .and_then(Json::as_str)
        .ok_or_else(|| "a type needs a `ty` tag or a string spelling".to_string())?;
    match tag {
        "Num" => Ok(Ty::Num),
        "Bool" => Ok(Ty::Bool),
        "Hole" => Ok(Ty::Hole),
        "Arrow" => {
            let from = value
                .get("from")
                .ok_or_else(|| "Arrow needs `from`".to_string())?;
            let to = value
                .get("to")
                .ok_or_else(|| "Arrow needs `to`".to_string())?;
            Ok(Ty::Arrow(
                Box::new(ty_from_json(from)?),
                Box::new(ty_from_json(to)?),
            ))
        }
        "Prod" => {
            let fst = value
                .get("fst")
                .ok_or_else(|| "Prod needs `fst`".to_string())?;
            let snd = value
                .get("snd")
                .ok_or_else(|| "Prod needs `snd`".to_string())?;
            Ok(Ty::Prod(
                Box::new(ty_from_json(fst)?),
                Box::new(ty_from_json(snd)?),
            ))
        }
        other => Err(format!("unknown type tag `{other}`")),
    }
}

pub fn exp_json(exp: &Exp, names: &NameTable) -> Json {
    let named = |id: Id| Json::str(names.display(id));
    match exp {
        Exp::Var(id) => Json::obj(vec![
            ("exp", Json::str("Var")),
            ("id", Json::str(id.to_string())),
            ("name", named(*id)),
        ]),
        Exp::Lam(id, ty, body) => Json::obj(vec![
            ("exp", Json::str("Lam")),
            ("id", Json::str(id.to_string())),
            ("name", named(*id)),
            ("ann", ty_json(ty)),
            ("body", exp_json(body, names)),
        ]),
        Exp::Ap(f, a) => Json::obj(vec![
            ("exp", Json::str("Ap")),
            ("fun", exp_json(f, names)),
            ("arg", exp_json(a, names)),
        ]),
        Exp::Num(n) => Json::obj(vec![("exp", Json::str("Num")), ("value", Json::Int(*n))]),
        Exp::Bool(b) => Json::obj(vec![("exp", Json::str("Bool")), ("value", Json::Bool(*b))]),
        Exp::BinOp(op, l, r) => Json::obj(vec![
            ("exp", Json::str("BinOp")),
            ("op", Json::str(op_str(*op))),
            ("lhs", exp_json(l, names)),
            ("rhs", exp_json(r, names)),
        ]),
        Exp::If(c, t, e) => Json::obj(vec![
            ("exp", Json::str("If")),
            ("cond", exp_json(c, names)),
            ("then", exp_json(t, names)),
            ("else", exp_json(e, names)),
        ]),
        Exp::Let(id, bound, body) => Json::obj(vec![
            ("exp", Json::str("Let")),
            ("id", Json::str(id.to_string())),
            ("name", named(*id)),
            ("bound", exp_json(bound, names)),
            ("body", exp_json(body, names)),
        ]),
        Exp::Pair(l, r) => Json::obj(vec![
            ("exp", Json::str("Pair")),
            ("fst", exp_json(l, names)),
            ("snd", exp_json(r, names)),
        ]),
        Exp::Proj(side, e) => Json::obj(vec![
            ("exp", Json::str("Proj")),
            ("side", Json::str(side_str(*side))),
            ("body", exp_json(e, names)),
        ]),
        Exp::EmptyHole(h) => Json::obj(vec![
            ("exp", Json::str("EmptyHole")),
            ("hole", Json::str(h.to_string())),
        ]),
        Exp::NonEmptyHole(h, inner) => Json::obj(vec![
            ("exp", Json::str("NonEmptyHole")),
            ("hole", Json::str(h.to_string())),
            ("inner", exp_json(inner, names)),
        ]),
    }
}

pub fn exp_kind(exp: &Exp) -> &'static str {
    match exp {
        Exp::Var(_) => "Var",
        Exp::Lam(..) => "Lam",
        Exp::Ap(..) => "Ap",
        Exp::Num(_) => "Num",
        Exp::Bool(_) => "Bool",
        Exp::BinOp(..) => "BinOp",
        Exp::If(..) => "If",
        Exp::Let(..) => "Let",
        Exp::Pair(..) => "Pair",
        Exp::Proj(..) => "Proj",
        Exp::EmptyHole(_) => "EmptyHole",
        Exp::NonEmptyHole(..) => "NonEmptyHole",
    }
}

pub fn action_json(action: &Action) -> Json {
    match action {
        Action::MoveChild(n) => Json::obj(vec![
            ("action", Json::str("MoveChild")),
            ("n", Json::Int(*n as i64)),
        ]),
        Action::MoveParent => Json::obj(vec![("action", Json::str("MoveParent"))]),
        Action::MoveNextSibling => Json::obj(vec![("action", Json::str("MoveNextSibling"))]),
        Action::MovePrevSibling => Json::obj(vec![("action", Json::str("MovePrevSibling"))]),
        Action::Delete => Json::obj(vec![("action", Json::str("Delete"))]),
        Action::ConstructNum(n) => Json::obj(vec![
            ("action", Json::str("ConstructNum")),
            ("value", Json::Int(*n)),
        ]),
        Action::ConstructBool(b) => Json::obj(vec![
            ("action", Json::str("ConstructBool")),
            ("value", Json::Bool(*b)),
        ]),
        Action::ConstructVar(id) => Json::obj(vec![
            ("action", Json::str("ConstructVar")),
            ("id", Json::str(id.to_string())),
        ]),
        Action::ConstructLam => Json::obj(vec![("action", Json::str("ConstructLam"))]),
        Action::ConstructAp => Json::obj(vec![("action", Json::str("ConstructAp"))]),
        Action::ConstructBinOp(op) => Json::obj(vec![
            ("action", Json::str("ConstructBinOp")),
            ("op", Json::str(op_str(*op))),
        ]),
        Action::ConstructIf => Json::obj(vec![("action", Json::str("ConstructIf"))]),
        Action::ConstructLet => Json::obj(vec![("action", Json::str("ConstructLet"))]),
        Action::ConstructPair => Json::obj(vec![("action", Json::str("ConstructPair"))]),
        Action::ConstructProj(side) => Json::obj(vec![
            ("action", Json::str("ConstructProj")),
            ("side", Json::str(side_str(*side))),
        ]),
        Action::ConstructNonEmptyHole => {
            Json::obj(vec![("action", Json::str("ConstructNonEmptyHole"))])
        }
        Action::SetAnn(ty) => Json::obj(vec![("action", Json::str("SetAnn")), ("ty", ty_json(ty))]),
        Action::SetBinderId(id) => Json::obj(vec![
            ("action", Json::str("SetBinderId")),
            ("id", Json::str(id.to_string())),
        ]),
        Action::Rename(id, name) => Json::obj(vec![
            ("action", Json::str("Rename")),
            ("id", Json::str(id.to_string())),
            ("name", Json::str(name.clone())),
        ]),
        Action::Finish => Json::obj(vec![("action", Json::str("Finish"))]),
        Action::CreateDefinition => Json::obj(vec![("action", Json::str("CreateDefinition"))]),
        Action::DeleteDefinition => Json::obj(vec![("action", Json::str("DeleteDefinition"))]),
        Action::SetDefAnn(ty) => Json::obj(vec![
            ("action", Json::str("SetDefAnn")),
            ("ty", ty_json(ty)),
        ]),
        Action::MoveNextDef => Json::obj(vec![("action", Json::str("MoveNextDef"))]),
        Action::MovePrevDef => Json::obj(vec![("action", Json::str("MovePrevDef"))]),
        Action::MoveToDef(id) => Json::obj(vec![
            ("action", Json::str("MoveToDef")),
            ("id", Json::str(id.to_string())),
        ]),
    }
}

fn field<'a>(value: &'a Json, key: &str, tag: &str) -> Result<&'a Json, String> {
    value
        .get(key)
        .ok_or_else(|| format!("`{tag}` needs a `{key}` field"))
}

fn id_field(value: &Json, key: &str, tag: &str) -> Result<Id, String> {
    let text = field(value, key, tag)?
        .as_str()
        .ok_or_else(|| format!("`{tag}`'s `{key}` must be a uuid string"))?;
    Id::parse(text).ok_or_else(|| format!("`{text}` is not a uuid"))
}

pub fn action_from_json(value: &Json) -> Result<Action, String> {
    let tag = value
        .get("action")
        .and_then(Json::as_str)
        .ok_or_else(|| "an action needs an `action` tag".to_string())?;
    match tag {
        "MoveChild" => {
            let n = field(value, "n", tag)?
                .as_usize()
                .ok_or_else(|| "`MoveChild` needs a non-negative `n`".to_string())?;
            Ok(Action::MoveChild(n))
        }
        "MoveParent" => Ok(Action::MoveParent),
        "MoveNextSibling" => Ok(Action::MoveNextSibling),
        "MovePrevSibling" => Ok(Action::MovePrevSibling),
        "Delete" => Ok(Action::Delete),
        "ConstructNum" => {
            let n = field(value, "value", tag)?
                .as_i64()
                .ok_or_else(|| "`ConstructNum` needs an integer `value`".to_string())?;
            Ok(Action::ConstructNum(n))
        }
        "ConstructBool" => {
            let b = field(value, "value", tag)?
                .as_bool()
                .ok_or_else(|| "`ConstructBool` needs a boolean `value`".to_string())?;
            Ok(Action::ConstructBool(b))
        }
        "ConstructVar" => Ok(Action::ConstructVar(id_field(value, "id", tag)?)),
        "ConstructLam" => Ok(Action::ConstructLam),
        "ConstructAp" => Ok(Action::ConstructAp),
        "ConstructBinOp" => {
            let text = field(value, "op", tag)?
                .as_str()
                .ok_or_else(|| "`ConstructBinOp` needs a string `op`".to_string())?;
            op_from_str(text)
                .map(Action::ConstructBinOp)
                .ok_or_else(|| format!("unknown operator `{text}`"))
        }
        "ConstructIf" => Ok(Action::ConstructIf),
        "ConstructLet" => Ok(Action::ConstructLet),
        "ConstructPair" => Ok(Action::ConstructPair),
        "ConstructProj" => {
            let text = field(value, "side", tag)?
                .as_str()
                .ok_or_else(|| "`ConstructProj` needs a string `side`".to_string())?;
            side_from_str(text)
                .map(Action::ConstructProj)
                .ok_or_else(|| format!("unknown projection side `{text}`"))
        }
        "ConstructNonEmptyHole" => Ok(Action::ConstructNonEmptyHole),
        "SetAnn" => Ok(Action::SetAnn(ty_from_json(field(value, "ty", tag)?)?)),
        "SetBinderId" => Ok(Action::SetBinderId(id_field(value, "id", tag)?)),
        "Rename" => {
            let name = field(value, "name", tag)?
                .as_str()
                .ok_or_else(|| "`Rename` needs a string `name`".to_string())?;
            Ok(Action::Rename(
                id_field(value, "id", tag)?,
                name.to_string(),
            ))
        }
        "Finish" => Ok(Action::Finish),
        "CreateDefinition" => Ok(Action::CreateDefinition),
        "DeleteDefinition" => Ok(Action::DeleteDefinition),
        "SetDefAnn" => Ok(Action::SetDefAnn(ty_from_json(field(value, "ty", tag)?)?)),
        "MoveNextDef" => Ok(Action::MoveNextDef),
        "MovePrevDef" => Ok(Action::MovePrevDef),
        "MoveToDef" => Ok(Action::MoveToDef(id_field(value, "id", tag)?)),
        other => Err(format!("unknown action `{other}`")),
    }
}

pub fn author_json(author: AuthorId) -> Json {
    Json::Int(author.0 as i64)
}

pub fn entry_json(entry: &LogEntry) -> Json {
    Json::obj(vec![
        ("action", action_json(&entry.action)),
        (
            "step",
            Json::str(nothing_action::script::action_name(&entry.action)),
        ),
        ("timestamp", Json::Int(entry.timestamp as i64)),
        ("author", author_json(entry.author)),
    ])
}

pub fn names_json(names: &NameTable) -> Json {
    let mut entries = names.flatten().entries();
    entries.sort_by_key(|(id, _)| id.as_u128());
    Json::arr(
        entries
            .into_iter()
            .map(|(id, name)| {
                Json::obj(vec![
                    ("id", Json::str(id.to_string())),
                    ("name", Json::str(name)),
                ])
            })
            .collect(),
    )
}

pub fn holes(exp: &Exp) -> (usize, usize) {
    fn go(exp: &Exp, empty: &mut usize, non_empty: &mut usize) {
        match exp {
            Exp::EmptyHole(_) => *empty += 1,
            Exp::NonEmptyHole(_, inner) => {
                *non_empty += 1;
                go(inner, empty, non_empty);
            }
            Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) => {}
            Exp::Lam(_, _, b) | Exp::Proj(_, b) => go(b, empty, non_empty),
            Exp::Ap(a, b) | Exp::BinOp(_, a, b) | Exp::Let(_, a, b) | Exp::Pair(a, b) => {
                go(a, empty, non_empty);
                go(b, empty, non_empty);
            }
            Exp::If(c, t, e) => {
                go(c, empty, non_empty);
                go(t, empty, non_empty);
                go(e, empty, non_empty);
            }
        }
    }
    let mut empty = 0;
    let mut non_empty = 0;
    go(exp, &mut empty, &mut non_empty);
    (empty, non_empty)
}

pub fn hole_ids(exp: &Exp) -> Vec<HoleId> {
    fn go(exp: &Exp, out: &mut Vec<HoleId>) {
        match exp {
            Exp::EmptyHole(h) => out.push(*h),
            Exp::NonEmptyHole(h, inner) => {
                out.push(*h);
                go(inner, out);
            }
            Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) => {}
            Exp::Lam(_, _, b) | Exp::Proj(_, b) => go(b, out),
            Exp::Ap(a, b) | Exp::BinOp(_, a, b) | Exp::Let(_, a, b) | Exp::Pair(a, b) => {
                go(a, out);
                go(b, out);
            }
            Exp::If(c, t, e) => {
                go(c, out);
                go(t, out);
                go(e, out);
            }
        }
    }
    let mut out = Vec::new();
    go(exp, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse;
    use nothing_core::examples;

    fn every_action() -> Vec<Action> {
        vec![
            Action::MoveChild(0),
            Action::MoveChild(2),
            Action::MoveParent,
            Action::MoveNextSibling,
            Action::MovePrevSibling,
            Action::Delete,
            Action::ConstructNum(-7),
            Action::ConstructBool(true),
            Action::ConstructBool(false),
            Action::ConstructVar(Id::from_u128(3)),
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
            Action::SetAnn(Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Bool))),
            Action::SetAnn(Ty::Prod(Box::new(Ty::Hole), Box::new(Ty::Num))),
            Action::SetBinderId(Id::from_u128(11)),
            Action::Rename(Id::from_u128(11), "total".to_string()),
            Action::Finish,
        ]
    }

    #[test]
    fn every_action_round_trips_through_json() {
        for action in every_action() {
            let text = action_json(&action).to_string();
            let parsed = parse(&text).unwrap();
            assert_eq!(action_from_json(&parsed).unwrap(), action, "{text}");
        }
    }

    #[test]
    fn a_type_may_be_given_as_a_string() {
        let value = Json::obj(vec![
            ("action", Json::str("SetAnn")),
            ("ty", Json::str("Num -> Bool")),
        ]);
        assert_eq!(
            action_from_json(&value).unwrap(),
            Action::SetAnn(Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Bool)))
        );
    }

    #[test]
    fn malformed_actions_are_errors_not_panics() {
        for text in [
            r#"{}"#,
            r#"{"action":"Frobnicate"}"#,
            r#"{"action":"MoveChild"}"#,
            r#"{"action":"MoveChild","n":-1}"#,
            r#"{"action":"ConstructNum"}"#,
            r#"{"action":"ConstructVar","id":"nine"}"#,
            r#"{"action":"ConstructBinOp","op":"pow"}"#,
            r#"{"action":"SetAnn","ty":"Str"}"#,
        ] {
            let value = parse(text).unwrap();
            assert!(action_from_json(&value).is_err(), "{text}");
        }
    }

    #[test]
    fn an_expression_encodes_with_its_display_names() {
        let json = exp_json(&examples::let_identity(), &examples::names()).to_string();
        assert!(json.contains("\"name\":\"x0\""), "{json}");
        assert!(json.contains("\"exp\":\"Let\""), "{json}");
        assert_eq!(
            parse(&json).unwrap().get("exp").unwrap().as_str(),
            Some("Let")
        );
    }

    #[test]
    fn holes_are_counted_by_kind() {
        assert_eq!(holes(&examples::add_with_empty_hole()), (1, 0));
        assert_eq!(holes(&examples::add_with_non_empty_hole()), (0, 1));
        assert_eq!(holes(&examples::let_identity()), (0, 0));
    }
}
