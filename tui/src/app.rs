use nothing_action::act::{Action, EditState, ctx_and_expected_ty_at_in};
use nothing_action::script::replay_script;
use nothing_action::zipper::{Frame, Zipper, all_positions};
use nothing_core::ctx::Ctx;
use nothing_core::exp::{Exp, Id};
use nothing_core::names::NameTable;
use nothing_core::render::{PREC_APP, Prec, op_prec};
use nothing_core::ty::Ty;

use crate::history::{History, Typing};
use crate::live::EngineHandle;
use crate::projection::ProjectionKind;

const FACTORIAL_FIXTURE: &str = include_str!("../../bench/fixtures/factorial.actions");

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum Slot {
    #[default]
    Node,
    BinderName,
    Annotation,
    DefName,
    DefAnn,
    FieldName,
    FieldPick,
    ConstructorName,
    ConstructorPick,
}

impl Slot {
    pub fn label(self) -> &'static str {
        match self {
            Slot::Node => "node",
            Slot::BinderName => "binder name",
            Slot::Annotation => "annotation",
            Slot::DefName => "definition name",
            Slot::DefAnn => "definition type",
            Slot::FieldName | Slot::FieldPick => "field",
            Slot::ConstructorName | Slot::ConstructorPick => "constructor",
        }
    }

    pub fn names_a_field(self) -> bool {
        matches!(self, Slot::FieldName | Slot::FieldPick)
    }

    pub fn names_a_constructor(self) -> bool {
        matches!(self, Slot::ConstructorName | Slot::ConstructorPick)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct AppState {
    pub edit: EditState,
    pub slot: Slot,
    pub entry: String,
    pub entry_committed: bool,
    pub string_open: bool,
    pub escape_armed: bool,
    pub hint: Option<String>,
    pub quit: bool,
    pub engine: EngineHandle,
    pub projection_override: Option<ProjectionKind>,
    base: EditState,
    history: History,
}

impl AppState {
    pub fn new(exp: Exp) -> AppState {
        AppState::from_edit(EditState::new(exp))
    }

    pub fn with_names(exp: Exp, names: NameTable) -> AppState {
        AppState::from_edit(EditState::with_names(exp, names))
    }

    pub fn empty() -> AppState {
        AppState::from_edit(EditState::empty())
    }

    pub fn from_edit(edit: EditState) -> AppState {
        AppState {
            base: edit.clone(),
            edit,
            slot: Slot::Node,
            entry: String::new(),
            entry_committed: false,
            string_open: false,
            escape_armed: false,
            hint: None,
            quit: false,
            engine: EngineHandle::new(),
            projection_override: None,
            history: History::new(),
        }
    }

    pub fn factorial() -> AppState {
        let state = replay_script(FACTORIAL_FIXTURE)
            .expect("the embedded factorial fixture must replay cleanly");
        AppState::from_edit(EditState::with_names(state.exp(), state.names.clone()))
    }

    pub fn binders_in_scope(&self) -> Vec<Id> {
        let binders = self.edit.zipper.binders();
        let mut out: Vec<Id> = self
            .edit
            .definition_ids()
            .into_iter()
            .filter(|id| !binders.contains(id))
            .collect();
        out.extend(binders);
        out
    }

    pub fn definitions(&self) -> Vec<Id> {
        self.edit.definition_ids()
    }

    pub fn definition_id(&self) -> Id {
        self.edit.def_id()
    }

    pub fn definition_index(&self) -> usize {
        self.edit.def_index()
    }

    pub fn definition_count(&self) -> usize {
        self.edit.def_count()
    }

    pub fn definition_ann(&self) -> Ty {
        self.edit.def_ann().clone()
    }

    pub fn definition_name(&self) -> String {
        self.edit.names.display(self.edit.def_id())
    }

    pub fn names(&self) -> &NameTable {
        &self.edit.names
    }

    pub fn display_name(&self, id: Id) -> String {
        self.edit.names.display(id)
    }

    pub fn focus_binder_id(&self) -> Option<Id> {
        self.edit.zipper.binder_id()
    }

    pub fn text(&self) -> String {
        nothing_core::render::render(&self.program(), self.names())
    }

    pub fn zipper(&self) -> &Zipper {
        &self.edit.zipper
    }

    pub fn focus(&self) -> &Exp {
        &self.edit.zipper.focus
    }

    pub fn program(&self) -> Exp {
        self.edit.exp()
    }

    pub fn ctx(&self) -> Ctx {
        ctx_and_expected_ty_at_in(&self.edit.scope(), &self.edit.zipper).0
    }

    pub fn finishes(&self) -> bool {
        matches!(self.focus(), Exp::NonEmptyHole(..)) && self.edit.apply(Action::Finish).is_some()
    }

    pub fn enclosing_quarantine(&self) -> Option<usize> {
        self.edit
            .zipper
            .path
            .iter()
            .rev()
            .position(|frame| matches!(frame, Frame::NonEmptyHoleBody(_)))
            .map(|steps| steps + 1)
    }

    pub fn enclosing_finishes(&self) -> Option<bool> {
        let steps = self.enclosing_quarantine()?;
        let mut actions = vec![Action::MoveParent; steps];
        actions.push(Action::Finish);
        Some(self.apply_actions(&actions).is_some())
    }

    pub fn quarantines(&self) -> usize {
        fn count(exp: &Exp) -> usize {
            let here = usize::from(matches!(exp, Exp::NonEmptyHole(..)));
            here + children(exp).iter().map(|c| count(c)).sum::<usize>()
        }
        count(&self.program())
    }

    pub fn expected_ty(&self) -> Ty {
        ctx_and_expected_ty_at_in(&self.edit.scope(), &self.edit.zipper).1
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> AppState {
        self.hint = Some(hint.into());
        self
    }

    pub fn clear_hint(mut self) -> AppState {
        self.hint = None;
        self
    }

    pub fn apply_actions(&self, actions: &[Action]) -> Option<AppState> {
        let mut edit = self.edit.clone();
        for action in actions {
            if !edit.apply_mut(action.clone()) {
                return None;
            }
        }
        let mut next = self.clone();
        next.edit = edit;
        next.slot = Slot::Node;
        next.clear_entry();
        next.hint = None;
        next.history.record(actions);
        Some(next)
    }

    pub fn clear_entry(&mut self) {
        self.entry.clear();
        self.entry_committed = false;
        self.string_open = false;
        self.escape_armed = false;
    }

    pub fn actions(&self) -> &[Action] {
        self.history.actions()
    }

    pub fn keystrokes(&self) -> usize {
        self.history.keystrokes()
    }

    pub fn open_keystroke(&self) -> (usize, Typing) {
        (self.history.applied(), self.typing())
    }

    pub fn close_keystroke(mut self, opened: (usize, Typing)) -> AppState {
        let after = self.typing();
        self.history.close_keystroke(opened.0, opened.1, after);
        self
    }

    fn typing(&self) -> Typing {
        Typing {
            slot: self.slot,
            text: self.entry.clone(),
            committed: self.entry_committed,
            string_open: self.string_open,
            escape_armed: self.escape_armed,
        }
    }

    pub fn undo(&self) -> Option<AppState> {
        let mut next = self.clone();
        let (prefix, typing) = next.history.undo()?;
        next.rewind_to(prefix, typing);
        Some(next)
    }

    pub fn redo(&self) -> Option<AppState> {
        let mut next = self.clone();
        let (prefix, typing) = next.history.redo()?;
        next.rewind_to(prefix, typing);
        Some(next)
    }

    fn rewind_to(&mut self, prefix: usize, typing: Typing) {
        let mut edit = self.base.clone();
        for action in &self.history.actions()[..prefix] {
            edit.apply_mut(action.clone());
        }
        self.edit = edit;
        self.slot = typing.slot;
        self.entry = typing.text;
        self.entry_committed = typing.committed;
        self.string_open = typing.string_open;
        self.escape_armed = typing.escape_armed;
    }

    fn in_slot(&self, slot: Slot) -> AppState {
        let mut next = self.clone();
        next.slot = slot;
        next.clear_entry();
        next
    }

    fn binder_kind(&self) -> Option<BinderKind> {
        match self.focus() {
            Exp::Lam(..) => Some(BinderKind::Lam),
            Exp::Let(..) | Exp::CmdBind(..) => Some(BinderKind::Let),
            _ => None,
        }
    }

    pub fn move_down(&self) -> Option<AppState> {
        match self.slot {
            Slot::BinderName
            | Slot::Annotation
            | Slot::FieldName
            | Slot::FieldPick
            | Slot::ConstructorName
            | Slot::ConstructorPick => None,
            Slot::DefName => Some(self.in_slot(Slot::DefAnn)),
            Slot::DefAnn => Some(self.in_slot(Slot::Node)),
            Slot::Node => match self.binder_kind() {
                Some(_) => Some(self.in_slot(Slot::BinderName)),
                None => {
                    let into = self.apply_actions(&[Action::MoveChild(0)])?;
                    Some(match into.edit.zipper.record_field_id() {
                        Some(_) => into.in_slot(Slot::FieldName),
                        None => into,
                    })
                }
            },
        }
    }

    pub fn move_up(&self) -> Option<AppState> {
        match self.slot {
            Slot::BinderName
            | Slot::Annotation
            | Slot::FieldName
            | Slot::FieldPick
            | Slot::ConstructorName
            | Slot::ConstructorPick => Some(self.in_slot(Slot::Node)),
            Slot::DefName => None,
            Slot::DefAnn => Some(self.in_slot(Slot::DefName)),
            Slot::Node => match self.apply_actions(&[Action::MoveParent]) {
                Some(next) => Some(next),
                None => Some(self.in_slot(Slot::DefName)),
            },
        }
    }

    pub fn move_next(&self) -> Option<AppState> {
        match (self.slot, self.binder_kind()) {
            (Slot::DefName, _) => Some(self.in_slot(Slot::DefAnn)),
            (Slot::DefAnn, _) => Some(self.in_slot(Slot::Node)),
            (Slot::BinderName, Some(BinderKind::Lam)) => Some(self.in_slot(Slot::Annotation)),
            (Slot::BinderName, Some(BinderKind::Let)) => {
                self.apply_actions(&[Action::MoveChild(0)])
            }

            (Slot::Annotation, _) => self.apply_actions(&[Action::MoveChild(0)]),
            (Slot::BinderName, None) => None,
            (Slot::FieldName | Slot::FieldPick, _) => Some(self.in_slot(Slot::Node)),
            (Slot::ConstructorName | Slot::ConstructorPick, _) => Some(self.in_slot(Slot::Node)),
            (Slot::Node, _) => match self.edit.zipper.path.last() {
                Some(Frame::LamBody(..)) | Some(Frame::LetBody(..)) | Some(Frame::BindBody(..)) => {
                    None
                }
                Some(Frame::RecordField(..)) => Some(
                    self.apply_actions(&[Action::MoveNextSibling])?
                        .in_slot(Slot::FieldName),
                ),
                Some(Frame::MatchArm(..)) => Some(
                    self.apply_actions(&[Action::MoveNextSibling])?
                        .in_slot(Slot::ConstructorName),
                ),
                Some(_) => self.apply_actions(&[Action::MoveNextSibling]),
                None => None,
            },
        }
    }

    pub fn move_prev(&self) -> Option<AppState> {
        match self.slot {
            Slot::DefName => None,
            Slot::DefAnn => Some(self.in_slot(Slot::DefName)),
            Slot::Annotation => Some(self.in_slot(Slot::BinderName)),

            Slot::BinderName | Slot::FieldPick | Slot::ConstructorPick => None,
            Slot::FieldName => self.apply_actions(&[Action::MovePrevSibling]),
            Slot::ConstructorName => self.apply_actions(&[Action::MovePrevSibling]),
            Slot::Node => match self.edit.zipper.path.last() {
                Some(Frame::LamBody(..)) => Some(
                    self.apply_actions(&[Action::MoveParent])?
                        .in_slot(Slot::Annotation),
                ),
                Some(Frame::LetBound(..)) => Some(
                    self.apply_actions(&[Action::MoveParent])?
                        .in_slot(Slot::BinderName),
                ),
                Some(Frame::RecordField(..)) => Some(self.in_slot(Slot::FieldName)),
                Some(Frame::MatchArm(..)) => Some(self.in_slot(Slot::ConstructorName)),
                Some(_) => self.apply_actions(&[Action::MovePrevSibling]),
                None => None,
            },
        }
    }

    pub fn field_slot_id(&self) -> Option<Id> {
        match self.slot {
            Slot::FieldName => self.edit.zipper.record_field_id(),
            Slot::FieldPick => self.edit.zipper.projected_field_id(),
            _ => None,
        }
    }

    pub fn field_name_target(&self) -> Option<AppState> {
        if self.edit.zipper.record_field_id().is_some() {
            return Some(self.in_slot(Slot::FieldName));
        }
        let last = match self.focus() {
            Exp::Record(fields) if !fields.is_empty() => fields.len() - 1,
            _ => return None,
        };
        Some(
            self.apply_actions(&[Action::MoveChild(last)])?
                .in_slot(Slot::FieldName),
        )
    }

    pub fn in_record(&self) -> bool {
        matches!(self.focus(), Exp::Record(_)) || self.edit.zipper.record_field_id().is_some()
    }

    pub fn constructor_slot_id(&self) -> Option<Id> {
        match self.slot {
            Slot::ConstructorName => self.edit.zipper.arm_constructor_id(),
            Slot::ConstructorPick => self.edit.zipper.injected_constructor_id(),
            _ => None,
        }
    }

    pub fn constructor_name_target(&self) -> Option<AppState> {
        if self.edit.zipper.arm_constructor_id().is_some() {
            return Some(self.in_slot(Slot::ConstructorName));
        }
        let last = match self.focus() {
            Exp::Match(_, arms) if !arms.is_empty() => arms.len(),
            _ => return None,
        };
        Some(
            self.apply_actions(&[Action::MoveChild(last)])?
                .in_slot(Slot::ConstructorName),
        )
    }

    pub fn in_match(&self) -> bool {
        matches!(self.focus(), Exp::Match(..)) || self.edit.zipper.arm_constructor_id().is_some()
    }

    pub fn move_to_hole(&self, forward: bool) -> Option<AppState> {
        let program = self.program();
        let positions = all_positions(&program);
        let here = position_index(&positions, &self.edit.zipper)?;

        let holes: Vec<usize> = positions
            .iter()
            .enumerate()
            .filter(|(_, z)| is_unfinished(&z.focus))
            .map(|(i, _)| i)
            .collect();
        if holes.is_empty() {
            return None;
        }

        let target = if forward {
            holes
                .iter()
                .copied()
                .find(|&i| i > here)
                .unwrap_or(holes[0])
        } else {
            holes
                .iter()
                .copied()
                .rev()
                .find(|&i| i < here)
                .unwrap_or(holes[holes.len() - 1])
        };

        let actions = moves_between(&self.edit.zipper, &positions[target]);
        self.apply_actions(&actions)
    }

    pub fn climb_actions(&self, prec: Prec) -> Vec<Action> {
        if self.slot != Slot::Node || matches!(self.focus(), Exp::EmptyHole(_)) {
            return Vec::new();
        }
        let mut path = self.edit.zipper.path.as_slice();
        let mut steps = 0;
        while let Some(frame) = path.last() {
            match climbable_prec(frame) {
                Some(parent) if parent >= prec => {}
                _ => break,
            }
            steps += 1;
            path = &path[..path.len() - 1];
        }
        vec![Action::MoveParent; steps]
    }

    pub fn binder_body_child(&self) -> Option<usize> {
        match self.focus() {
            Exp::Lam(..) => Some(0),
            Exp::Let(..) => Some(1),
            _ => None,
        }
    }

    pub fn exit_slot_to_body(&self) -> Option<AppState> {
        let child = self.binder_body_child()?;
        self.apply_actions(&[Action::MoveChild(child)])
    }

    pub fn active_projection(&self) -> ProjectionKind {
        crate::projection::active_kind(self)
    }

    pub fn cycle_projection(&self) -> AppState {
        let mut next = self.clone();
        next.projection_override = crate::projection::next_override(self.projection_override);
        next.hint = None;
        next
    }

    pub fn focus_binder(&self) -> Option<AppState> {
        if self.binder_kind().is_some() {
            return Some(self.clone());
        }
        for step in [Action::MoveParent, Action::MoveChild(0)] {
            if let Some(next) = self.apply_actions(&[step])
                && next.binder_kind().is_some()
            {
                return Some(next);
            }
        }
        None
    }
}

fn climbable_prec(frame: &Frame) -> Option<Prec> {
    match frame {
        Frame::BinOpRight(op, _) => Some(op_prec(*op)),
        Frame::ApArg(_) | Frame::ProjBody(_) => Some(PREC_APP),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BinderKind {
    Lam,
    Let,
}

fn is_unfinished(exp: &Exp) -> bool {
    matches!(exp, Exp::EmptyHole(_) | Exp::NonEmptyHole(..))
}

fn children(exp: &Exp) -> Vec<&Exp> {
    match exp {
        Exp::Var(_)
        | Exp::Num(_)
        | Exp::Bool(_)
        | Exp::Str(_)
        | Exp::Nil
        | Exp::Readline
        | Exp::EmptyHole(_) => Vec::new(),
        Exp::Lam(_, _, b)
        | Exp::Proj(_, b)
        | Exp::Field(b, _)
        | Exp::Print(b)
        | Exp::CmdPure(b)
        | Exp::NonEmptyHole(_, b) => vec![b],
        Exp::Inj(_, payload) => vec![payload],
        Exp::Match(scrutinee, arms) => {
            let mut out = vec![&**scrutinee];
            out.extend(arms.iter().map(|(_, _, body)| body));
            out
        }
        Exp::Ap(a, b)
        | Exp::BinOp(_, a, b)
        | Exp::Let(_, a, b)
        | Exp::Pair(a, b)
        | Exp::CmdBind(a, _, b)
        | Exp::Cons(a, b) => vec![a, b],
        Exp::If(c, t, e) | Exp::Fold(c, t, e) => vec![c, t, e],
        Exp::Record(fields) => fields.iter().map(|(_, value)| value).collect(),
    }
}

pub fn index_path(z: &Zipper) -> Vec<usize> {
    z.path.iter().map(Frame::child_index).collect()
}

fn position_index(positions: &[Zipper], z: &Zipper) -> Option<usize> {
    let target = index_path(z);
    positions.iter().position(|p| index_path(p) == target)
}

pub fn moves_between(from: &Zipper, to: &Zipper) -> Vec<Action> {
    let from_path = index_path(from);
    let to_path = index_path(to);
    let common = from_path
        .iter()
        .zip(to_path.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let mut actions = vec![Action::MoveParent; from_path.len() - common];
    actions.extend(to_path[common..].iter().map(|&i| Action::MoveChild(i)));
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::examples;

    fn at(state: &AppState) -> (Exp, Vec<usize>, Slot) {
        (state.program(), index_path(state.zipper()), state.slot)
    }

    #[test]
    fn factorial_demo_renders_the_reference_program() {
        let state = AppState::factorial();
        assert_eq!(
            state.text(),
            "λx0:Num. if x0 == 0 then 1 else x0 * main (x0 - 1)"
        );
        assert!(state.edit.zipper.is_root());
        assert_eq!(state.slot, Slot::Node);
    }

    #[test]
    fn moving_never_changes_the_program() {
        let start = AppState::factorial();
        let program = start.program();
        let mut state = start;

        for step in [
            AppState::move_down,
            AppState::move_next,
            AppState::move_next,
            AppState::move_down,
            AppState::move_next,
            AppState::move_up,
            AppState::move_prev,
        ] {
            if let Some(next) = step(&state) {
                state = next;
            }
            assert_eq!(state.program(), program);
        }
    }

    #[test]
    fn lambda_slots_walk_name_annotation_body() {
        let lam = AppState::factorial();
        let name = lam.move_down().expect("a lambda has a binder name");
        assert_eq!(name.slot, Slot::BinderName);
        assert_eq!(name.zipper().path.len(), 0, "a slot stays on the node");

        let ann = name.move_next().expect("name → annotation");
        assert_eq!(ann.slot, Slot::Annotation);

        let body = ann.move_next().expect("annotation → body");
        assert_eq!(body.slot, Slot::Node);
        assert!(matches!(body.focus(), Exp::If(..)));
        assert!(body.move_next().is_none(), "the body is the last child");

        let back_ann = body.move_prev().expect("body → annotation");
        assert_eq!(at(&back_ann), at(&ann));
        assert_eq!(back_ann.move_prev().as_ref().map(at), Some(at(&name)),);
        assert!(name.move_prev().is_none(), "the name is the first child");
        assert_eq!(name.move_up().as_ref().map(at), Some(at(&lam)));
    }

    #[test]
    fn let_slots_walk_name_bound_body() {
        let root = AppState::new(examples::let_identity());
        let name = root.move_down().expect("a let has a binder name");
        assert_eq!(name.slot, Slot::BinderName);

        let bound = name.move_next().expect("name → bound expression");
        assert_eq!(bound.slot, Slot::Node);
        assert_eq!(bound.zipper().child_index(), Some(0));

        let body = bound.move_next().expect("bound → body");
        assert_eq!(body.zipper().child_index(), Some(1));
        assert!(body.move_next().is_none());

        assert_eq!(body.move_prev().as_ref().map(at), Some(at(&bound)));
        assert_eq!(bound.move_prev().as_ref().map(at), Some(at(&name)));
    }

    #[test]
    fn a_slot_has_no_children_and_ascends_to_its_node() {
        let name = AppState::factorial().move_down().unwrap();
        assert!(name.move_down().is_none());
        assert_eq!(name.move_up().map(|s| s.slot), Some(Slot::Node));
    }

    #[test]
    fn tab_cycles_the_empty_holes() {
        let state = AppState::new(examples::add_with_empty_hole());
        let hole = state.move_to_hole(true).expect("there is a hole");
        assert!(matches!(hole.focus(), Exp::EmptyHole(_)));
        assert_eq!(hole.zipper().child_index(), Some(1));
        assert_eq!(hole.move_to_hole(true).as_ref().map(at), Some(at(&hole)));
        assert_eq!(hole.move_to_hole(false).as_ref().map(at), Some(at(&hole)));
    }

    #[test]
    fn tab_wraps_and_shift_tab_reverses() {
        let program = Exp::pair(
            Exp::empty_hole(nothing_core::exp::HoleId::from_u128(0)),
            Exp::empty_hole(nothing_core::exp::HoleId::from_u128(1)),
        );
        let root = AppState::new(program);

        let fst = root.move_to_hole(true).expect("two holes");
        assert_eq!(index_path(fst.zipper()), vec![0]);
        let snd = fst.move_to_hole(true).expect("two holes");
        assert_eq!(index_path(snd.zipper()), vec![1]);

        let wrapped = snd.move_to_hole(true).expect("two holes");
        assert_eq!(index_path(wrapped.zipper()), vec![0]);

        assert_eq!(
            index_path(snd.move_to_hole(false).unwrap().zipper()),
            vec![0]
        );
        assert_eq!(
            index_path(fst.move_to_hole(false).unwrap().zipper()),
            vec![1]
        );
    }

    #[test]
    fn tab_declines_on_a_program_with_no_holes() {
        let state = AppState::new(examples::increment_applied());
        assert!(state.move_to_hole(true).is_none());
        assert!(state.move_to_hole(false).is_none());
    }

    #[test]
    fn moves_between_is_up_then_down() {
        let program = examples::clamp_to_one();
        let positions = all_positions(&program);
        let from = positions
            .iter()
            .find(|z| index_path(z) == vec![0, 0, 0])
            .expect("cond's lhs");
        let to = positions
            .iter()
            .find(|z| index_path(z) == vec![0, 2])
            .expect("else branch");
        assert_eq!(
            moves_between(from, to),
            vec![Action::MoveParent, Action::MoveParent, Action::MoveChild(2)]
        );

        let arrived = AppState::from_edit(EditState::at(
            from.clone(),
            nothing_action::act::Fresh::from_program(&program),
            examples::names(),
        ))
        .apply_actions(&moves_between(from, to))
        .expect("the expansion applies");
        assert_eq!(index_path(arrived.zipper()), vec![0, 2]);
        assert_eq!(arrived.program(), program);
    }

    #[test]
    fn movement_clears_the_entry_buffer() {
        let mut state = AppState::factorial();
        state.entry = "fact".to_string();
        assert_eq!(state.move_down().map(|s| s.entry), Some(String::new()));
    }
}
