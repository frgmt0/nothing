//! Per-keystroke undo, as truncate-and-replay over the primitive log.
//!
//! `KEYS.md` §Coverage: *"undo as truncate-and-replay **per keystroke** (one
//! `C-z` undoes one key, even when the key expanded to several actions)"*.
//! Two things follow, and this module is exactly those two things:
//!
//! 1. **The log is primitive.** Every editor-level gesture — a climb, a
//!    `Tab`, a name-run re-commit — reaches the calculus as a sequence of
//!    `Action`s through [`crate::app::AppState::apply_actions`], and it is
//!    that sequence which is recorded. The log never learns an editor
//!    concept, so Phase 9's diff and Phase 10's provenance can read it.
//! 2. **A keystroke is a span of that log.** [`Keystroke`] records the half
//!    open range of log indices one key produced, plus the [`Typing`] state
//!    the key started and ended in, because the buffer and the binder slot
//!    are not in the program and replay cannot reconstruct them.
//!
//! Undo does not invert anything: it replays the log prefix ending where the
//! keystroke began, from the session's base snapshot. That is Phase 2's own
//! argument (`action::log`) — every action carries well-typedness left to
//! right, so replay can never get stuck, whereas an inverse for `Delete`
//! does not exist.
//!
//! # Why not `action::log::ActionLog`
//!
//! [`ActionLog`](nothing_action::log::ActionLog) replays *from the empty
//! program*, and an editing session starts from whatever program was opened
//! (`AppState::factorial()` opens on the benchmark fixture's output). This
//! log is therefore relative to a base snapshot the session holds, and it
//! carries keystroke spans, which `ActionLog` deliberately does not model.
//! It stays a plain `Vec<Action>` rather than growing timestamps and author
//! ids: those belong to the session layer that will write the log to disk in
//! Phase 8, and inventing them here would put a clock inside a pure
//! function.

use nothing_action::act::Action;

use crate::app::Slot;

/// The editor-level state a keystroke started or ended in: everything the
/// program itself does not record.
///
/// `text` is the live token run (`KEYS.md` §"Literal entry") and `committed`
/// says whether that run has already written something into the program that
/// the next keystroke of the run must replace.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Typing {
    pub slot: Slot,
    pub text: String,
    pub committed: bool,
}

/// One keystroke's worth of log: `log[start..end]`, with the editor state on
/// either side of it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Keystroke {
    /// Index of this keystroke's first action in the log.
    pub start: usize,
    /// One past its last action.
    pub end: usize,
    /// The editor state the keystroke began in — what `C-z` restores.
    pub before: Typing,
    /// The editor state it ended in — what `C-r` restores.
    pub after: Typing,
}

/// The primitive actions applied since the session's base snapshot, grouped
/// into keystrokes, with an undo cursor.
///
/// Invariant: the program currently on screen is the base snapshot with
/// `log[..applied()]` applied to it. When `done == keystrokes.len()` that is
/// the whole log; after an undo it is a prefix, and the tail is the redo
/// history — kept, not discarded, until a new keystroke replaces it.
// `Action` is `PartialEq` but not `Eq` (it carries `i64` payloads through a
// derive chain that stops short of `Eq`), so neither is this.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct History {
    log: Vec<Action>,
    keystrokes: Vec<Keystroke>,
    done: usize,
}

impl History {
    /// An empty history — nothing typed yet.
    pub fn new() -> History {
        History::default()
    }

    /// Every primitive action in the log, including any redo tail.
    pub fn actions(&self) -> &[Action] {
        &self.log
    }

    /// How many of them are currently applied to the program. Equal to
    /// `actions().len()` except while sitting inside an undo.
    pub fn applied(&self) -> usize {
        match self.keystrokes.get(self.done) {
            Some(k) => k.start,
            None => self.log.len(),
        }
    }

    /// How many keystrokes are currently applied. This is the number the
    /// benchmark counts; `applied()` is the action count beside it.
    pub fn keystrokes(&self) -> usize {
        self.done
    }

    /// Record actions that have just been applied to the program.
    ///
    /// Applying anything while inside an undo discards the redo tail first —
    /// the same rule `action::log::EditSession` follows, and the only way
    /// this log ever shrinks.
    pub fn record(&mut self, actions: &[Action]) {
        let applied = self.applied();
        if applied < self.log.len() {
            self.log.truncate(applied);
            self.keystrokes.truncate(self.done);
        }
        self.log.extend(actions.iter().cloned());
    }

    /// Close the keystroke that began at log index `start`, if it changed
    /// anything.
    ///
    /// "Anything" is the program *or* the editor state around it: `:` moves
    /// into the annotation slot without applying an action, and `C-z` must
    /// still take it back, or one `C-z` would not undo one key. What is
    /// deliberately *not* recorded is a keystroke that changed nothing at
    /// all — a declined motion, an unbound key, a run character that matched
    /// no candidate and wrote nothing — because an undo step that does
    /// nothing visible is worse than no undo step.
    pub fn close_keystroke(&mut self, start: usize, before: Typing, after: Typing) {
        // `applied()`, not `log.len()`: a keystroke that was itself an undo
        // leaves the log long and the applied prefix short, and must not be
        // recorded as new history on top of the tail it just stepped off.
        let end = self.applied();
        if end < start || (end == start && before == after) {
            return;
        }
        self.keystrokes.truncate(self.done);
        self.keystrokes.push(Keystroke {
            start,
            end,
            before,
            after,
        });
        self.done = self.keystrokes.len();
    }

    pub fn can_undo(&self) -> bool {
        self.done > 0
    }

    pub fn can_redo(&self) -> bool {
        self.done < self.keystrokes.len()
    }

    /// Step back one keystroke: the log prefix to replay, and the editor
    /// state to restore alongside it.
    pub fn undo(&mut self) -> Option<(usize, Typing)> {
        if !self.can_undo() {
            return None;
        }
        self.done -= 1;
        let k = &self.keystrokes[self.done];
        Some((k.start, k.before.clone()))
    }

    /// Step forward one keystroke, the same way.
    pub fn redo(&mut self) -> Option<(usize, Typing)> {
        if !self.can_redo() {
            return None;
        }
        let k = &self.keystrokes[self.done];
        self.done += 1;
        Some((k.end, k.after.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typing(text: &str) -> Typing {
        Typing {
            slot: Slot::Node,
            text: text.to_string(),
            committed: !text.is_empty(),
        }
    }

    fn keystroke(history: &mut History, actions: &[Action]) {
        let start = history.applied();
        history.record(actions);
        history.close_keystroke(start, typing(""), typing(""));
    }

    #[test]
    fn a_keystroke_spanning_several_actions_undoes_in_one_step() {
        let mut h = History::new();
        keystroke(&mut h, &[Action::ConstructNum(1)]);
        keystroke(
            &mut h,
            &[Action::MoveParent, Action::MoveParent, Action::ConstructIf],
        );
        assert_eq!(h.applied(), 4);
        assert_eq!(h.keystrokes(), 2);

        assert_eq!(h.undo().map(|(n, _)| n), Some(1));
        assert_eq!(h.applied(), 1, "one C-z undid all three actions");
        assert_eq!(h.keystrokes(), 1);
        assert!(h.can_redo());

        assert_eq!(h.redo().map(|(n, _)| n), Some(4));
        assert_eq!(h.applied(), 4);
    }

    #[test]
    fn a_new_keystroke_after_an_undo_discards_the_redo_tail() {
        let mut h = History::new();
        keystroke(&mut h, &[Action::ConstructNum(1)]);
        keystroke(&mut h, &[Action::ConstructNum(2)]);
        h.undo();
        keystroke(&mut h, &[Action::ConstructBool(true)]);

        assert_eq!(
            h.actions(),
            &[Action::ConstructNum(1), Action::ConstructBool(true)]
        );
        assert_eq!(h.applied(), 2);
        assert!(!h.can_redo());
    }

    #[test]
    fn a_keystroke_that_applied_nothing_is_not_undoable() {
        let mut h = History::new();
        keystroke(&mut h, &[]);
        assert!(!h.can_undo());
        assert_eq!(h.applied(), 0);
    }

    #[test]
    fn undo_stops_at_the_base_snapshot_and_redo_at_the_tip() {
        let mut h = History::new();
        keystroke(&mut h, &[Action::ConstructNum(1)]);
        assert!(h.undo().is_some());
        assert!(h.undo().is_none(), "nothing before the base snapshot");
        assert!(h.redo().is_some());
        assert!(h.redo().is_none(), "nothing after the tip");
    }
}
