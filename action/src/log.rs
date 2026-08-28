use crate::act::{Action, EditState};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct AuthorId(pub u64);

impl AuthorId {
    pub const fn new(id: u64) -> AuthorId {
        AuthorId(id)
    }
}

pub fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Clone, PartialEq, Debug)]
pub struct LogEntry {
    pub action: Action,
    pub timestamp: u64,
    pub author: AuthorId,
}

impl LogEntry {
    pub fn new(action: Action, timestamp: u64, author: AuthorId) -> LogEntry {
        LogEntry {
            action,
            timestamp,
            author,
        }
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct ActionLog {
    entries: Vec<LogEntry>,
}

impl ActionLog {
    pub fn new() -> ActionLog {
        ActionLog::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    pub fn append(&mut self, action: Action, timestamp: u64, author: AuthorId) {
        self.entries.push(LogEntry::new(action, timestamp, author));
    }

    pub fn truncate(&mut self, len: usize) {
        self.entries.truncate(len);
    }

    pub fn replay_prefix(&self, n: usize) -> EditState {
        let mut state = EditState::empty();
        for entry in self.entries.iter().take(n) {
            state.apply_mut(entry.action.clone());
        }
        state
    }

    pub fn replay(&self) -> EditState {
        self.replay_prefix(self.entries.len())
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct EditSession {
    log: ActionLog,
    cursor: usize,
    state: EditState,
}

impl EditSession {
    pub fn new() -> EditSession {
        EditSession {
            log: ActionLog::new(),
            cursor: 0,
            state: EditState::empty(),
        }
    }

    pub fn state(&self) -> &EditState {
        &self.state
    }

    pub fn exp(&self) -> nothing_core::exp::Exp {
        self.state.exp()
    }

    pub fn log(&self) -> &ActionLog {
        &self.log
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.log.len()
    }

    pub fn apply(&mut self, action: Action, timestamp: u64, author: AuthorId) -> bool {
        match self.state.apply(action.clone()) {
            Some(next) => {
                self.log.truncate(self.cursor);
                self.log.append(action, timestamp, author);
                self.cursor = self.log.len();
                self.state = next;
                true
            }
            None => false,
        }
    }

    pub fn undo(&mut self) -> bool {
        if !self.can_undo() {
            return false;
        }
        self.cursor -= 1;
        self.state = self.log.replay_prefix(self.cursor);
        true
    }

    pub fn redo(&mut self) -> bool {
        if !self.can_redo() {
            return false;
        }
        self.cursor += 1;
        self.state = self.log.replay_prefix(self.cursor);
        true
    }
}

impl Default for EditSession {
    fn default() -> EditSession {
        EditSession::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::Rng;
    use nothing_core::exp::Op;
    use nothing_core::exp::Side;
    use nothing_core::typing::is_well_typed;

    fn action_pool() -> Vec<Action> {
        vec![
            Action::ConstructNum(1),
            Action::ConstructNum(-3),
            Action::ConstructBool(true),
            Action::ConstructBool(false),
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
            Action::ConstructAp,
            Action::ConstructLam,
            Action::Finish,
            Action::Delete,
            Action::MoveChild(0),
            Action::MoveChild(1),
            Action::MoveChild(2),
            Action::MoveParent,
            Action::MoveNextSibling,
            Action::MovePrevSibling,
        ]
    }

    fn try_one_random_action(state: &EditState, rng: &mut Rng) -> Option<Action> {
        let pool = action_pool();
        let offset = rng.below(pool.len());
        for i in 0..pool.len() {
            let action = pool[(offset + i) % pool.len()].clone();
            if state.apply(action.clone()).is_some() {
                return Some(action);
            }
        }
        None
    }

    #[test]
    fn appending_100_actions_produces_100_entries() {
        let mut state = EditState::empty();
        let mut log = ActionLog::new();
        let mut rng = Rng::new(0xC0FFEE);

        while log.len() < 100 {
            let action = try_one_random_action(&state, &mut rng)
                .expect("the action pool always has something that applies");
            let next = state
                .apply(action.clone())
                .expect("just verified this applies");
            state = next;
            log.append(action, log.len() as u64, AuthorId::new(1));
        }

        assert_eq!(log.len(), 100);
        assert!(is_well_typed(&state.exp()));
    }

    #[test]
    fn replaying_100_actions_from_the_empty_program_reproduces_the_final_program_exactly() {
        let mut state = EditState::empty();
        let mut log = ActionLog::new();
        let mut rng = Rng::new(0xC0FFEE);

        while log.len() < 100 {
            let action = try_one_random_action(&state, &mut rng)
                .expect("the action pool always has something that applies");
            state = state
                .apply(action.clone())
                .expect("just verified this applies");
            log.append(action, log.len() as u64, AuthorId::new(2));
        }

        assert_eq!(log.len(), 100);
        let replayed = log.replay();
        assert_eq!(replayed.exp(), state.exp());
        assert_eq!(replayed.zipper, state.zipper);
    }

    #[test]
    fn replaying_a_prefix_reproduces_the_state_at_that_point() {
        let mut state = EditState::empty();
        let mut log = ActionLog::new();
        let mut rng = Rng::new(42);
        let mut snapshots = Vec::new();

        for _ in 0..20 {
            let action = try_one_random_action(&state, &mut rng).expect("pool applies");
            state = state
                .apply(action.clone())
                .expect("just verified this applies");
            log.append(action, log.len() as u64, AuthorId::new(3));
            snapshots.push(state.exp());
        }

        for (i, expected) in snapshots.iter().enumerate() {
            let replayed = log.replay_prefix(i + 1);
            assert_eq!(&replayed.exp(), expected, "prefix of length {}", i + 1);
        }
    }

    #[test]
    fn empty_log_replays_to_the_empty_program() {
        let log = ActionLog::new();
        assert_eq!(log.replay().exp(), EditState::empty().exp());
    }

    #[test]
    fn new_session_starts_at_the_empty_program_with_an_empty_log() {
        let session = EditSession::new();
        assert_eq!(session.exp(), EditState::empty().exp());
        assert_eq!(session.log().len(), 0);
        assert_eq!(session.cursor(), 0);
        assert!(!session.can_undo());
        assert!(!session.can_redo());
    }

    #[test]
    fn fifty_random_actions_undo_fully_to_empty_and_redo_to_the_same_final_state() {
        let mut session = EditSession::new();
        let mut rng = Rng::new(0xDEADBEEF);
        let mut applied = 0usize;

        while applied < 50 {
            let action = try_one_random_action(session.state(), &mut rng)
                .expect("the action pool always has something that applies");
            let ok = session.apply(action, applied as u64, AuthorId::new(7));
            assert!(ok, "the action was pre-verified to apply");
            applied += 1;
        }

        assert_eq!(session.log().len(), 50);
        assert_eq!(session.cursor(), 50);
        let final_exp = session.exp();
        assert!(is_well_typed(&final_exp));

        for _ in 0..50 {
            assert!(session.can_undo());
            assert!(session.undo());
        }
        assert!(!session.can_undo());
        assert_eq!(session.cursor(), 0);
        assert_eq!(session.exp(), EditState::empty().exp());

        assert!(!session.undo());

        for _ in 0..50 {
            assert!(session.can_redo());
            assert!(session.redo());
        }
        assert!(!session.can_redo());
        assert_eq!(session.cursor(), 50);
        assert_eq!(session.exp(), final_exp);

        assert!(!session.redo());
    }

    #[test]
    fn undo_then_new_action_truncates_the_redo_tail() {
        let mut session = EditSession::new();
        let mut rng = Rng::new(123);

        for i in 0..10 {
            let action = try_one_random_action(session.state(), &mut rng).expect("pool applies");
            assert!(session.apply(action, i, AuthorId::new(1)));
        }
        assert_eq!(session.log().len(), 10);

        assert!(session.undo());
        assert!(session.undo());
        assert!(session.undo());
        assert_eq!(session.cursor(), 7);
        assert_eq!(session.log().len(), 10);

        let new_action = try_one_random_action(session.state(), &mut rng).expect("pool applies");
        assert!(session.apply(new_action.clone(), 999, AuthorId::new(9)));
        assert_eq!(session.log().len(), 8);
        assert_eq!(session.cursor(), 8);
        assert!(!session.can_redo());
        assert_eq!(session.log().entries().last().unwrap().action, new_action);
    }

    #[test]
    fn a_failed_action_does_not_touch_the_log_or_the_cursor() {
        let mut session = EditSession::new();

        assert!(!session.apply(Action::MoveParent, 0, AuthorId::new(1)));
        assert!(!session.apply(Action::MoveNextSibling, 0, AuthorId::new(1)));
        assert!(!session.apply(Action::MovePrevSibling, 0, AuthorId::new(1)));
        assert_eq!(session.log().len(), 0);
        assert_eq!(session.cursor(), 0);
        assert_eq!(session.exp(), EditState::empty().exp());
    }

    #[test]
    fn timestamps_and_authors_round_trip_through_the_log() {
        let mut log = ActionLog::new();
        log.append(Action::ConstructNum(5), 1_000, AuthorId::new(11));
        log.append(Action::Delete, 2_000, AuthorId::new(12));
        assert_eq!(log.entries()[0].timestamp, 1_000);
        assert_eq!(log.entries()[0].author, AuthorId::new(11));
        assert_eq!(log.entries()[1].timestamp, 2_000);
        assert_eq!(log.entries()[1].author, AuthorId::new(12));
    }

    #[test]
    fn now_millis_is_nonzero_and_increases() {
        let a = now_millis();
        assert!(a > 0);
        let b = now_millis();
        assert!(b >= a);
    }
}
