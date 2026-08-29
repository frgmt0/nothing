use nothing_action::act::{Action, EditState};
use nothing_action::log::{ActionLog, AuthorId, LogEntry};
use nothing_action::script::{ParseError, Step, parse_step};
use nothing_action::zipper::Frame;
use nothing_core::exp::Exp;
use nothing_core::names::NameTable;
use nothing_store::document::{Document, decode_document, encode_document};

#[derive(Clone, PartialEq, Debug)]
pub struct AgentSession {
    base: EditState,
    log: ActionLog,
    cursor: usize,
    state: EditState,
    author: AuthorId,
    clock: u64,
}

impl AgentSession {
    pub fn new(author: AuthorId) -> AgentSession {
        AgentSession::from_base(EditState::empty(), ActionLog::new(), author)
    }

    pub fn from_base(base: EditState, log: ActionLog, author: AuthorId) -> AgentSession {
        let mut session = AgentSession {
            base,
            log,
            cursor: 0,
            state: EditState::empty(),
            author,
            clock: 0,
        };
        session.cursor = session.log.len();
        session.state = session.replay_prefix(session.cursor);
        session
    }

    pub fn author(&self) -> AuthorId {
        self.author
    }

    pub fn set_author(&mut self, author: AuthorId) {
        self.author = author;
    }

    pub fn state(&self) -> &EditState {
        &self.state
    }

    pub fn log(&self) -> &ActionLog {
        &self.log
    }

    pub fn base(&self) -> &EditState {
        &self.base
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn exp(&self) -> Exp {
        self.state.exp()
    }

    pub fn names(&self) -> &NameTable {
        self.state.names()
    }

    pub fn cursor_path(&self) -> Vec<usize> {
        self.state
            .zipper
            .path
            .iter()
            .map(Frame::child_index)
            .collect()
    }

    fn replay_prefix(&self, n: usize) -> EditState {
        let mut state = self.base.clone();
        for entry in self.log.entries().iter().take(n) {
            state.apply_mut(entry.action.clone());
        }
        state
    }

    fn tick(&mut self) -> u64 {
        let stamp = nothing_action::log::now_millis();
        self.clock = self.clock.max(stamp).max(self.clock + 1);
        self.clock
    }

    pub fn resolve(&self, step: &Step) -> Result<Action, ParseError> {
        step.resolve(&self.state)
    }

    pub fn parse(&self, text: &str) -> Result<Action, ParseError> {
        self.resolve(&parse_step(text)?)
    }

    pub fn apply(&mut self, action: Action) -> bool {
        match self.state.apply(action.clone()) {
            Some(next) => {
                let stamp = self.tick();
                self.log.truncate(self.cursor);
                self.log.append(action, stamp, self.author);
                self.cursor = self.log.len();
                self.state = next;
                true
            }
            None => false,
        }
    }

    pub fn apply_as(&mut self, action: Action, author: AuthorId) -> bool {
        let previous = self.author;
        self.author = author;
        let applied = self.apply(action);
        self.author = previous;
        applied
    }

    pub fn apply_text(&mut self, text: &str) -> Result<bool, ParseError> {
        let action = self.parse(text)?;
        Ok(self.apply(action))
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.log.len()
    }

    pub fn undo(&mut self) -> bool {
        if !self.can_undo() {
            return false;
        }
        self.cursor -= 1;
        self.state = self.replay_prefix(self.cursor);
        true
    }

    pub fn redo(&mut self) -> bool {
        if !self.can_redo() {
            return false;
        }
        self.cursor += 1;
        self.state = self.replay_prefix(self.cursor);
        true
    }

    pub fn reset(&mut self) {
        self.base = EditState::empty();
        self.log = ActionLog::new();
        self.cursor = 0;
        self.state = self.base.clone();
    }

    pub fn document(&self) -> Document {
        let mut log = ActionLog::new();
        for entry in self.log.entries().iter().take(self.cursor) {
            log.append(entry.action.clone(), entry.timestamp, entry.author);
        }
        Document::from_doc(self.state.doc(), self.names().flatten(), log)
    }

    pub fn save(&self, path: &str) -> Result<usize, String> {
        let bytes = encode_document(&self.document());
        std::fs::write(path, &bytes).map_err(|e| format!("cannot write {path}: {e}"))?;
        Ok(bytes.len())
    }

    pub fn load(&mut self, path: &str) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
        let doc = decode_document(&bytes).map_err(|e| format!("cannot decode {path}: {e:?}"))?;
        self.adopt(doc);
        Ok(())
    }

    pub fn adopt(&mut self, doc: Document) {
        let replayed = replay_log(&doc.log);
        if replayed.doc() == doc.doc {
            self.base = EditState::empty();
            self.log = doc.log;
        } else {
            self.base = EditState::with_doc(&doc.doc, doc.names, 0)
                .expect("a decoded document always has a first definition");
            self.log = doc.log;
        }
        self.cursor = self.log.len();
        self.state = self.replay_prefix(self.cursor);
    }

    pub fn entries(&self) -> &[LogEntry] {
        self.log.entries()
    }

    pub fn applied_entries(&self) -> Vec<LogEntry> {
        self.log
            .entries()
            .iter()
            .take(self.cursor)
            .cloned()
            .collect()
    }
}

pub fn replay_log(log: &ActionLog) -> EditState {
    let mut state = EditState::empty();
    for entry in log.entries() {
        state.apply_mut(entry.action.clone());
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::typing::is_well_typed;

    fn factorial_script() -> Vec<&'static str> {
        vec![
            "construct-lam",
            "move-parent",
            "rename x0",
            "set-ann Num",
            "move-child 0",
            "construct-if",
            "construct-binop eq",
            "construct-var x0",
            "move-next-sibling",
            "construct-num 0",
            "move-parent",
            "move-next-sibling",
            "construct-num 1",
            "move-next-sibling",
            "construct-var x0",
            "construct-binop mul",
        ]
    }

    fn built() -> AgentSession {
        let mut session = AgentSession::new(AuthorId::new(1));
        for step in factorial_script() {
            assert!(session.apply_text(step).unwrap(), "`{step}` did not apply");
        }
        session
    }

    #[test]
    fn a_session_builds_the_factorial_fixture() {
        let session = built();
        assert_eq!(
            session.state().render(),
            "λx0:Num. if x0 == 0 then 1 else x0 * ⦇⦈"
        );
        assert!(is_well_typed(&session.exp()));
        assert_eq!(session.log().len(), factorial_script().len());
    }

    #[test]
    fn undo_walks_back_to_the_empty_program_and_redo_returns() {
        let mut session = built();
        let head = session.exp();
        while session.can_undo() {
            assert!(session.undo());
        }
        assert!(matches!(session.exp(), Exp::EmptyHole(_)));
        while session.can_redo() {
            assert!(session.redo());
        }
        assert_eq!(session.exp(), head);
    }

    #[test]
    fn every_log_entry_carries_the_session_author_and_a_rising_timestamp() {
        let session = built();
        let mut previous = 0u64;
        for entry in session.entries() {
            assert_eq!(entry.author, AuthorId::new(1));
            assert!(entry.timestamp > previous, "timestamps must strictly rise");
            previous = entry.timestamp;
        }
    }

    #[test]
    fn a_document_round_trips_through_save_and_load() {
        let session = built();
        let dir = std::env::temp_dir().join("nothing-agentapi-session-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("factorial.nothing");
        let path = path.to_str().unwrap();
        session.save(path).unwrap();

        let mut fresh = AgentSession::new(AuthorId::new(2));
        fresh.load(path).unwrap();
        assert_eq!(fresh.exp(), session.exp());
        assert_eq!(fresh.state().render(), session.state().render());
        assert_eq!(fresh.log().len(), session.log().len());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn reset_returns_to_the_empty_program_with_an_empty_log() {
        let mut session = built();
        session.reset();
        assert!(matches!(session.exp(), Exp::EmptyHole(_)));
        assert_eq!(session.log().len(), 0);
        assert!(!session.can_undo());
    }

    #[test]
    fn a_failed_action_leaves_the_log_alone() {
        let mut session = AgentSession::new(AuthorId::new(1));
        assert!(!session.apply(Action::MoveParent));
        assert_eq!(session.log().len(), 0);
    }

    #[test]
    fn the_cursor_path_tracks_movement() {
        let mut session = AgentSession::new(AuthorId::new(1));
        assert_eq!(session.cursor_path(), Vec::<usize>::new());
        assert!(session.apply_text("construct-lam").unwrap());
        assert_eq!(session.cursor_path(), vec![0]);
        assert!(session.apply_text("move-parent").unwrap());
        assert_eq!(session.cursor_path(), Vec::<usize>::new());
    }

    #[test]
    fn an_author_can_be_supplied_per_action() {
        let mut session = AgentSession::new(AuthorId::new(1));
        assert!(session.apply_as(Action::ConstructNum(1), AuthorId::new(9)));
        assert_eq!(session.entries()[0].author, AuthorId::new(9));
        assert_eq!(session.author(), AuthorId::new(1));
    }
}
