//! The action log (Phase 2) and undo/redo built on top of it.
//!
//! Every applied action is appended here with a timestamp and an author
//! ID. Provenance (Phase 10) and structural diff (Phase 9) are meant to
//! read from this log rather than from snapshots of the tree, and undo/redo
//! (below) is the first consumer that actually does so.
//!
//! # Design
//!
//! - [`AuthorId`] is a plain newtype over `u64`. There is no user/identity
//!   system yet (that is not a Phase-2 concern), so it is just an opaque
//!   tag a caller can assign per editing session or per collaborator.
//! - The timestamp is `u64` milliseconds since the Unix epoch, not
//!   [`std::time::SystemTime`]. A `SystemTime` cannot be constructed with a
//!   literal in a test and does not implement `Eq`/`Hash` portably; a plain
//!   `u64` does, round-trips through comparisons trivially, and is what
//!   [`ActionLog::append`] takes directly — the caller supplies it (see
//!   [`now_millis`] for the real-clock case), so replay and tests are
//!   deterministic and never touch the system clock.
//! - Replay is *from the empty program* ([`EditState::empty`], a single
//!   empty hole at the root) *by re-running [`apply_with`]*, not by storing
//!   snapshots. This is deliberate for undo/redo below: truncate-and-replay
//!   is simpler than inverse actions and is always correct, because every
//!   action already carries well-typedness left to right — there is no
//!   broken intermediate state replay could get stuck in.

use crate::act::{Action, EditState};

/// Opaque identifier for whoever performed an action.
///
/// No identity system exists yet; this is a tag a caller assigns (one
/// session, one collaborator, one bot). Deliberately a bare newtype rather
/// than an enum so it never needs to change shape when one is added.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct AuthorId(pub u64);

impl AuthorId {
    pub const fn new(id: u64) -> AuthorId {
        AuthorId(id)
    }
}

/// Milliseconds since the Unix epoch, from the real clock.
///
/// The log itself never calls this — [`ActionLog::append`] takes a
/// timestamp as a parameter precisely so replay and tests stay
/// deterministic — but a real editing session needs *some* way to produce
/// one, so it lives here rather than being reinvented at every call site.
pub fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// One applied action, with its provenance.
#[derive(Clone, PartialEq, Debug)]
pub struct LogEntry {
    pub action: Action,
    /// Milliseconds since the Unix epoch. Injectable (see the module docs)
    /// so replay is deterministic.
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

/// An append-only record of every action applied to a program, in order.
///
/// The log only ever grows at the tail via [`ActionLog::append`] — there is
/// no in-place edit — except [`ActionLog::truncate`], which exists solely
/// for undo/redo's "new action after undo discards the redo tail" rule.
/// [`ActionLog`] does not itself apply actions or know about a "current"
/// program; it is deliberately just data, so [`EditSession`] (or a future
/// provenance/diff reader) can each interpret it their own way.
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

    /// Append one entry to the tail of the log.
    pub fn append(&mut self, action: Action, timestamp: u64, author: AuthorId) {
        self.entries.push(LogEntry::new(action, timestamp, author));
    }

    /// Drop every entry from index `len` onward. A no-op if `len >=
    /// self.len()`. Used by undo/redo to discard the redo tail when a new
    /// action is applied after an undo.
    pub fn truncate(&mut self, len: usize) {
        self.entries.truncate(len);
    }

    /// Replay the first `n` entries (clamped to the log's length) from the
    /// empty program, applying each in turn.
    ///
    /// Every entry replayed here was, by construction (see
    /// [`EditSession::apply`]), an action that applied successfully the
    /// first time against the exact same prefix, so this never encounters
    /// an action that fails to apply — `apply_with`'s fresh-id counter and
    /// the cursor evolve identically both times. It is still a plain loop
    /// over [`EditState::apply_mut`], not a special replay path, because
    /// there is nothing else it needs to do.
    pub fn replay_prefix(&self, n: usize) -> EditState {
        let mut state = EditState::empty();
        for entry in self.entries.iter().take(n) {
            state.apply_mut(entry.action.clone());
        }
        state
    }

    /// Replay the whole log from the empty program.
    pub fn replay(&self) -> EditState {
        self.replay_prefix(self.entries.len())
    }
}

/// An editing session: the current state plus the log it was reached by,
/// with undo/redo over that log.
///
/// Undo and redo are **not** inverse actions. `undo` steps a cursor back
/// one entry and rebuilds the program by [`ActionLog::replay_prefix`] from
/// the empty snapshot; `redo` steps the cursor forward the same way.
/// Simpler than writing an inverse for every [`Action`] variant (what is
/// the inverse of `Delete`, which discards information?) and always
/// correct, because replay is just [`apply_with`] run again — the same
/// mechanism that already guarantees every reachable state is well-typed.
///
/// `cursor` is the number of log entries currently "applied" — i.e. `state
/// == log.replay_prefix(cursor)` is an invariant maintained by every method
/// on this type. A new action applied after one or more undos truncates
/// everything from `cursor` onward before appending, per the spec.
#[derive(Clone, PartialEq, Debug)]
pub struct EditSession {
    log: ActionLog,
    cursor: usize,
    state: EditState,
}

impl EditSession {
    /// A fresh session starting from the empty program, with an empty log.
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

    /// How many of the log's entries are currently applied. Equal to
    /// `log().len()` except while sitting in the middle of an undo/redo
    /// history.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.log.len()
    }

    /// Apply a new action against the current state. On success it is
    /// appended to the log, after first truncating any redo tail left over
    /// from a previous undo. On failure the session is untouched, and
    /// nothing is appended — the log only ever records actions that
    /// actually applied.
    ///
    /// Returns whether the action applied.
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

    /// Step back one entry, replaying from the empty snapshot. Returns
    /// `false` (and leaves the session untouched) if already at the base
    /// snapshot.
    pub fn undo(&mut self) -> bool {
        if !self.can_undo() {
            return false;
        }
        self.cursor -= 1;
        self.state = self.log.replay_prefix(self.cursor);
        true
    }

    /// Step forward one entry from the redo tail, replaying from the empty
    /// snapshot. Returns `false` (and leaves the session untouched) if
    /// there is no redo tail.
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

    /// A pool of actions broad enough that, tried in some order from
    /// almost any cursor position, at least one of them applies. Kept
    /// local to this test module rather than reusing `act`'s private
    /// `every_construction` (which is `#[cfg(test)]`-private to that
    /// module and, being a different crate module, not visible here
    /// anyway).
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

    /// Try the pool in a rotation starting at a random offset, applying
    /// the first action that succeeds. With a pool this broad (leaf
    /// constructions always succeed on a hole, and `Delete` always
    /// succeeds anywhere) this always finds one, so this never loops
    /// forever — but it is bounded by the pool length regardless.
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
            let next = state.apply(action.clone()).expect("just verified this applies");
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
            state = state.apply(action.clone()).expect("just verified this applies");
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
            state = state.apply(action.clone()).expect("just verified this applies");
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

        // Undo all the way back to the empty program.
        for _ in 0..50 {
            assert!(session.can_undo());
            assert!(session.undo());
        }
        assert!(!session.can_undo());
        assert_eq!(session.cursor(), 0);
        assert_eq!(session.exp(), EditState::empty().exp());
        // One more undo is a clean no-op.
        assert!(!session.undo());

        // Redo all the way back to the final state.
        for _ in 0..50 {
            assert!(session.can_redo());
            assert!(session.redo());
        }
        assert!(!session.can_redo());
        assert_eq!(session.cursor(), 50);
        assert_eq!(session.exp(), final_exp);
        // One more redo is a clean no-op.
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

        // Undo three steps, so there is a three-entry redo tail.
        assert!(session.undo());
        assert!(session.undo());
        assert!(session.undo());
        assert_eq!(session.cursor(), 7);
        assert_eq!(session.log().len(), 10);

        // A new action discards the redo tail rather than being inserted
        // into the middle of it.
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
        // At the root of the empty program, `MoveParent` and every sibling
        // move fail cleanly: there is no parent and no sibling.
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
