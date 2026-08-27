//! The editor state and the editor-level cursor (Phase 4).
//!
//! # What this module owns
//!
//! [`AppState`] is the whole editor: an [`EditState`] (the zipper cursor
//! plus the fresh-id supply) and the small amount of *editor-level* state
//! that the action calculus deliberately does not model. Everything here is
//! a pure value — there is no terminal, no I/O, and no interior mutability,
//! so every binding in [`crate::keys`] is unit-testable headlessly.
//!
//! # Slots — the one piece of positional state beyond the zipper
//!
//! `KEYS.md` §"Slots, not modes": a binder's *name* and *annotation* are
//! parts of a node that the zipper has no child index for, and
//! `SetBinderId` / `SetAnn` are unreachable without a cursor that can
//! address them. [`Slot`] is that cursor extension. The editor therefore
//! walks a slightly larger tree than the zipper does:
//!
//! | node | editor child 0 | 1 | 2 |
//! |---|---|---|---|
//! | `Lam` | binder **name** (slot) | **annotation** (slot) | body (zipper child 0) |
//! | `Let` | binder **name** (slot) | bound (zipper child 0) | body (zipper child 1) |
//! | everything else | zipper child 0 | 1 | 2 |
//!
//! The four arrow keys walk *this* tree ([`AppState::move_down`],
//! [`AppState::move_up`], [`AppState::move_next`], [`AppState::move_prev`]),
//! which is why binder parts cost no extra bindings. A slot is not a mode:
//! the render marks the name or the type itself (see [`crate::render`]) and
//! the status line always names the slot.
//!
//! # The seams
//!
//! Everything the keyboard does goes through four of them, and a change to
//! the grammar should use them rather than restructure this module:
//!
//! - [`AppState::apply_actions`] is the **only** path from the editor to
//!   the calculus. Every editor-level gesture — the slot-aware arrows,
//!   `Tab`, an operator climb, a name run's delete-and-recommit — expands to
//!   a `Vec<Action>` of *primitive* actions and goes through here, which is
//!   what keeps the log ([`crate::history`]) primitive and one `C-z` equal
//!   to one keystroke.
//! - [`AppState::entry`] is the live token run: the identifier characters
//!   typed since the cursor last moved, the annotation slot's type tokens,
//!   or the binder-name slot's characters. Movement clears it (a run is
//!   defined as "since the cursor last moved") and so does `Esc`.
//!   [`AppState::entry_committed`] says whether the run has already written
//!   something the next keystroke must replace.
//! - [`AppState::hint`] is the status-line feedback channel for keys that
//!   decline. `KEYS.md` §"Which keys can decline" lists exactly three things
//!   that may ever decline — `SetAnn`, `SetBinderId`, `Finish`; everything
//!   else that sets a hint is a key that means nothing where it was pressed.
//! - [`AppState::climb_actions`] is the operator-climbing rule, as a
//!   sequence of `MoveParent`s. A new wrapping key needs a precedence and
//!   nothing else.
//!
//! Candidate *ranking* is not here: it lives in [`crate::complete`], which
//! reads [`AppState::ctx`] and [`AppState::expected_ty`] — the two halves of
//! `act::ctx_and_expected_ty_at` — and needs nothing else from this module.

use nothing_action::act::{Action, EditState, ctx_and_expected_ty_at};
use nothing_action::script::replay_script;
use nothing_action::zipper::{Frame, Zipper, all_positions};
use nothing_core::ctx::Ctx;
use nothing_core::exp::{Exp, Id};
use nothing_core::render::{PREC_APP, Prec, op_prec};
use nothing_core::ty::Ty;

use crate::history::{History, Typing};

/// The recorded action sequence that builds the factorial reference
/// program, shared verbatim with the benchmark harness so the editor's
/// start-up program is the *same* program `nothing-bench` measures.
const FACTORIAL_FIXTURE: &str = include_str!("../../bench/fixtures/factorial.actions");

/// Which part of the focused node the cursor addresses.
///
/// `Slot::Node` — the whole focused expression, the only possibility for
/// every form except `Lam` and `Let`. The other two are the binder parts;
/// see the module docs for the editor-level child table.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum Slot {
    /// The focused expression itself.
    #[default]
    Node,
    /// The binder's name: `λ»x«:Num. e`, `let »x« = e in e'`.
    BinderName,
    /// A lambda's type annotation: `λx:»Num«. e`.
    Annotation,
}

impl Slot {
    /// The status-line name of this slot.
    pub fn label(self) -> &'static str {
        match self {
            Slot::Node => "node",
            Slot::BinderName => "binder name",
            Slot::Annotation => "annotation",
        }
    }
}

/// The whole editor, as a value.
///
/// Cloning is cheap enough to be the norm: every key handler takes a state
/// and returns a new one rather than mutating, which is what makes
/// `(KeyEvent, AppState) -> AppState` testable without a terminal.
#[derive(Clone, PartialEq, Debug)]
pub struct AppState {
    /// The program under edit, plus the cursor and the fresh-id supply.
    pub edit: EditState,
    /// Which part of the focused node the cursor addresses.
    pub slot: Slot,
    /// The live token run: the identifier characters typed since the cursor
    /// last moved, the annotation slot's type tokens, or the binder-name
    /// slot's characters (`KEYS.md` §"Literal entry"). Cleared by every
    /// movement and by `Esc`.
    pub entry: String,
    /// Whether the live run has already written something into the program
    /// that the run's next keystroke must replace. Invariant: false whenever
    /// [`AppState::entry`] is empty. Maintained by [`AppState::clear_entry`],
    /// which every path that ends a run goes through.
    pub entry_committed: bool,
    /// Status-line feedback for the keystroke just processed. Cleared at
    /// the start of every keystroke, so it always describes the last key.
    pub hint: Option<String>,
    /// Set by the quit key; the terminal loop reads it and stops.
    pub quit: bool,
    /// The program this session opened on. Undo replays from here (see
    /// [`crate::history`]), so it is also the floor `C-z` cannot go below.
    base: EditState,
    /// Every primitive action applied since `base`, grouped by keystroke.
    history: History,
}

impl AppState {
    /// Start editing `exp`, cursor at its root.
    pub fn new(exp: Exp) -> AppState {
        AppState::from_edit(EditState::new(exp))
    }

    /// Start editing the empty program, `⦇⦈`.
    pub fn empty() -> AppState {
        AppState::from_edit(EditState::empty())
    }

    /// Open a session on an existing [`EditState`], which becomes the undo
    /// history's base snapshot.
    pub fn from_edit(edit: EditState) -> AppState {
        AppState {
            base: edit.clone(),
            edit,
            slot: Slot::Node,
            entry: String::new(),
            entry_committed: false,
            hint: None,
            quit: false,
            history: History::new(),
        }
    }

    /// The factorial reference program, `λx0:Num. if x0 == 0 then 1 else
    /// x0 * ⦇⦈`, obtained by replaying the benchmark fixture through the
    /// real action calculus — the editor contains no hand-built program and
    /// no parser of its own.
    ///
    /// # Panics
    ///
    /// If the embedded fixture stops replaying cleanly. That is a
    /// compile-time-embedded constant replayed through the one action
    /// parser, so a panic here means the fixture and the calculus have
    /// diverged, which the benchmark would fail on too; the test
    /// `factorial_demo_renders_the_reference_program` pins it.
    pub fn factorial() -> AppState {
        let state = replay_script(FACTORIAL_FIXTURE)
            .expect("the embedded factorial fixture must replay cleanly");
        AppState::new(state.exp())
    }

    /// The in-scope binders at the cursor, outermost first — the order
    /// `KEYS.md` ranks completion candidates by ("then innermost scope").
    ///
    /// [`Ctx`] is a map and cannot be enumerated, so the binders are read
    /// off the zipper's own path, which is where `ctx_and_expected_ty_at`
    /// gets them from too.
    pub fn binders_in_scope(&self) -> Vec<Id> {
        let ctx = self.ctx();
        self.edit
            .zipper
            .path
            .iter()
            .filter_map(|frame| match frame {
                Frame::LamBody(id, _) => Some(*id),
                Frame::LetBody(id, _) => Some(*id),
                _ => None,
            })
            .filter(|id| ctx.lookup(id).is_some())
            .collect()
    }

    /// The cursor.
    pub fn zipper(&self) -> &Zipper {
        &self.edit.zipper
    }

    /// The focused subexpression.
    pub fn focus(&self) -> &Exp {
        &self.edit.zipper.focus
    }

    /// The whole program under edit.
    pub fn program(&self) -> Exp {
        self.edit.exp()
    }

    /// The bindings in scope at the cursor. What completion filters.
    pub fn ctx(&self) -> Ctx {
        ctx_and_expected_ty_at(&self.edit.zipper).0
    }

    /// Would `Enter` unwrap the quarantined expression under the cursor?
    ///
    /// `KEYS.md` requires a quarantine marker on every non-empty hole
    /// reading "fits now — press Enter" exactly when `Finish` would succeed,
    /// and the only honest way to know that is to ask the calculus. The
    /// trial application is discarded, so nothing is logged.
    pub fn finishes(&self) -> bool {
        matches!(self.focus(), Exp::NonEmptyHole(..)) && self.edit.apply(Action::Finish).is_some()
    }

    /// How many `MoveParent`s separate the cursor from the nearest
    /// quarantine it is *inside*, or `None` when it is not inside one.
    ///
    /// The wrapper and its contents are two cursor positions that differ by
    /// two brackets in a long line (`FRICTION.md` #13), and the keystroke
    /// that finishes an expression leaves the cursor on the contents. So the
    /// editor answers the question from in here rather than making the user
    /// walk out and back to ask it: `Enter` finishes the enclosing wrapper
    /// ([`crate::keys`]) and the status line says it fits
    /// ([`crate::render::status_line`]).
    pub fn enclosing_quarantine(&self) -> Option<usize> {
        self.edit
            .zipper
            .path
            .iter()
            .rev()
            .position(|frame| matches!(frame, Frame::NonEmptyHoleBody(_)))
            .map(|steps| steps + 1)
    }

    /// Would `Enter` finish the quarantine the cursor is inside? `None` when
    /// it is not inside one. The trial application is discarded.
    pub fn enclosing_finishes(&self) -> Option<bool> {
        let steps = self.enclosing_quarantine()?;
        let mut actions = vec![Action::MoveParent; steps];
        actions.push(Action::Finish);
        Some(self.apply_actions(&actions).is_some())
    }

    /// How many quarantined expressions the whole program still contains.
    ///
    /// The status line shows this because "am I done?" was a question the
    /// editor previously answered by counting empty holes only, while
    /// insisting on the same breath that there was "no empty hole in this
    /// program" with two `⦇e⦈` on screen (`FRICTION.md` #12).
    pub fn quarantines(&self) -> usize {
        fn count(exp: &Exp) -> usize {
            let here = usize::from(matches!(exp, Exp::NonEmptyHole(..)));
            here + children(exp).iter().map(|c| count(c)).sum::<usize>()
        }
        count(&self.program())
    }

    /// Why `SetBinderId(new)` must not be applied here, if it must not.
    ///
    /// `KEYS.md` §"Which keys can decline" promises the binder slot is
    /// "warned live, before the keystroke lands" when "the identity would
    /// capture or orphan a reference". *Orphaning* the calculus already
    /// declines by itself — a reference left unbound does not synthesise, so
    /// `SetBinderId` fails the well-typedness check. **Capture does not**: it
    /// leaves a perfectly well-typed program that means something else, which
    /// is the one way this editor could silently change what a program says
    /// (`FRICTION.md` #7). So it is checked here, before the action is
    /// offered to the calculus.
    ///
    /// Both directions of the rebinding are conflicts:
    ///
    /// - **capture** — the body already refers to `new`, resolving to some
    ///   outer binder; after the rename those references would point at *this*
    ///   binder instead;
    /// - **escape** — the body refers to this binder's current id, and an
    ///   outer binder shares it, so the references would survive the rename
    ///   by silently re-binding outwards rather than being orphaned.
    ///
    /// Pre-Phase-5 an `Id` *is* the display name, so capture also makes the
    /// outer binder unreachable from the keyboard (`FRICTION.md` #8): the
    /// candidate list offers one `x0` and there is no keystroke that names the
    /// other. Phase 5 separates name from identity and shadowing becomes
    /// expressible; this check is about identity, and stays.
    pub fn rename_conflict(&self, new: Id) -> Option<RenameConflict> {
        let (old, body) = match self.focus() {
            Exp::Lam(id, _, body) => (*id, body.as_ref()),
            // A `let`'s binder scopes over its body only: the bound
            // expression is written in the enclosing scope.
            Exp::Let(id, _, body) => (*id, body.as_ref()),
            _ => return None,
        };
        if new == old {
            return None;
        }
        match free_occurrences(body, new) {
            0 => {}
            captured => return Some(RenameConflict::Capture { id: new, captured }),
        }
        // An outer binder of the same id is what makes this an escape rather
        // than an orphaning; without one the calculus declines on its own.
        match free_occurrences(body, old) {
            0 => None,
            escaping if self.ctx().lookup(&old).is_some() => {
                Some(RenameConflict::Escape { id: old, escaping })
            }
            _ => None,
        }
    }

    /// The type expected at the cursor. `KEYS.md` requires this on the
    /// status line at all times: it is what makes candidate ranking legible
    /// rather than magic.
    pub fn expected_ty(&self) -> Ty {
        ctx_and_expected_ty_at(&self.edit.zipper).1
    }

    /// Attach a status-line hint, consuming and returning the state so key
    /// handlers can write `state.with_hint("…")` in the declining branch.
    pub fn with_hint(mut self, hint: impl Into<String>) -> AppState {
        self.hint = Some(hint.into());
        self
    }

    /// Clear the transient per-keystroke state. Called once at the top of
    /// [`crate::keys::handle_key`].
    pub fn clear_hint(mut self) -> AppState {
        self.hint = None;
        self
    }

    /// Apply a sequence of **primitive** actions, all or nothing.
    ///
    /// This is the single door between the editor and the calculus (see the
    /// module docs). An editor-level gesture — a slot-aware arrow key,
    /// `Tab`, later an operator climb — expands to primitives here, so the
    /// action log never has to learn an editor concept. Returns `None`, and
    /// leaves `self` untouched, if any action in the sequence does not
    /// apply.
    ///
    /// The resulting slot is [`Slot::Node`]: any sequence that moves the
    /// zipper has left whatever binder part the cursor was addressing.
    /// Callers that mean to land in a slot set it afterwards.
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

    /// End the live token run. Keeps the two fields that describe it in
    /// step, which is the invariant on [`AppState::entry_committed`].
    pub fn clear_entry(&mut self) {
        self.entry.clear();
        self.entry_committed = false;
    }

    /// The primitive actions applied since the session opened, including any
    /// redo tail. The benchmark reads this beside the keystroke count:
    /// `KEYS.md` §Coverage asks for both, because the ratio and the
    /// composition answer different questions.
    pub fn actions(&self) -> &[Action] {
        self.history.actions()
    }

    /// How many keystrokes have been applied (undone ones no longer count).
    pub fn keystrokes(&self) -> usize {
        self.history.keystrokes()
    }

    /// The log index a keystroke starting now would begin at. Paired with
    /// [`AppState::close_keystroke`] by [`crate::keys::handle_key`], which is
    /// the only caller.
    pub fn open_keystroke(&self) -> (usize, Typing) {
        (self.history.applied(), self.typing())
    }

    /// Close the keystroke opened by [`AppState::open_keystroke`], making it
    /// one undo step. A keystroke that applied nothing is not recorded.
    pub fn close_keystroke(mut self, opened: (usize, Typing)) -> AppState {
        let after = self.typing();
        self.history.close_keystroke(opened.0, opened.1, after);
        self
    }

    /// The editor-level state undo has to restore alongside the program.
    fn typing(&self) -> Typing {
        Typing {
            slot: self.slot,
            text: self.entry.clone(),
            committed: self.entry_committed,
        }
    }

    /// `C-z` — step back one keystroke by replaying the log prefix that ends
    /// where it began. `None` at the base snapshot.
    pub fn undo(&self) -> Option<AppState> {
        let mut next = self.clone();
        let (prefix, typing) = next.history.undo()?;
        next.rewind_to(prefix, typing);
        Some(next)
    }

    /// `C-r` — step forward one keystroke, the same way.
    pub fn redo(&self) -> Option<AppState> {
        let mut next = self.clone();
        let (prefix, typing) = next.history.redo()?;
        next.rewind_to(prefix, typing);
        Some(next)
    }

    /// Replay `log[..prefix]` from the base snapshot and restore the editor
    /// state that went with it.
    fn rewind_to(&mut self, prefix: usize, typing: Typing) {
        let mut edit = self.base.clone();
        for action in &self.history.actions()[..prefix] {
            // Every action here applied once already against this exact
            // prefix, so it applies again; `apply_mut` reporting false would
            // mean the calculus is not deterministic, which the Phase 2
            // proptests would have caught first.
            edit.apply_mut(action.clone());
        }
        self.edit = edit;
        self.slot = typing.slot;
        self.entry = typing.text;
        self.entry_committed = typing.committed;
    }

    /// Move within the focused node without touching the program: land on
    /// `slot` of the node the cursor is already on.
    fn in_slot(&self, slot: Slot) -> AppState {
        let mut next = self.clone();
        next.slot = slot;
        next.clear_entry();
        next
    }

    /// Does the focused node have binder slots, and which shape?
    fn binder_kind(&self) -> Option<BinderKind> {
        match self.focus() {
            Exp::Lam(..) => Some(BinderKind::Lam),
            Exp::Let(..) => Some(BinderKind::Let),
            _ => None,
        }
    }

    // --- movement over the editor-level tree (KEYS.md §MOVEMENT) ---

    /// `↓` — into editor child 0: the binder name of a `Lam`/`Let`,
    /// otherwise zipper child 0. `None` when there is nothing below.
    pub fn move_down(&self) -> Option<AppState> {
        match self.slot {
            // A slot is a leaf of the editor tree.
            Slot::BinderName | Slot::Annotation => None,
            Slot::Node => match self.binder_kind() {
                Some(_) => Some(self.in_slot(Slot::BinderName)),
                None => self.apply_actions(&[Action::MoveChild(0)]),
            },
        }
    }

    /// `↑` — to the parent. Out of a binder slot that is the node itself;
    /// otherwise `MoveParent`. `None` at the root.
    pub fn move_up(&self) -> Option<AppState> {
        match self.slot {
            Slot::BinderName | Slot::Annotation => Some(self.in_slot(Slot::Node)),
            Slot::Node => self.apply_actions(&[Action::MoveParent]),
        }
    }

    /// `→` — the next editor sibling. Walks a binder's name → annotation →
    /// body without leaving the node, then falls through to
    /// `MoveNextSibling`. `None` on the last child.
    pub fn move_next(&self) -> Option<AppState> {
        match (self.slot, self.binder_kind()) {
            // name → annotation (lambda) / bound expression (let)
            (Slot::BinderName, Some(BinderKind::Lam)) => Some(self.in_slot(Slot::Annotation)),
            (Slot::BinderName, Some(BinderKind::Let)) => {
                self.apply_actions(&[Action::MoveChild(0)])
            }
            // annotation → body
            (Slot::Annotation, _) => self.apply_actions(&[Action::MoveChild(0)]),
            (Slot::BinderName, None) => None,
            (Slot::Node, _) => match self.edit.zipper.path.last() {
                // A binder's body is its last editor child.
                Some(Frame::LamBody(..)) | Some(Frame::LetBody(..)) => None,
                Some(_) => self.apply_actions(&[Action::MoveNextSibling]),
                None => None,
            },
        }
    }

    /// `←` — the previous editor sibling. The inverse of
    /// [`AppState::move_next`]: a binder's body steps back into its
    /// annotation (lambda) or bound expression (let), and the first editor
    /// child of a binder is its name.
    pub fn move_prev(&self) -> Option<AppState> {
        match self.slot {
            Slot::Annotation => Some(self.in_slot(Slot::BinderName)),
            // The first editor child of anything.
            Slot::BinderName => None,
            Slot::Node => match self.edit.zipper.path.last() {
                Some(Frame::LamBody(..)) => Some(
                    self.apply_actions(&[Action::MoveParent])?
                        .in_slot(Slot::Annotation),
                ),
                Some(Frame::LetBound(..)) => Some(
                    self.apply_actions(&[Action::MoveParent])?
                        .in_slot(Slot::BinderName),
                ),
                Some(_) => self.apply_actions(&[Action::MovePrevSibling]),
                None => None,
            },
        }
    }

    /// `Tab` / `S-Tab` — the next (previous) **unfinished** position in
    /// source order, wrapping at the ends.
    ///
    /// Editor-level, and expanded to primitives by [`moves_between`] before
    /// it reaches the calculus: `KEYS.md` §"Rejected" #2 chose this over
    /// auto-advancing after every construction, and worked example 3 shows
    /// it collapsing a run of four `MoveParent`s into one key.
    ///
    /// "Unfinished" is **both** hole kinds, not just the empty one
    /// (`FRICTION.md` #12). A quarantine is the editor's own record of "this
    /// expression does not fit yet"; a navigation key that walks the
    /// program's remaining work and skips exactly the construct that means
    /// "there is remaining work" is answering the wrong question. Empty holes
    /// are still where most of the work is, so they stay in the same source
    /// order — a `⦇e⦈` simply also stops the cursor now.
    ///
    /// `None` when the program contains no hole of either kind: the honest
    /// meaning of "this program is finished".
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

    /// The `MoveParent`s a wrapping key of precedence `prec` performs before
    /// it wraps — `KEYS.md` §"Operator climbing":
    ///
    /// > Before an operator or low-precedence form key wraps, ascend from
    /// > the focus while **(a)** the parent frame is a `BinOp`, `Ap`, or
    /// > `Proj` frame, **(b)** the focus is that frame's **rightmost**
    /// > child, and **(c)** the parent's precedence ≥ the arriving key's.
    ///
    /// Without it, typing `1 * 2 + 3` left to right builds `1 * (2 + 3)`.
    /// The rightmost-child clause confines the rule to the "typing at the
    /// end" case, which is the only case where text intuition applies.
    ///
    /// An **empty hole never climbs**: the matrix's column A says a wrapping
    /// key at `⦇⦈` wraps the hole in place, and that is also what stops `*`
    /// at the right operand of `1 + ⦇⦈` from swallowing the addition.
    ///
    /// Climbing is not a new action — this is a plain `Vec<Action>` that
    /// goes through [`AppState::apply_actions`] like everything else.
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

    /// Which editor-level child of the focused binder is its body: `Lam`'s
    /// is zipper child 0, `Let`'s is child 1. `None` off a binder.
    pub fn binder_body_child(&self) -> Option<usize> {
        match self.focus() {
            Exp::Lam(..) => Some(0),
            Exp::Let(..) => Some(1),
            _ => None,
        }
    }

    /// Leave a binder slot for the binder's body — `KEYS.md`'s
    /// "exit → body, reprocess", and the `.` key.
    ///
    /// "Exit and reprocess is never a refusal": the caller re-dispatches the
    /// character afterwards, so no keystroke is spent on leaving.
    pub fn exit_slot_to_body(&self) -> Option<AppState> {
        let child = self.binder_body_child()?;
        self.apply_actions(&[Action::MoveChild(child)])
    }

    /// Put the cursor on the binder node itself after constructing one.
    ///
    /// `ConstructLam`/`ConstructLet` leave the cursor on the *first empty
    /// child* of the new form — its body, usually — and quarantine may have
    /// left a `⦇…⦈` wrapper in between. `KEYS.md` wants `\` and `;` to land
    /// in the binder-name slot, so this walks the one step back up (or, when
    /// the form itself was quarantined and had no empty child, the one step
    /// down into the wrapper).
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

/// The precedence a frame's parent binds at, for the climb rule — `None`
/// when the frame is not one climbing may cross, which includes the case
/// where the focus is not the parent's rightmost child.
///
/// `Lam`, `Let`, `If` and `NonEmptyHole` frames are absent on purpose: those
/// forms extend as far right as possible, exactly as in a text grammar, so
/// climbing out of them would swallow the form the user is still writing.
fn climbable_prec(frame: &Frame) -> Option<Prec> {
    match frame {
        Frame::BinOpRight(op, _) => Some(op_prec(*op)),
        Frame::ApArg(_) | Frame::ProjBody(_) => Some(PREC_APP),
        _ => None,
    }
}

/// Which binder form the focus is, for the editor-level child list.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BinderKind {
    Lam,
    Let,
}

/// Why a binder cannot take a given identity — see
/// [`AppState::rename_conflict`], which is the only thing that builds one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenameConflict {
    /// The body already refers to `id`, resolving to an outer binder;
    /// renaming would silently re-bind `captured` references to this one.
    Capture { id: Id, captured: usize },
    /// The body refers to this binder's current `id`, which an outer binder
    /// also has: renaming would silently re-bind `escaping` references
    /// outwards instead of orphaning them.
    Escape { id: Id, escaping: usize },
}

impl RenameConflict {
    /// The status-line warning, naming the count — the user's next question
    /// after "why not" is always "how much".
    pub fn explain(self) -> String {
        let plural = |n: usize| if n == 1 { "reference" } else { "references" };
        match self {
            RenameConflict::Capture { id, captured } => format!(
                "{} is already in scope here — naming this binder {} would capture {captured} {}",
                nothing_core::render::render_id(id),
                nothing_core::render::render_id(id),
                plural(captured),
            ),
            RenameConflict::Escape { id, escaping } => format!(
                "{escaping} {} to {} would re-bind to the outer {}",
                plural(escaping),
                nothing_core::render::render_id(id),
                nothing_core::render::render_id(id),
            ),
        }
    }
}

/// Is this expression a hole of either kind — a place the program is not
/// finished? What `Tab` walks.
fn is_unfinished(exp: &Exp) -> bool {
    matches!(exp, Exp::EmptyHole(_) | Exp::NonEmptyHole(..))
}

/// The immediate subexpressions of `exp`, in source order.
fn children(exp: &Exp) -> Vec<&Exp> {
    match exp {
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::EmptyHole(_) => Vec::new(),
        Exp::Lam(_, _, b) | Exp::Proj(_, b) | Exp::NonEmptyHole(_, b) => vec![b],
        Exp::Ap(a, b) | Exp::BinOp(_, a, b) | Exp::Let(_, a, b) | Exp::Pair(a, b) => vec![a, b],
        Exp::If(c, t, e) => vec![c, t, e],
    }
}

/// How many occurrences of `Var(id)` in `exp` are **free** — not bound by a
/// binder of the same id inside `exp`.
///
/// The one piece of scope arithmetic the editor does for itself. It is here
/// rather than in `core` because it answers an editor question (may this
/// keystroke land?) rather than a typing one; `syn`/`ana` already answer the
/// typing question, and they are what catches orphaning.
fn free_occurrences(exp: &Exp, id: Id) -> usize {
    match exp {
        Exp::Var(v) => usize::from(*v == id),
        Exp::Lam(binder, _, _) if *binder == id => 0,
        Exp::Let(binder, bound, body) if *binder == id => {
            // The bound expression is outside the binder's own scope, so it
            // still counts; the body does not.
            free_occurrences(bound, id)
        }
        other => children(other)
            .into_iter()
            .map(|child| free_occurrences(child, id))
            .sum(),
    }
}

/// The child indices from the root down to `z` — the path, stripped of the
/// siblings it carries. Two cursors into the same program are at the same
/// position exactly when these agree.
pub fn index_path(z: &Zipper) -> Vec<usize> {
    z.path.iter().map(Frame::child_index).collect()
}

/// Where `z` sits in a `all_positions` listing of its own program.
fn position_index(positions: &[Zipper], z: &Zipper) -> Option<usize> {
    let target = index_path(z);
    positions.iter().position(|p| index_path(p) == target)
}

/// The primitive movement actions taking the cursor from `from` to `to`
/// within one program: up to the common ancestor, then back down.
///
/// Editor-level jumps (`Tab` today, structural search later) are defined by
/// their destination, but the log records only primitives — this is the
/// expansion that keeps that true.
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
    use nothing_core::render::render;

    /// Where the editor is, ignoring how it got there. Two states with the
    /// same program, cursor and slot are at the same *position* even though
    /// their undo histories differ — which they always do, because history
    /// is a record of the route.
    fn at(state: &AppState) -> (Exp, Vec<usize>, Slot) {
        (state.program(), index_path(state.zipper()), state.slot)
    }

    #[test]
    fn factorial_demo_renders_the_reference_program() {
        let state = AppState::factorial();
        assert_eq!(
            render(&state.program()),
            "λx0:Num. if x0 == 0 then 1 else x0 * ⦇⦈"
        );
        assert!(state.edit.zipper.is_root());
        assert_eq!(state.slot, Slot::Node);
    }

    #[test]
    fn moving_never_changes_the_program() {
        let start = AppState::factorial();
        let program = start.program();
        let mut state = start;
        // Down into the lambda's name, right through its slots, into the
        // body, and back out.
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
        // λx0:Num. if …
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

        // …and back, exactly.
        let back_ann = body.move_prev().expect("body → annotation");
        assert_eq!(at(&back_ann), at(&ann));
        assert_eq!(back_ann.move_prev().as_ref().map(at), Some(at(&name)),);
        assert!(name.move_prev().is_none(), "the name is the first child");
        assert_eq!(name.move_up().as_ref().map(at), Some(at(&lam)));
    }

    #[test]
    fn let_slots_walk_name_bound_body() {
        // let x0 = λx1:?. x1 in x0
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
        // 1 + ⦇⦈ has exactly one hole: Tab from the root lands on it and
        // stays there.
        let state = AppState::new(examples::add_with_empty_hole());
        let hole = state.move_to_hole(true).expect("there is a hole");
        assert!(matches!(hole.focus(), Exp::EmptyHole(_)));
        assert_eq!(hole.zipper().child_index(), Some(1));
        assert_eq!(hole.move_to_hole(true).as_ref().map(at), Some(at(&hole)));
        assert_eq!(hole.move_to_hole(false).as_ref().map(at), Some(at(&hole)));
    }

    #[test]
    fn tab_wraps_and_shift_tab_reverses() {
        // `(⦇⦈, ⦇⦈)` — two holes, so the wrap-around is observable.
        let program = Exp::pair(
            Exp::empty_hole(nothing_core::exp::HoleId::new(0)),
            Exp::empty_hole(nothing_core::exp::HoleId::new(1)),
        );
        let root = AppState::new(program);

        let fst = root.move_to_hole(true).expect("two holes");
        assert_eq!(index_path(fst.zipper()), vec![0]);
        let snd = fst.move_to_hole(true).expect("two holes");
        assert_eq!(index_path(snd.zipper()), vec![1]);
        // Forward from the last hole wraps to the first.
        let wrapped = snd.move_to_hole(true).expect("two holes");
        assert_eq!(index_path(wrapped.zipper()), vec![0]);
        // And backwards is the exact reverse.
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
        let program = examples::clamp_to_one(); // λx0:Num. if x0 < 1 then 1 else x0
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
        // …and the expansion actually gets there.
        let arrived = AppState::from_edit(EditState {
            zipper: from.clone(),
            fresh: nothing_action::act::Fresh::from_program(&program),
        })
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
