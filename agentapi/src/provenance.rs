use std::collections::HashMap;

use nothing_action::act::{Action, EditState};
use nothing_action::log::{AuthorId, LogEntry};
use nothing_core::doc::Doc;
use nothing_core::exp::{Exp, Id, Side};
use nothing_core::names::NameTable;
use nothing_core::render::{
    PREC_APP, PREC_ATOM, PREC_BINDER, PREC_CMP, Prec, op_prec, op_str, render_id,
};
use nothing_merge::path::{Path, arity, at, child, extend};

use crate::json::Json;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NodeProvenance {
    pub author: AuthorId,
    pub timestamp: u64,
    pub entry: usize,
}

type Nodes = HashMap<Path, Option<NodeProvenance>>;

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Provenance {
    nodes: Nodes,
    per_def: HashMap<Id, Nodes>,
    names: HashMap<Id, NodeProvenance>,
}

impl Provenance {
    pub fn get(&self, path: &[usize]) -> Option<NodeProvenance> {
        self.nodes.get(path).copied().flatten()
    }

    pub fn get_in(&self, definition: Id, path: &[usize]) -> Option<NodeProvenance> {
        self.per_def
            .get(&definition)
            .and_then(|nodes| nodes.get(path))
            .copied()
            .flatten()
    }

    pub fn in_definition(&self, definition: Id) -> Provenance {
        Provenance {
            nodes: self.per_def.get(&definition).cloned().unwrap_or_default(),
            per_def: self.per_def.clone(),
            names: self.names.clone(),
        }
    }

    pub fn definitions(&self) -> Vec<Id> {
        let mut out: Vec<Id> = self.per_def.keys().copied().collect();
        out.sort();
        out
    }

    pub fn name_provenance(&self, id: Id) -> Option<NodeProvenance> {
        self.names.get(&id).copied()
    }

    pub fn paths(&self) -> Vec<Path> {
        let mut out: Vec<Path> = self.nodes.keys().cloned().collect();
        out.sort();
        out
    }

    pub fn authors(&self) -> Vec<AuthorId> {
        let mut out: Vec<AuthorId> = Vec::new();
        for value in self.nodes.values().flatten() {
            if !out.contains(&value.author) {
                out.push(value.author);
            }
        }
        out.sort();
        out
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

fn shallow_key(exp: &Exp) -> String {
    match exp {
        Exp::Var(id) => format!("Var:{id}"),
        Exp::Lam(id, ty, _) => format!("Lam:{id}:{ty}"),
        Exp::Ap(..) => "Ap".to_string(),
        Exp::Num(n) => format!("Num:{n}"),
        Exp::Bool(b) => format!("Bool:{b}"),
        Exp::BinOp(op, ..) => format!("BinOp:{}", op_str(*op)),
        Exp::If(..) => "If".to_string(),
        Exp::Let(id, ..) => format!("Let:{id}"),
        Exp::Pair(..) => "Pair".to_string(),
        Exp::Proj(side, _) => format!(
            "Proj:{}",
            match side {
                Side::L => "L",
                Side::R => "R",
            }
        ),
        Exp::EmptyHole(h) => format!("EmptyHole:{h}"),
        Exp::NonEmptyHole(h, _) => format!("NonEmptyHole:{h}"),
    }
}

fn deep_key(exp: &Exp) -> String {
    let mut out = shallow_key(exp);
    out.push('(');
    for n in 0..arity(exp) {
        if let Some(c) = child(exp, n) {
            out.push_str(&deep_key(c));
            out.push(',');
        }
    }
    out.push(')');
    out
}

fn walk(exp: &Exp, path: Path, out: &mut Vec<Path>) {
    out.push(path.clone());
    for n in 0..arity(exp) {
        if let Some(c) = child(exp, n) {
            walk(c, extend(&path, n), out);
        }
    }
}

pub fn paths_of(exp: &Exp) -> Vec<Path> {
    let mut out = Vec::new();
    walk(exp, Vec::new(), &mut out);
    out
}

fn stayed_put(old: &Exp, new: &Exp, path: &[usize]) -> bool {
    match (at(old, path), at(new, path)) {
        (Some(before), Some(after)) => shallow_key(before) == shallow_key(after),
        _ => false,
    }
}

fn displaced(old: &Exp, new: &Exp, nodes: &Nodes) -> HashMap<String, Vec<Option<NodeProvenance>>> {
    let mut index: HashMap<String, Vec<Option<NodeProvenance>>> = HashMap::new();
    for path in paths_of(old) {
        if stayed_put(old, new, &path) {
            continue;
        }
        if let Some(node) = at(old, &path) {
            index
                .entry(deep_key(node))
                .or_default()
                .push(nodes.get(&path).copied().flatten());
        }
    }
    index
}

fn carry(old: &Exp, new: &Exp, nodes: &Nodes, stamp: NodeProvenance) -> Nodes {
    let mut orphans = displaced(old, new, nodes);
    let mut next: Nodes = HashMap::new();
    let mut unresolved: Vec<Path> = Vec::new();

    for path in paths_of(new) {
        if stayed_put(old, new, &path) {
            next.insert(path.clone(), nodes.get(&path).copied().flatten());
        } else {
            unresolved.push(path);
        }
    }

    for path in unresolved {
        let Some(node) = at(new, &path) else {
            continue;
        };
        let carried = orphans
            .get_mut(&deep_key(node))
            .and_then(|available| available.pop());
        next.insert(path, carried.unwrap_or(Some(stamp)));
    }

    next
}

fn untouched(exp: &Exp) -> Nodes {
    paths_of(exp).into_iter().map(|path| (path, None)).collect()
}

fn authored(exp: &Exp, stamp: NodeProvenance) -> Nodes {
    paths_of(exp)
        .into_iter()
        .map(|path| (path, Some(stamp)))
        .collect()
}

fn bodies(state: &EditState) -> Vec<(Id, Exp)> {
    state
        .doc()
        .defs()
        .iter()
        .map(|def| (def.id, def.body.clone()))
        .collect()
}

pub fn provenance_of(base: &EditState, entries: &[LogEntry]) -> Provenance {
    let mut state = base.clone();
    let mut per_def: HashMap<Id, Nodes> = bodies(&state)
        .into_iter()
        .map(|(id, body)| (id, untouched(&body)))
        .collect();
    let mut names: HashMap<Id, NodeProvenance> = HashMap::new();

    for (index, entry) in entries.iter().enumerate() {
        let old: HashMap<Id, Exp> = bodies(&state).into_iter().collect();
        if !state.apply_mut(entry.action.clone()) {
            continue;
        }
        let stamp = NodeProvenance {
            author: entry.author,
            timestamp: entry.timestamp,
            entry: index,
        };
        if let Action::Rename(id, _) = &entry.action {
            names.insert(*id, stamp);
        }

        let mut next: HashMap<Id, Nodes> = HashMap::new();
        for (id, body) in bodies(&state) {
            let updated = match (old.get(&id), per_def.get(&id)) {
                (Some(before), Some(nodes)) => carry(before, &body, nodes, stamp),
                _ => authored(&body, stamp),
            };
            next.insert(id, updated);
        }
        per_def = next;
    }

    let nodes = per_def.get(&state.def_id()).cloned().unwrap_or_default();
    Provenance {
        nodes,
        per_def,
        names,
    }
}

pub fn provenance_json(map: &Provenance) -> Json {
    Json::arr(
        map.paths()
            .into_iter()
            .map(|path| {
                let mut fields = vec![(
                    "path",
                    Json::arr(path.iter().map(|n| Json::Int(*n as i64)).collect()),
                )];
                match map.get(&path) {
                    None => {
                        fields.push(("author", Json::Null));
                        fields.push(("timestamp", Json::Null));
                        fields.push(("entry", Json::Null));
                    }
                    Some(value) => {
                        fields.push(("author", Json::Int(value.author.0 as i64)));
                        fields.push(("timestamp", Json::Int(value.timestamp as i64)));
                        fields.push(("entry", Json::Int(value.entry as i64)));
                    }
                }
                Json::obj(fields)
            })
            .collect(),
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    Human,
    Agent,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Palette {
    pub agent_open: String,
    pub agent_close: String,
    pub human_open: String,
    pub human_close: String,
}

impl Palette {
    pub fn brackets() -> Palette {
        Palette {
            agent_open: "⟦".to_string(),
            agent_close: "⟧".to_string(),
            human_open: "⟨".to_string(),
            human_close: "⟩".to_string(),
        }
    }

    pub fn ansi() -> Palette {
        Palette {
            agent_open: "\u{1b}[35m".to_string(),
            agent_close: "\u{1b}[0m".to_string(),
            human_open: "\u{1b}[36m".to_string(),
            human_close: "\u{1b}[0m".to_string(),
        }
    }

    pub fn plain() -> Palette {
        Palette {
            agent_open: String::new(),
            agent_close: String::new(),
            human_open: String::new(),
            human_close: String::new(),
        }
    }

    fn open(&self, class: Class) -> &str {
        match class {
            Class::Agent => &self.agent_open,
            Class::Human => &self.human_open,
        }
    }

    fn close(&self, class: Class) -> &str {
        match class {
            Class::Agent => &self.agent_close,
            Class::Human => &self.human_close,
        }
    }
}

fn prec_of(exp: &Exp) -> Prec {
    match exp {
        Exp::Var(_)
        | Exp::Num(_)
        | Exp::Bool(_)
        | Exp::EmptyHole(_)
        | Exp::NonEmptyHole(_, _)
        | Exp::Pair(_, _) => PREC_ATOM,
        Exp::Ap(_, _) | Exp::Proj(_, _) => PREC_APP,
        Exp::BinOp(op, _, _) => op_prec(*op),
        Exp::If(_, _, _) | Exp::Let(_, _, _) | Exp::Lam(_, _, _) => PREC_BINDER,
    }
}

struct Marker<'a> {
    names: &'a NameTable,
    map: &'a Provenance,
    agents: &'a [AuthorId],
    palette: &'a Palette,
}

impl Marker<'_> {
    fn class_at(&self, path: &[usize]) -> Class {
        match self.map.get(path) {
            Some(value) if self.agents.contains(&value.author) => Class::Agent,
            _ => Class::Human,
        }
    }

    fn go(&self, exp: &Exp, path: &[usize], min_prec: Prec, enclosing: Class, out: &mut String) {
        let class = self.class_at(path);
        let marked = class != enclosing;
        if marked {
            out.push_str(self.palette.open(class));
        }
        let needs_parens = prec_of(exp) < min_prec;
        if needs_parens {
            out.push('(');
        }
        self.body(exp, path, class, out);
        if needs_parens {
            out.push(')');
        }
        if marked {
            out.push_str(self.palette.close(class));
        }
    }

    fn kid(
        &self,
        exp: &Exp,
        path: &[usize],
        n: usize,
        min_prec: Prec,
        class: Class,
        out: &mut String,
    ) {
        if let Some(c) = child(exp, n) {
            self.go(c, &extend(path, n), min_prec, class, out);
        }
    }

    fn body(&self, exp: &Exp, path: &[usize], class: Class, out: &mut String) {
        match exp {
            Exp::Var(id) => out.push_str(&render_id(*id, self.names)),
            Exp::Num(n) => out.push_str(&n.to_string()),
            Exp::Bool(b) => out.push_str(&b.to_string()),
            Exp::EmptyHole(_) => out.push_str("⦇⦈"),
            Exp::NonEmptyHole(..) => {
                out.push('⦇');
                self.kid(exp, path, 0, PREC_BINDER, class, out);
                out.push('⦈');
            }
            Exp::Pair(..) => {
                out.push('(');
                self.kid(exp, path, 0, PREC_BINDER, class, out);
                out.push_str(", ");
                self.kid(exp, path, 1, PREC_BINDER, class, out);
                out.push(')');
            }
            Exp::Proj(side, _) => {
                out.push_str(match side {
                    Side::L => "fst ",
                    Side::R => "snd ",
                });
                self.kid(exp, path, 0, PREC_ATOM, class, out);
            }
            Exp::Ap(..) => {
                self.kid(exp, path, 0, PREC_APP, class, out);
                out.push(' ');
                self.kid(exp, path, 1, PREC_ATOM, class, out);
            }
            Exp::BinOp(op, ..) => {
                let p = op_prec(*op);
                self.kid(exp, path, 0, p, class, out);
                out.push_str(&format!(" {} ", op_str(*op)));
                self.kid(exp, path, 1, p + 1, class, out);
            }
            Exp::If(..) => {
                out.push_str("if ");
                self.kid(exp, path, 0, PREC_CMP, class, out);
                out.push_str(" then ");
                self.kid(exp, path, 1, PREC_CMP, class, out);
                out.push_str(" else ");
                self.kid(exp, path, 2, PREC_BINDER, class, out);
            }
            Exp::Let(id, ..) => {
                out.push_str(&format!("let {} = ", render_id(*id, self.names)));
                self.kid(exp, path, 0, PREC_CMP, class, out);
                out.push_str(" in ");
                self.kid(exp, path, 1, PREC_BINDER, class, out);
            }
            Exp::Lam(id, ty, _) => {
                out.push_str(&format!("λ{}:{}. ", render_id(*id, self.names), ty));
                self.kid(exp, path, 0, PREC_BINDER, class, out);
            }
        }
    }
}

pub fn annotate(
    exp: &Exp,
    names: &NameTable,
    map: &Provenance,
    agents: &[AuthorId],
    palette: &Palette,
) -> String {
    let marker = Marker {
        names,
        map,
        agents,
        palette,
    };
    let mut out = String::new();
    marker.go(exp, &[], PREC_BINDER, Class::Human, &mut out);
    out
}

pub fn annotate_document(
    doc: &Doc,
    names: &NameTable,
    map: &Provenance,
    agents: &[AuthorId],
    palette: &Palette,
) -> String {
    doc.defs()
        .iter()
        .map(|def| {
            format!(
                "{} : {} = {}",
                render_id(def.id, names),
                def.ann,
                annotate(
                    &def.body,
                    names,
                    &map.in_definition(def.id),
                    agents,
                    palette
                )
            )
        })
        .collect::<Vec<String>>()
        .join("\n")
}

pub fn legend(palette: &Palette) -> String {
    format!(
        "{}model-authored{}   {}human-authored{}",
        palette.agent_open, palette.agent_close, palette.human_open, palette.human_close
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::AgentSession;
    use nothing_core::render::render;

    const HUMAN: AuthorId = AuthorId::new(1);
    const MODEL: AuthorId = AuthorId::new(2);

    fn mixed() -> AgentSession {
        let mut session = AgentSession::new(HUMAN);
        for step in [
            "construct-lam",
            "move-parent",
            "rename n",
            "set-ann Num",
            "move-child 0",
        ] {
            assert!(session.apply_text(step).unwrap(), "{step}");
        }
        session.set_author(MODEL);
        for step in ["construct-var n", "construct-binop mul", "construct-num 2"] {
            assert!(session.apply_text(step).unwrap(), "{step}");
        }
        session
    }

    #[test]
    fn every_node_of_the_head_program_has_a_provenance_slot() {
        let session = mixed();
        let map = provenance_of(session.base(), &session.applied_entries());
        assert_eq!(map.len(), paths_of(&session.exp()).len());
    }

    #[test]
    fn model_authored_nodes_carry_the_model_author() {
        let session = mixed();
        assert_eq!(session.state().render(), "λn:Num. n * 2");
        let map = provenance_of(session.base(), &session.applied_entries());
        assert_eq!(map.get(&[]).map(|v| v.author), Some(HUMAN));
        assert_eq!(map.get(&[0]).map(|v| v.author), Some(MODEL));
        assert_eq!(map.get(&[0, 0]).map(|v| v.author), Some(MODEL));
        assert_eq!(map.get(&[0, 1]).map(|v| v.author), Some(MODEL));
        assert_eq!(map.authors(), vec![HUMAN, MODEL]);
    }

    #[test]
    fn wrapping_a_node_does_not_reattribute_the_node_that_was_wrapped() {
        let mut session = AgentSession::new(HUMAN);
        assert!(session.apply_text("construct-num 1").unwrap());
        session.set_author(MODEL);
        assert!(session.apply_text("construct-binop add").unwrap());
        assert!(session.apply_text("construct-num 2").unwrap());
        assert_eq!(session.state().render(), "1 + 2");

        let map = provenance_of(session.base(), &session.applied_entries());
        assert_eq!(map.get(&[]).map(|v| v.author), Some(MODEL));
        assert_eq!(
            map.get(&[0]).map(|v| v.author),
            Some(HUMAN),
            "the wrapped `1` was written by the human"
        );
        assert_eq!(map.get(&[1]).map(|v| v.author), Some(MODEL));
    }

    #[test]
    fn a_rename_creates_no_node_but_is_recorded_against_the_binder() {
        let mut session = AgentSession::new(HUMAN);
        assert!(session.apply_text("construct-lam").unwrap());
        assert!(session.apply_text("move-parent").unwrap());
        let before = provenance_of(session.base(), &session.applied_entries());
        session.set_author(MODEL);
        assert!(session.apply_text("rename total").unwrap());
        let after = provenance_of(session.base(), &session.applied_entries());
        assert_eq!(
            before.get(&[]).map(|v| v.author),
            after.get(&[]).map(|v| v.author)
        );
        let id = session.state().zipper.binder_id().unwrap();
        assert_eq!(after.name_provenance(id).map(|v| v.author), Some(MODEL));
    }

    #[test]
    fn a_plain_palette_reproduces_the_ordinary_projection() {
        let session = mixed();
        let map = provenance_of(session.base(), &session.applied_entries());
        assert_eq!(
            annotate(
                &session.exp(),
                session.names(),
                &map,
                &[MODEL],
                &Palette::plain()
            ),
            render(&session.exp(), session.names())
        );
    }

    #[test]
    fn model_authored_spans_are_visually_distinguished() {
        let session = mixed();
        let map = provenance_of(session.base(), &session.applied_entries());
        let marked = annotate(
            &session.exp(),
            session.names(),
            &map,
            &[MODEL],
            &Palette::brackets(),
        );
        assert_eq!(marked, "λn:Num. ⟦n * 2⟧");

        let stripped = marked.replace(['⟦', '⟧', '⟨', '⟩'], "");
        assert_eq!(stripped, render(&session.exp(), session.names()));
    }

    #[test]
    fn a_human_node_inside_a_model_span_is_marked_back_out() {
        let mut session = AgentSession::new(HUMAN);
        assert!(session.apply_text("construct-num 1").unwrap());
        session.set_author(MODEL);
        assert!(session.apply_text("construct-binop add").unwrap());
        assert!(session.apply_text("construct-num 2").unwrap());
        let map = provenance_of(session.base(), &session.applied_entries());
        let marked = annotate(
            &session.exp(),
            session.names(),
            &map,
            &[MODEL],
            &Palette::brackets(),
        );
        assert_eq!(marked, "⟦⟨1⟩ + 2⟧");
    }

    #[test]
    fn with_no_agents_declared_nothing_is_marked() {
        let session = mixed();
        let map = provenance_of(session.base(), &session.applied_entries());
        assert_eq!(
            annotate(
                &session.exp(),
                session.names(),
                &map,
                &[],
                &Palette::brackets()
            ),
            render(&session.exp(), session.names())
        );
    }

    #[test]
    fn the_ansi_palette_emits_colour_and_still_strips_back() {
        let session = mixed();
        let map = provenance_of(session.base(), &session.applied_entries());
        let marked = annotate(
            &session.exp(),
            session.names(),
            &map,
            &[MODEL],
            &Palette::ansi(),
        );
        assert!(marked.contains("\u{1b}[35m"));
        let stripped = marked
            .replace("\u{1b}[35m", "")
            .replace("\u{1b}[36m", "")
            .replace("\u{1b}[0m", "");
        assert_eq!(stripped, render(&session.exp(), session.names()));
    }

    fn two_definitions() -> AgentSession {
        let mut session = AgentSession::new(HUMAN);
        assert!(session.apply_text("construct-num 1").unwrap());
        session.set_author(MODEL);
        for step in ["create-definition", "rename-def helper", "construct-num 2"] {
            assert!(session.apply_text(step).unwrap(), "{step}");
        }
        session
    }

    #[test]
    fn every_definition_keeps_its_own_provenance() {
        let session = two_definitions();
        let ids = session.state().definition_ids();
        assert_eq!(ids.len(), 2);
        let map = provenance_of(session.base(), &session.applied_entries());

        assert_eq!(map.definitions(), {
            let mut sorted = ids.clone();
            sorted.sort();
            sorted
        });
        assert_eq!(map.get_in(ids[0], &[]).map(|v| v.author), Some(HUMAN));
        assert_eq!(map.get_in(ids[1], &[]).map(|v| v.author), Some(MODEL));
    }

    #[test]
    fn editing_one_definition_does_not_reattribute_another() {
        let mut session = two_definitions();
        let ids = session.state().definition_ids();
        session.set_author(HUMAN);
        assert!(session.apply_text("construct-binop add").unwrap());
        assert!(session.apply_text("construct-num 5").unwrap());

        let map = provenance_of(session.base(), &session.applied_entries());
        assert_eq!(
            map.get_in(ids[0], &[]).map(|v| v.author),
            Some(HUMAN),
            "the untouched definition changed hands"
        );
        assert_eq!(map.get_in(ids[1], &[0]).map(|v| v.author), Some(MODEL));
        assert_eq!(map.get_in(ids[1], &[1]).map(|v| v.author), Some(HUMAN));
    }

    #[test]
    fn the_whole_document_annotates_definition_by_definition() {
        let session = two_definitions();
        let map = provenance_of(session.base(), &session.applied_entries());
        let marked = annotate_document(
            &session.state().doc(),
            session.names(),
            &map,
            &[MODEL],
            &Palette::brackets(),
        );
        assert_eq!(marked, "main : ? = 1\nhelper : ? = ⟦2⟧");
    }

    #[test]
    fn the_json_projection_lists_every_node() {
        let session = mixed();
        let map = provenance_of(session.base(), &session.applied_entries());
        let text = provenance_json(&map).to_string();
        let parsed = crate::json::parse(&text).unwrap();
        assert_eq!(parsed.as_arr().unwrap().len(), map.len());
    }
}
