
use ratatui::crossterm::event::KeyEvent;

use nothing_core::exp::Exp;

use crate::app::AppState;
use crate::beginner;
use crate::render::{program_line, wrap_lines};
use crate::state_machine;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ProjectionKind {
    Text,
    StateMachine,
    Beginner,
}

impl ProjectionKind {
    pub fn label(self) -> &'static str {
        match self {
            ProjectionKind::Text => "text",
            ProjectionKind::StateMachine => "state machine",
            ProjectionKind::Beginner => "beginner",
        }
    }
}

pub trait Projection {
    fn kind(&self) -> ProjectionKind;

    fn recognizes(&self, program: &Exp) -> bool;

    fn marked_text(&self, state: &AppState) -> String;

    fn handle_key(&self, key: KeyEvent, state: AppState) -> Option<AppState>;
}

pub struct TextProjection;
pub struct StateMachineProjection;
pub struct BeginnerProjection;

impl Projection for TextProjection {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::Text
    }

    fn recognizes(&self, _program: &Exp) -> bool {
        true
    }

    fn marked_text(&self, state: &AppState) -> String {
        program_line(state)
    }

    fn handle_key(&self, _key: KeyEvent, _state: AppState) -> Option<AppState> {
        None
    }
}

impl Projection for StateMachineProjection {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::StateMachine
    }

    fn recognizes(&self, program: &Exp) -> bool {
        state_machine::recognize(program).is_some()
    }

    fn marked_text(&self, state: &AppState) -> String {
        state_machine::marked_text(state)
    }

    fn handle_key(&self, key: KeyEvent, state: AppState) -> Option<AppState> {
        state_machine::handle_key(key, state)
    }
}

impl Projection for BeginnerProjection {
    fn kind(&self) -> ProjectionKind {
        ProjectionKind::Beginner
    }

    fn recognizes(&self, _program: &Exp) -> bool {
        true
    }

    fn marked_text(&self, state: &AppState) -> String {
        beginner::marked_text(state)
    }

    fn handle_key(&self, _key: KeyEvent, _state: AppState) -> Option<AppState> {
        None
    }
}

static TEXT: TextProjection = TextProjection;
static STATE_MACHINE: StateMachineProjection = StateMachineProjection;
static BEGINNER: BeginnerProjection = BeginnerProjection;

pub fn projection(kind: ProjectionKind) -> &'static dyn Projection {
    match kind {
        ProjectionKind::Text => &TEXT,
        ProjectionKind::StateMachine => &STATE_MACHINE,
        ProjectionKind::Beginner => &BEGINNER,
    }
}

const AUTO_CANDIDATES: [ProjectionKind; 1] = [ProjectionKind::StateMachine];

pub fn active_kind(state: &AppState) -> ProjectionKind {
    if let Some(kind) = state.projection_override {
        return kind;
    }
    let program = state.program();
    for kind in AUTO_CANDIDATES {
        if projection(kind).recognizes(&program) {
            return kind;
        }
    }
    ProjectionKind::Text
}

pub fn active(state: &AppState) -> &'static dyn Projection {
    projection(active_kind(state))
}

pub fn next_override(current: Option<ProjectionKind>) -> Option<ProjectionKind> {
    match current {
        None => Some(ProjectionKind::Text),
        Some(ProjectionKind::Text) => Some(ProjectionKind::StateMachine),
        Some(ProjectionKind::StateMachine) => Some(ProjectionKind::Beginner),
        Some(ProjectionKind::Beginner) => None,
    }
}

pub fn lines_for(state: &AppState, width: usize) -> Vec<String> {
    let text = active(state).marked_text(state);
    let mut out = Vec::new();
    for line in text.split('\n') {
        out.extend(wrap_lines(line, width));
    }
    out
}
