use nothing_action::act::Action;

use crate::app::Slot;

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Typing {
    pub slot: Slot,
    pub text: String,
    pub committed: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Keystroke {
    pub start: usize,
    pub end: usize,
    pub before: Typing,
    pub after: Typing,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct History {
    log: Vec<Action>,
    keystrokes: Vec<Keystroke>,
    done: usize,
}

impl History {
    pub fn new() -> History {
        History::default()
    }

    pub fn actions(&self) -> &[Action] {
        &self.log
    }

    pub fn applied(&self) -> usize {
        match self.keystrokes.get(self.done) {
            Some(k) => k.start,
            None => self.log.len(),
        }
    }

    pub fn keystrokes(&self) -> usize {
        self.done
    }

    pub fn record(&mut self, actions: &[Action]) {
        let applied = self.applied();
        if applied < self.log.len() {
            self.log.truncate(applied);
            self.keystrokes.truncate(self.done);
        }
        self.log.extend(actions.iter().cloned());
    }

    pub fn close_keystroke(&mut self, start: usize, before: Typing, after: Typing) {
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

    pub fn undo(&mut self) -> Option<(usize, Typing)> {
        if !self.can_undo() {
            return None;
        }
        self.done -= 1;
        let k = &self.keystrokes[self.done];
        Some((k.start, k.before.clone()))
    }

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
