use nothing_action::act::Action;
use nothing_action::log::ActionLog;
use nothing_core::exp::{Exp, Op, Side};
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;

use crate::document::Document;

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn ty_json(ty: &Ty) -> String {
    match ty {
        Ty::Num => "{\"ty\":\"Num\"}".to_string(),
        Ty::Bool => "{\"ty\":\"Bool\"}".to_string(),
        Ty::Hole => "{\"ty\":\"Hole\"}".to_string(),
        Ty::Arrow(a, b) => format!(
            "{{\"ty\":\"Arrow\",\"from\":{},\"to\":{}}}",
            ty_json(a),
            ty_json(b)
        ),
        Ty::Prod(a, b) => format!(
            "{{\"ty\":\"Prod\",\"fst\":{},\"snd\":{}}}",
            ty_json(a),
            ty_json(b)
        ),
    }
}

fn op_str(op: Op) -> &'static str {
    match op {
        Op::Add => "Add",
        Op::Sub => "Sub",
        Op::Mul => "Mul",
        Op::Lt => "Lt",
        Op::Eq => "Eq",
    }
}

fn side_str(side: Side) -> &'static str {
    match side {
        Side::L => "L",
        Side::R => "R",
    }
}

fn exp_json(exp: &Exp) -> String {
    match exp {
        Exp::Var(id) => format!("{{\"exp\":\"Var\",\"id\":{}}}", escape(&id.to_string())),
        Exp::Lam(id, ty, body) => format!(
            "{{\"exp\":\"Lam\",\"id\":{},\"ty\":{},\"body\":{}}}",
            escape(&id.to_string()),
            ty_json(ty),
            exp_json(body)
        ),
        Exp::Ap(f, a) => format!(
            "{{\"exp\":\"Ap\",\"fun\":{},\"arg\":{}}}",
            exp_json(f),
            exp_json(a)
        ),
        Exp::Num(n) => format!("{{\"exp\":\"Num\",\"value\":{n}}}"),
        Exp::Bool(b) => format!("{{\"exp\":\"Bool\",\"value\":{b}}}"),
        Exp::BinOp(op, l, r) => format!(
            "{{\"exp\":\"BinOp\",\"op\":{},\"lhs\":{},\"rhs\":{}}}",
            escape(op_str(*op)),
            exp_json(l),
            exp_json(r)
        ),
        Exp::If(c, t, e) => format!(
            "{{\"exp\":\"If\",\"cond\":{},\"then\":{},\"else\":{}}}",
            exp_json(c),
            exp_json(t),
            exp_json(e)
        ),
        Exp::Let(id, bound, body) => format!(
            "{{\"exp\":\"Let\",\"id\":{},\"bound\":{},\"body\":{}}}",
            escape(&id.to_string()),
            exp_json(bound),
            exp_json(body)
        ),
        Exp::Pair(l, r) => format!(
            "{{\"exp\":\"Pair\",\"fst\":{},\"snd\":{}}}",
            exp_json(l),
            exp_json(r)
        ),
        Exp::Proj(side, e) => format!(
            "{{\"exp\":\"Proj\",\"side\":{},\"body\":{}}}",
            escape(side_str(*side)),
            exp_json(e)
        ),
        Exp::EmptyHole(h) => format!(
            "{{\"exp\":\"EmptyHole\",\"hole\":{}}}",
            escape(&h.to_string())
        ),
        Exp::NonEmptyHole(h, inner) => format!(
            "{{\"exp\":\"NonEmptyHole\",\"hole\":{},\"inner\":{}}}",
            escape(&h.to_string()),
            exp_json(inner)
        ),
    }
}

fn names_json(names: &NameTable) -> String {
    let mut entries = names.flatten().entries();
    entries.sort_by_key(|(id, _)| id.as_u128());
    let items: Vec<String> = entries
        .iter()
        .map(|(id, name)| format!("{{\"id\":{},\"name\":{}}}", escape(&id.to_string()), escape(name)))
        .collect();
    format!("[{}]", items.join(","))
}

fn action_json(action: &Action) -> String {
    match action {
        Action::MoveChild(n) => format!("{{\"action\":\"MoveChild\",\"n\":{n}}}"),
        Action::MoveParent => "{\"action\":\"MoveParent\"}".to_string(),
        Action::MoveNextSibling => "{\"action\":\"MoveNextSibling\"}".to_string(),
        Action::MovePrevSibling => "{\"action\":\"MovePrevSibling\"}".to_string(),
        Action::Delete => "{\"action\":\"Delete\"}".to_string(),
        Action::ConstructNum(n) => format!("{{\"action\":\"ConstructNum\",\"value\":{n}}}"),
        Action::ConstructBool(b) => format!("{{\"action\":\"ConstructBool\",\"value\":{b}}}"),
        Action::ConstructVar(id) => format!(
            "{{\"action\":\"ConstructVar\",\"id\":{}}}",
            escape(&id.to_string())
        ),
        Action::ConstructLam => "{\"action\":\"ConstructLam\"}".to_string(),
        Action::ConstructAp => "{\"action\":\"ConstructAp\"}".to_string(),
        Action::ConstructBinOp(op) => format!(
            "{{\"action\":\"ConstructBinOp\",\"op\":{}}}",
            escape(op_str(*op))
        ),
        Action::ConstructIf => "{\"action\":\"ConstructIf\"}".to_string(),
        Action::ConstructLet => "{\"action\":\"ConstructLet\"}".to_string(),
        Action::ConstructPair => "{\"action\":\"ConstructPair\"}".to_string(),
        Action::ConstructProj(side) => format!(
            "{{\"action\":\"ConstructProj\",\"side\":{}}}",
            escape(side_str(*side))
        ),
        Action::ConstructNonEmptyHole => "{\"action\":\"ConstructNonEmptyHole\"}".to_string(),
        Action::SetAnn(ty) => format!("{{\"action\":\"SetAnn\",\"ty\":{}}}", ty_json(ty)),
        Action::SetBinderId(id) => format!(
            "{{\"action\":\"SetBinderId\",\"id\":{}}}",
            escape(&id.to_string())
        ),
        Action::Rename(id, name) => format!(
            "{{\"action\":\"Rename\",\"id\":{},\"name\":{}}}",
            escape(&id.to_string()),
            escape(name)
        ),
        Action::Finish => "{\"action\":\"Finish\"}".to_string(),
    }
}

fn log_json(log: &ActionLog) -> String {
    let items: Vec<String> = log
        .entries()
        .iter()
        .map(|entry| {
            format!(
                "{{\"timestamp\":{},\"author\":{},\"action\":{}}}",
                entry.timestamp,
                entry.author.0,
                action_json(&entry.action)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

pub fn to_debug_json(doc: &Document) -> String {
    format!(
        "{{\"exp\":{},\"names\":{},\"log\":{}}}",
        exp_json(&doc.exp),
        names_json(&doc.names),
        log_json(&doc.log)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::examples;

    #[test]
    fn debug_json_is_well_formed_enough_to_have_balanced_braces() {
        let doc = Document::new(examples::square_and_compare(), examples::names(), ActionLog::new());
        let json = to_debug_json(&doc);
        let opens = json.matches('{').count();
        let closes = json.matches('}').count();
        assert_eq!(opens, closes);
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
    }
}
