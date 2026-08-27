//! The keyboard grammar (Phase 4), as a pure function.
//!
//! [`handle_key`] is `(KeyEvent, AppState) -> AppState` with no terminal
//! I/O of any kind, which is the whole architecture: every binding is
//! unit-testable headlessly, and `crate::term`'s loop is a shim that reads
//! events, calls this, and draws.
//!
//! `KEYS.md` at the repository root is the authoritative grammar and this
//! module is its implementation. Two of its sections are *normative* and are
//! transcribed here rather than reinterpreted:
//!
//! - **the grammar table** — which key means which construction;
//! - **the printable-character matrix** — what every printable character
//!   does in each of the seven contexts a cursor can be in.
//!
//! ```text
//! MOVEMENT                                   LITERALS & NAMES
//!   ↓ ↑ → ←  editor tree / binder slots        0-9   append to a focused Num,
//!   Tab / S-Tab   next / prev hole, either kind      else ConstructNum(d)
//!                                              ~     negate the focused Num
//! OPERATORS  (climb, then wrap)                a-zA-Z_  name run, commit-live
//!   + - * < =    e op ⦇⦈                             (true/false are candidates)
//! FORMS  (climb, then wrap)                  HOLES & HISTORY
//!   space  e ⦇⦈      \  λ⦇⦈:?. e               Bksp  un-type · ascend · Delete
//!   ?  if e …        ;  let ⦇⦈ = e in ⦇⦈       Del   focus → ⦇⦈
//!   ,  (e, ⦇⦈)       [ ]  fst e / snd e        Enter Finish the ⦇e⦈ on or
//!   !  ⦇e⦈           :  annotation slot              around the cursor
//!                    .  binder body            Esc   end the run
//!                                              C-z / C-r   undo / redo · C-q quit
//! ```
//!
//! # The three rules that make the matrix small
//!
//! 1. **Typing replaces the selection** (`node_key`), except a digit on a
//!    `Num`, which appends (`digit`) — editing `100` into `1000` must not
//!    require a delete first.
//! 2. **A non-empty hole is transparent to typing**: anything typed at
//!    `⦇e⦈` is typed at `e`. Only `!`, `Enter` and `Del`/`Bksp` address the
//!    wrapper. See the first branch of `node_key`.
//! 3. **"Exit and reprocess" is never a refusal**: a character a binder slot
//!    has no meaning for means "I am done here", and it gets its normal
//!    meaning one step out (`exit_and_reprocess`). No keystroke is ever
//!    spent purely on leaving anything.
//!
//! Nothing here refuses a keystroke that `KEYS.md` says is accepted: the
//! calculus auto-quarantines type-inconsistent entry (`<` on `true` gives
//! `⦇true⦈ < ⦇⦈`), so a construction key applies at every focus. The three
//! things that may decline — `SetAnn`, `SetBinderId`, `Finish` — each leave
//! the program untouched and say why on the status line.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use nothing_action::act::Action;
use nothing_core::exp::{Exp, Id, Op, Side};
use nothing_core::render::{PREC_APP, PREC_ATOM, PREC_BINDER, Prec, op_prec};

use crate::annot::{self, Accept};
use crate::app::{AppState, Slot};
use crate::complete;

/// Apply one keystroke. Pure: no I/O, no globals, no clock.
///
/// Key *releases* are ignored here as well as in the terminal loop, so a
/// `KeyEventKind`-reporting terminal cannot double-apply an edit.
///
/// The keystroke is opened and closed around the dispatch so that whatever
/// primitive actions it expanded to — a climb, a `Tab`, a run's
/// delete-and-recommit — undo as **one** step (`KEYS.md` §Coverage).
pub fn handle_key(key: KeyEvent, state: AppState) -> AppState {
    if key.kind == KeyEventKind::Release {
        return state;
    }
    let state = state.clear_hint();

    // The history keys are handled here rather than in `dispatch` because
    // they must not become history themselves: an undo moves the applied
    // prefix backwards, and recording that as a keystroke would make the
    // next `C-z` undo the undo.
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && key.code == KeyCode::Char('z') {
        return or_hint(state.undo(), state, "nothing to undo");
    }
    if ctrl && key.code == KeyCode::Char('r') {
        return or_hint(state.redo(), state, "nothing to redo");
    }

    let opened = state.open_keystroke();
    dispatch(key, state).close_keystroke(opened)
}

/// One arm per binding. Printable characters all land in [`printable`],
/// which is where the matrix lives.
fn dispatch(key: KeyEvent, state: AppState) -> AppState {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match (key.code, ctrl) {
        // --- quit (undo/redo are handled in `handle_key`) ---
        (KeyCode::Char('q'), true) => quit(state),

        // --- movement (KEYS.md §MOVEMENT) ---
        (KeyCode::Down, false) => {
            or_hint(state.move_down(), state, "nothing below: this is a leaf")
        }
        (KeyCode::Up, false) => or_hint(state.move_up(), state, "already at the root"),
        (KeyCode::Right, false) => or_hint(state.move_next(), state, "no next sibling"),
        (KeyCode::Left, false) => or_hint(state.move_prev(), state, "no previous sibling"),
        (KeyCode::Tab, false) => or_hint(state.move_to_hole(true), state, NOTHING_UNFINISHED),
        (KeyCode::BackTab, _) => or_hint(state.move_to_hole(false), state, NOTHING_UNFINISHED),

        // --- holes and history (KEYS.md §"HOLES & HISTORY") ---
        (KeyCode::Backspace, false) => backspace(state),
        (KeyCode::Delete, false) => delete(state),
        (KeyCode::Enter, false) => enter(state),

        // --- the token run (KEYS.md: the program is already committed, so
        // this only clears the buffer) ---
        (KeyCode::Esc, false) => end_run(state),

        // --- everything printable ---
        (KeyCode::Char(c), false) => printable(c, state),

        _ => unbound(key, state),
    }
}

// --- the printable-character matrix ----------------------------------------

/// Dispatch a printable character by *what the cursor is on* — the one thing
/// this grammar is a function of. The columns of `KEYS.md`'s matrix are the
/// three slots plus, inside `node_key`, the shape of the focused node.
fn printable(c: char, state: AppState) -> AppState {
    match state.slot {
        Slot::BinderName => binder_name_key(c, state),
        Slot::Annotation => annotation_key(c, state),
        Slot::Node => node_key(c, state),
    }
}

/// Matrix columns **A** (empty hole), **B** (written expression), **C**
/// (focused `Num`), **D** (mid-name run) and **G** (non-empty hole).
fn node_key(c: char, state: AppState) -> AppState {
    // Column G: a non-empty hole is transparent to typing — you quarantined
    // it to keep editing it. `!` is the exception that addresses the
    // wrapper.
    if matches!(state.focus(), Exp::NonEmptyHole(..)) && c != '!' {
        return match state.apply_actions(&[Action::MoveChild(0)]) {
            Some(mut inner) => {
                // The run survives the descent. A name that had to be
                // quarantined leaves the cursor on the wrapper, and the run
                // that wrote it must go on refining the name inside rather
                // than starting again as a number.
                inner.entry = state.entry.clone();
                inner.entry_committed = state.entry_committed;
                node_key(c, inner)
            }
            None => state.with_hint("cannot look inside this hole"),
        };
    }

    // Column D: identifier characters extend the live run.
    if !state.entry.is_empty() && is_name_char(c) {
        return name_run(c, state);
    }

    // "End run, then as B" — the run's last commit is already in the
    // program, so no keystroke is consumed by the run ending. An
    // *unresolved* buffer is instead offered to `\` and `;` as the new
    // binder's name, which is what the user meant by typing it.
    let pending = (!state.entry_committed && !state.entry.is_empty()).then(|| state.entry.clone());
    let mut state = state;
    state.clear_entry();

    match c {
        '0'..='9' => digit(c, state),
        c if is_name_start(c) => name_run(c, state),

        '+' => operator(Op::Add, state),
        '-' => operator(Op::Sub, state),
        '*' => operator(Op::Mul, state),
        '<' => operator(Op::Lt, state),
        '=' => operator(Op::Eq, state),

        ' ' => wrap(state, PREC_APP, Action::ConstructAp, "application"),
        '\\' => binder(state, Action::ConstructLam, pending, "λ"),
        '?' => wrap(state, PREC_BINDER, Action::ConstructIf, "if"),
        ';' => binder(state, Action::ConstructLet, pending, "let"),
        ',' => wrap(state, PREC_BINDER, Action::ConstructPair, "pair"),
        '[' => wrap(state, PREC_APP, Action::ConstructProj(Side::L), "fst"),
        ']' => wrap(state, PREC_APP, Action::ConstructProj(Side::R), "snd"),
        // Quarantine addresses the focus itself, never an ancestor, so it
        // is given a precedence no frame can out-bind: it never climbs.
        '!' => wrap(
            state,
            PREC_ATOM,
            Action::ConstructNonEmptyHole,
            "quarantine",
        ),

        '~' => negate(state),
        ':' => to_annotation(state),
        '.' => state.with_hint("`.` addresses a binder's body; the cursor is not on a binder"),
        _ => state.with_hint(format!("`{c}` is not bound here")),
    }
}

/// Matrix column **F**: the binder-name slot. Free text, one keystroke per
/// character, plus the three keys that leave it for another part of the same
/// binder.
fn binder_name_key(c: char, state: AppState) -> AppState {
    match c {
        ':' if matches!(state.focus(), Exp::Lam(..)) => to_annotation(state),
        '=' if matches!(state.focus(), Exp::Let(..)) => {
            // → the bound expression: editor child 1 of a `let`, which is
            // zipper child 0.
            match state.apply_actions(&[Action::MoveChild(0)]) {
                Some(next) => next,
                None => state.with_hint("this let has no bound expression"),
            }
        }
        '.' => to_body(state),
        // `~` is the negation key and a binder name is not a number.
        '~' => state.with_hint("`~` negates a number"),
        c if is_name_char(c) => name_binder(c.to_string(), state, true),
        _ => exit_and_reprocess(c, state),
    }
}

/// Matrix column **E**: the annotation slot, which re-issues `SetAnn` with
/// the whole token buffer on every keystroke (see [`crate::annot`]).
fn annotation_key(c: char, state: AppState) -> AppState {
    if c == '.' {
        return to_body(state);
    }
    if c == ':' {
        // The key that *reaches* this slot; pressing it here means nothing
        // and must not throw the cursor out into the body.
        return state.with_hint("already in the annotation slot");
    }
    match annot::accept(&state.entry, c) {
        Accept::Ignore => state.with_hint("there is no `(` to close"),
        Accept::Exit => exit_and_reprocess(c, state),
        Accept::Append | Accept::Swallow => {
            let mut buffer = state.entry.clone();
            buffer.push(c);
            set_ann(buffer, state)
        }
    }
}

// --- literal entry ---------------------------------------------------------

/// Matrix row **digit**. Stateless, and that is the point: a digit on a
/// focused `Num(n)` re-issues `ConstructNum(n·10 ± d)`, anywhere else
/// `ConstructNum(d)`. `4` then `2` gives `42` because `42 = 4·10 + 2`, not
/// because a buffer was open — come back a week later and type `7` and you
/// get `427`, which is also the right answer to "extend this number".
fn digit(c: char, state: AppState) -> AppState {
    let d = i64::from(c.to_digit(10).expect("matched against 0-9"));
    let action = match state.focus() {
        Exp::Num(n) => {
            let extended = if *n < 0 {
                n.checked_mul(10).and_then(|shifted| shifted.checked_sub(d))
            } else {
                n.checked_mul(10).and_then(|shifted| shifted.checked_add(d))
            };
            match extended {
                Some(value) => Action::ConstructNum(value),
                None => return state.with_hint("that number does not fit in 64 bits"),
            }
        }
        _ => Action::ConstructNum(d),
    };
    apply_or_hint(state, &[action], "a number does not fit here")
}

/// Matrix row **`~`**: negate the focused number. `-` stays subtraction —
/// if `-` negated, `1 - 2` would be untypable.
fn negate(state: AppState) -> AppState {
    match state.focus() {
        Exp::Num(n) => match n.checked_neg() {
            Some(negated) => apply_or_hint(
                state,
                &[Action::ConstructNum(negated)],
                "a number does not fit here",
            ),
            None => state.with_hint("that number cannot be negated"),
        },
        _ => state.with_hint("`~` negates a number"),
    }
}

/// One keystroke of a name run, committed live.
///
/// The run's previous commit is replaced rather than appended to: the
/// expansion is `Delete` + the new construction, which is the anchored
/// re-derivation `KEYS.md` describes, expressed in primitive actions. You
/// are never wrong for more than one keystroke and you never press Enter to
/// accept.
fn name_run(c: char, state: AppState) -> AppState {
    let mut buffer = state.entry.clone();
    buffer.push(c);
    commit_run(buffer, state)
}

/// Commit `buffer`'s top-ranked candidate over the run's previous commit.
fn commit_run(buffer: String, state: AppState) -> AppState {
    let committed = state.entry_committed;
    let Some(candidate) = complete::best(&state, &buffer) else {
        // Nothing matches. A free variable has no meaning to quarantine —
        // `ConstructVar` on an out-of-scope id returns `None` — so this is
        // the one place typing does not write to the program. Anything this
        // run already committed is taken back out, leaving the hole the run
        // started from.
        let mut next = match (committed, state.apply_actions(&[Action::Delete])) {
            (true, Some(deleted)) => deleted,
            _ => state,
        };
        next.entry = buffer.clone();
        next.entry_committed = false;
        return next.with_hint(format!("no name in scope starts with `{buffer}`"));
    };

    let mut actions = Vec::new();
    if committed {
        actions.push(Action::Delete);
    }
    actions.push(candidate.action());

    match state.apply_actions(&actions) {
        Some(mut next) => {
            next.entry = buffer;
            next.entry_committed = true;
            next
        }
        None => state.with_hint(format!("`{}` does not apply here", candidate.name)),
    }
}

/// Take one character back off the run and re-commit what is left.
fn run_backspace(state: AppState) -> AppState {
    let mut buffer = state.entry.clone();
    buffer.pop();
    if !buffer.is_empty() {
        return commit_run(buffer, state);
    }
    let mut next = match (
        state.entry_committed,
        state.apply_actions(&[Action::Delete]),
    ) {
        (true, Some(deleted)) => deleted,
        _ => state,
    };
    next.clear_entry();
    next
}

/// Write the binder-name buffer, and with it the binder's identity.
///
/// Pre-Phase-5 there is no name table: the projection renders a binder as
/// `x<id>` (`core::render::render_id`), so the slot resolves the *digits* of
/// what was typed to `SetBinderId`. `KEYS.md`: when Phase 5 lands this
/// becomes a name-table write and not one binding changes.
fn name_binder(text: String, state: AppState, append: bool) -> AppState {
    let mut buffer = if append {
        state.entry.clone()
    } else {
        String::new()
    };
    buffer.push_str(&text);

    let digits: String = buffer.chars().filter(char::is_ascii_digit).collect();
    let Ok(id) = digits.parse::<u64>() else {
        let mut next = state;
        next.slot = Slot::BinderName;
        next.entry = buffer;
        next.entry_committed = false;
        return next
            .with_hint("pre-Phase-5 a binder is identified by the digits in its name, as in `x0`");
    };

    // One of the three things that may decline, and the half of it the
    // calculus cannot see: capture leaves a *well-typed* program that means
    // something else. `KEYS.md` promises this is "warned live, before the
    // keystroke lands", so the warning happens instead of the edit and the
    // slot stays open for the next digit.
    if let Some(conflict) = state.rename_conflict(Id::new(id)) {
        let mut next = state;
        next.slot = Slot::BinderName;
        next.entry = buffer;
        next.entry_committed = false;
        return next.with_hint(conflict.explain());
    }

    match state.apply_actions(&[Action::SetBinderId(Id::new(id))]) {
        Some(mut next) => {
            next.slot = Slot::BinderName;
            next.entry = buffer;
            next.entry_committed = true;
            next
        }
        // The other half: a reference this binder was holding up would be
        // left unbound, which does not synthesise, so the calculus declines.
        None => {
            let mut next = state;
            next.slot = Slot::BinderName;
            next.entry = buffer;
            next.entry_committed = false;
            next.with_hint(format!("x{id} would leave a reference unbound here"))
        }
    }
}

/// Re-issue `SetAnn` from the annotation buffer.
fn set_ann(buffer: String, state: AppState) -> AppState {
    let ty = annot::parse(&buffer);
    match state.apply_actions(&[Action::SetAnn(ty.clone())]) {
        Some(mut next) => {
            next.slot = Slot::Annotation;
            next.entry = buffer;
            next.entry_committed = true;
            next
        }
        // One of the three things that may decline: a type is not an
        // expression, so there is nothing to quarantine. The slot stays
        // open, so the next keystroke can still reach a type that fits.
        None => {
            let mut next = state;
            next.entry = buffer;
            next.entry_committed = false;
            next.with_hint(format!("`{ty}` would leave the body untypable"))
        }
    }
}

// --- forms -----------------------------------------------------------------

/// Climb, then wrap the focus into a new form.
fn wrap(state: AppState, prec: Prec, action: Action, what: &str) -> AppState {
    let mut actions = state.climb_actions(prec);
    actions.push(action);
    apply_or_hint(state, &actions, &format!("{what} does not apply here"))
}

/// `\` and `;`: wrap into a binder and land in its **name** slot.
///
/// The construction itself leaves the cursor on the new form's first empty
/// child; `KEYS.md` wants the name, so this steps back onto the binder (see
/// [`AppState::focus_binder`]). An unresolved name run is carried into the
/// slot: typing `total` at a hole where nothing matches and then `;` means
/// `let total = ⦇⦈ in ⦇⦈`.
fn binder(state: AppState, action: Action, pending: Option<String>, what: &str) -> AppState {
    let mut actions = state.climb_actions(PREC_BINDER);
    actions.push(action);
    let Some(next) = state.apply_actions(&actions) else {
        return state.with_hint(format!("{what} does not apply here"));
    };
    let Some(mut binder) = next.focus_binder() else {
        return next.with_hint(format!("{what} was built but its binder is unreachable"));
    };
    binder.slot = Slot::BinderName;
    match pending {
        Some(text) => name_binder(text, binder, false),
        None => binder,
    }
}

/// Climb, then wrap with a binary operator.
fn operator(op: Op, state: AppState) -> AppState {
    wrap(
        state,
        op_prec(op),
        Action::ConstructBinOp(op),
        "this operator",
    )
}

// --- slots -----------------------------------------------------------------

/// `:` — address the focused lambda's annotation.
fn to_annotation(state: AppState) -> AppState {
    if !matches!(state.focus(), Exp::Lam(..)) {
        return state.with_hint("annotations live on lambdas");
    }
    let mut next = state;
    next.slot = Slot::Annotation;
    next.entry = String::new();
    next.entry_committed = false;
    next
}

/// `.` — address the focused binder's body.
fn to_body(state: AppState) -> AppState {
    match state.exit_slot_to_body() {
        Some(next) => next,
        None => state.with_hint("the cursor is not on a binder"),
    }
}

/// A character a slot has no meaning for: leave the slot for the binder's
/// body and give the character its ordinary meaning there.
fn exit_and_reprocess(c: char, state: AppState) -> AppState {
    match state.exit_slot_to_body() {
        Some(next) => printable(c, next),
        None => state.with_hint(format!("`{c}` means nothing in this slot")),
    }
}

// --- holes and history -----------------------------------------------------

/// `Bksp` — un-type one character of a run, one digit of a number, or one
/// step up out of an empty hole; otherwise `Delete`.
///
/// Ascending out of an empty hole before deleting anything is deliberate:
/// the first `Bksp` shows you, highlighted, what the second one will destroy.
fn backspace(state: AppState) -> AppState {
    match state.slot {
        Slot::BinderName => {
            let mut buffer = state.entry.clone();
            buffer.pop();
            let mut next = state;
            next.entry = buffer;
            next.entry_committed = false;
            next
        }
        Slot::Annotation => {
            let mut buffer = state.entry.clone();
            buffer.pop();
            set_ann(buffer, state)
        }
        Slot::Node => {
            if !state.entry.is_empty() {
                return run_backspace(state);
            }
            match *state.focus() {
                Exp::Num(n) if n / 10 != 0 => apply_or_hint(
                    state,
                    &[Action::ConstructNum(n / 10)],
                    "a number does not fit here",
                ),
                Exp::EmptyHole(_) => or_hint(
                    state.move_up(),
                    state,
                    "the whole program is one empty hole",
                ),
                _ => delete(state),
            }
        }
    }
}

/// `Del` — replace the focus with an empty hole.
fn delete(state: AppState) -> AppState {
    if state.slot != Slot::Node {
        return state
            .with_hint("Del removes an expression — press ↑ to leave the binder slot first");
    }
    apply_or_hint(state, &[Action::Delete], "nothing to delete here")
}

/// `Enter` — `Finish` the quarantine the cursor is on **or inside**,
/// otherwise jump to the next unfinished position.
///
/// The "or inside" is `FRICTION.md` #10: the keystroke that finally makes a
/// quarantined expression fit leaves the cursor *in* the wrapper, and
/// standing on the wrapper to press `Enter` cost `↑` `↓` and a re-read of
/// `KEYS.md` — three keystrokes to undo something the editor already knew was
/// repaired. The climb is expanded to `MoveParent`s here, so the log stays
/// primitive and one `C-z` still undoes one key.
fn enter(state: AppState) -> AppState {
    if state.slot == Slot::Node && matches!(state.focus(), Exp::NonEmptyHole(..)) {
        return match state.apply_actions(&[Action::Finish]) {
            Some(next) => next,
            None => {
                let expected = state.expected_ty();
                state.with_hint(format!("does not fit yet: expected {expected}"))
            }
        };
    }

    if state.slot == Slot::Node
        && let Some(steps) = state.enclosing_quarantine()
    {
        let ups = vec![Action::MoveParent; steps];
        let mut actions = ups.clone();
        actions.push(Action::Finish);
        return match state.apply_actions(&actions) {
            Some(next) => next,
            None => {
                // Saying why beats teleporting to an unrelated hole: from
                // inside a quarantine, the quarantine is the thing `Enter`
                // is about.
                let expected = match state.apply_actions(&ups) {
                    Some(wrapper) => wrapper.expected_ty().to_string(),
                    None => "its context".to_string(),
                };
                state.with_hint(format!(
                    "the ⦇⦈ around the cursor does not fit yet: expected {expected}"
                ))
            }
        };
    }

    or_hint(state.move_to_hole(true), state, NOTHING_UNFINISHED)
}

/// What `Tab`, `S-Tab` and `Enter` say when there is nowhere left to go.
///
/// It used to read "no empty hole in this program" — said, verbatim, with two
/// quarantines on screen (`FRICTION.md` #12). Now both hole kinds are walked,
/// so the message can be the true one.
const NOTHING_UNFINISHED: &str = "nothing unfinished: this program has no holes";

// --- plumbing --------------------------------------------------------------

/// Ask for the quit key to be honoured. The terminal loop owns the actual
/// exit; this only records the request, so quitting is testable.
fn quit(mut state: AppState) -> AppState {
    state.quit = true;
    state
}

/// End the live token run. Committed-live entry means the program already
/// holds what was typed, so there is nothing to undo here.
fn end_run(mut state: AppState) -> AppState {
    if state.entry.is_empty() {
        return state;
    }
    state.clear_entry();
    state
}

/// A key that is not bound: change nothing, say so.
fn unbound(key: KeyEvent, state: AppState) -> AppState {
    state.with_hint(format!("{} is not bound", describe(key)))
}

/// Take the moved-to state, or leave the state alone with a hint saying why
/// the motion declined.
fn or_hint(moved: Option<AppState>, state: AppState, why: &str) -> AppState {
    match moved {
        Some(next) => next,
        None => state.with_hint(why),
    }
}

/// Apply an expansion, or explain why it did not.
fn apply_or_hint(state: AppState, actions: &[Action], why: &str) -> AppState {
    or_hint(state.apply_actions(actions), state, why)
}

/// May `c` start a name? Letters and `_`; digits cannot, because they are
/// the number path.
fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

/// May `c` continue a name?
fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// A key's name for the status line.
fn describe(key: KeyEvent) -> String {
    let mut name = String::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        name.push_str("C-");
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        name.push_str("M-");
    }
    match key.code {
        KeyCode::Char(' ') => name.push_str("space"),
        KeyCode::Char(c) => name.push(c),
        other => name.push_str(&format!("{other:?}").to_lowercase()),
    }
    name
}

/// Build a plain key event. Test-facing, and the terminal loop needs
/// nothing like it, so it lives here rather than in a test module.
pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Build a control-modified key event.
pub fn ctrl(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::index_path;
    use nothing_core::examples;
    use nothing_core::render::render;

    /// Type a string of printable characters, one keystroke each.
    fn type_chars(text: &str, state: AppState) -> AppState {
        text.chars()
            .fold(state, |state, c| handle_key(key(KeyCode::Char(c)), state))
    }

    /// Type into the empty program and render the result.
    fn typed(text: &str) -> String {
        render(&type_chars(text, AppState::empty()).program())
    }

    // --- the spec's own criterion ---

    #[test]
    fn one_plus_two_is_three_keystrokes() {
        let state = type_chars("1+2", AppState::empty());
        assert_eq!(render(&state.program()), "1 + 2");
        assert_eq!(state.keystrokes(), 3);
        assert_eq!(state.actions().len(), 3, "no hidden actions");
    }

    // --- literals ---

    #[test]
    fn digits_extend_the_focused_number() {
        assert_eq!(typed("427"), "427");
        assert_eq!(typed("1+23"), "1 + 23");
        // ...and a digit on a written expression that is not a number
        // replaces it, per "typing replaces the selection".
        let state = type_chars("1+2", AppState::empty());
        let state = handle_key(key(KeyCode::Up), state); // the whole `1 + 2`
        assert_eq!(render(&type_chars("9", state).program()), "9");
        // Application binds tighter than `+`, so space at the `2` does not
        // climb out of the addition; `2` is not a function, so it is
        // quarantined rather than the keystroke being refused.
        assert_eq!(typed("1+2 3"), "1 + ⦇2⦈ 3");
    }

    #[test]
    fn a_digit_typed_a_week_later_still_extends_the_number() {
        // The stateless-number claim: leave the node and come back.
        let state = type_chars("42", AppState::empty());
        let state = handle_key(key(KeyCode::Esc), state);
        assert_eq!(render(&type_chars("7", state).program()), "427");
    }

    #[test]
    fn tilde_negates_and_minus_stays_subtraction() {
        assert_eq!(typed("2~"), "-2");
        assert_eq!(typed("2~5"), "-25", "digits extend a negative number");
        assert_eq!(typed("1-2"), "1 - 2");
    }

    #[test]
    fn backspace_drops_one_digit_then_deletes() {
        let state = type_chars("427", AppState::empty());
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(render(&state.program()), "42");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(render(&state.program()), "4");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(render(&state.program()), "⦇⦈");
    }

    #[test]
    fn backspace_ascends_out_of_an_empty_hole_before_deleting_anything() {
        // The first Bksp shows you, highlighted, what the second destroys.
        let state = type_chars("1+", AppState::empty());
        assert!(matches!(state.focus(), Exp::EmptyHole(_)));
        let up = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(render(&up.program()), "1 + ⦇⦈", "nothing destroyed yet");
        assert!(
            matches!(up.focus(), Exp::BinOp(..)),
            "the whole `+` is now selected"
        );
        let gone = handle_key(key(KeyCode::Backspace), up);
        assert_eq!(render(&gone.program()), "⦇⦈");
    }

    #[test]
    fn backspace_un_types_a_name_one_character_at_a_time() {
        let state = type_chars("\\x0:n.x0", AppState::empty());
        assert_eq!(render(&state.program()), "λx0:Num. x0");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(state.entry, "x", "one character of the run is gone");
        assert_eq!(
            render(&state.program()),
            "λx0:Num. x0",
            "and `x` still names x0"
        );
        let state = handle_key(key(KeyCode::Backspace), state);
        assert!(state.entry.is_empty());
        assert_eq!(
            render(&state.program()),
            "λx0:Num. ⦇⦈",
            "the run wrote nothing"
        );
    }

    #[test]
    fn backspace_in_the_annotation_slot_drops_a_token() {
        let state = type_chars("\\x0:n>n", AppState::empty());
        assert_eq!(render(&state.program()), "λx0:Num -> Num. ⦇⦈");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(render(&state.program()), "λx0:Num -> ?. ⦇⦈");
        let state = handle_key(key(KeyCode::Backspace), state);
        assert_eq!(render(&state.program()), "λx0:Num. ⦇⦈");
        assert_eq!(state.slot, Slot::Annotation, "still annotating");
    }

    #[test]
    fn delete_replaces_the_focus_with_a_hole() {
        let state = type_chars("1+2", AppState::empty());
        let state = handle_key(key(KeyCode::Delete), state);
        assert_eq!(render(&state.program()), "1 + ⦇⦈");
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Delete), state);
        assert_eq!(render(&state.program()), "⦇⦈");
    }

    #[test]
    fn enter_jumps_to_the_next_hole_when_there_is_nothing_to_finish() {
        let state = type_chars("\\x0:n.?", AppState::empty());
        let state = handle_key(key(KeyCode::Enter), state);
        assert!(matches!(state.focus(), Exp::EmptyHole(_)));
        assert_eq!(index_path(state.zipper()), vec![0, 1], "the then-branch");
    }

    // --- names ---

    #[test]
    fn a_name_run_commits_live_and_refines() {
        // λx0:Num. λx1:Num. ⦇⦈ — then type `x`, which commits *something*
        // immediately, and `1`, which refines it.
        let state = type_chars("\\x0:n.\\x1:n.", AppState::empty());
        let after_x = type_chars("x", state);
        assert!(
            matches!(after_x.focus(), Exp::Var(_)),
            "the first keystroke of a run already wrote a variable"
        );
        let after_x1 = type_chars("1", after_x);
        assert_eq!(
            render(&after_x1.program()),
            "λx0:Num. λx1:Num. x1",
            "the second keystroke re-derived the commit"
        );
        assert_eq!(after_x1.entry, "x1");
    }

    #[test]
    fn an_unmatched_name_leaves_the_program_alone() {
        let state = type_chars("\\x0:n.", AppState::empty());
        let before = state.program();
        let after = type_chars("zz", state);
        assert_eq!(after.program(), before, "a free variable has no meaning");
        assert_eq!(after.entry, "zz");
        assert!(!after.entry_committed);
        assert!(after.hint.is_some(), "and the status line says so");
    }

    #[test]
    fn an_unresolved_run_becomes_the_new_binders_name() {
        // KEYS.md: "Pressing `\` or `;` next uses the buffer as the new
        // binder's name, which is what you meant."
        let state = type_chars("total7;", AppState::empty());
        assert_eq!(render(&state.program()), "let x7 = ⦇⦈ in ⦇⦈");
        assert_eq!(state.slot, Slot::BinderName);
        assert_eq!(state.entry, "total7");
    }

    #[test]
    fn true_and_false_are_candidates_not_keys() {
        assert_eq!(typed("t"), "true");
        assert_eq!(typed("f"), "false");
        assert_eq!(typed("?t"), "if true then ⦇⦈ else ⦇⦈");
    }

    // --- operators, forms and climbing ---

    #[test]
    fn operators_climb_so_left_to_right_typing_means_what_it_says() {
        assert_eq!(typed("1*2+3"), "1 * 2 + 3");
        assert_eq!(typed("1+2*3"), "1 + 2 * 3");
        assert_eq!(typed("1+2+3"), "1 + 2 + 3");
        // `==` climbs out of the `<` and then finds a Bool where it wants a
        // Num, so the comparison is quarantined — climbed, then wrapped.
        assert_eq!(typed("1<2=3"), "⦇1 < 2⦈ == 3");
    }

    #[test]
    fn climbing_never_crosses_a_binder_or_a_conditional() {
        // The `then` branch of an `if` extends as far right as possible.
        let state = type_chars("?t", AppState::empty());
        let state = handle_key(key(KeyCode::Tab), state);
        assert_eq!(
            render(&type_chars("1+2", state).program()),
            "if true then 1 + 2 else ⦇⦈"
        );
        assert_eq!(typed("\\x0:n.1+2"), "λx0:Num. 1 + 2");
    }

    #[test]
    fn an_empty_hole_wraps_in_place_rather_than_climbing() {
        // `1 + ⦇⦈` with the cursor in the hole: `*` must not swallow the
        // addition. (Constructing an operator *at* a hole leaves the cursor
        // on its left operand, which is where the `2` then lands.)
        assert_eq!(typed("1+*2"), "1 + 2 * ⦇⦈");
    }

    #[test]
    fn application_climbs_left_associatively() {
        // `f 1` then space gives `f 1 ⦇⦈`, because the rule is ≥, not >.
        let state = type_chars("\\x0:n>n>n.x0 1 2", AppState::empty());
        assert_eq!(render(&state.program()), "λx0:Num -> Num -> Num. x0 1 2");
    }

    #[test]
    fn every_form_key_builds_its_form() {
        // `1` is not a function, so applying it quarantines it rather than
        // refusing the keystroke — KEYS.md's own example.
        assert_eq!(typed("1 "), "⦇1⦈ ⦇⦈");
        assert_eq!(typed("\\"), "λx0:?. ⦇⦈");
        assert_eq!(typed("1?"), "if ⦇1⦈ then ⦇⦈ else ⦇⦈");
        assert_eq!(typed("1;"), "let x0 = 1 in ⦇⦈");
        assert_eq!(typed("1,2"), "(1, 2)");
        assert_eq!(typed("1["), "fst ⦇1⦈");
        assert_eq!(typed("1]"), "snd ⦇1⦈");
        assert_eq!(typed("1!"), "⦇1⦈");
    }

    // --- binder slots ---

    #[test]
    fn a_lambda_is_named_and_annotated_from_the_slots() {
        let state = type_chars("\\", AppState::empty());
        assert_eq!(state.slot, Slot::BinderName, "λ lands on the name");
        let state = type_chars("x0", state);
        let state = type_chars(":", state);
        assert_eq!(state.slot, Slot::Annotation);
        let state = type_chars("n>n", state);
        assert_eq!(render(&state.program()), "λx0:Num -> Num. ⦇⦈");
        let state = type_chars(".", state);
        assert_eq!(state.slot, Slot::Node);
        assert!(matches!(state.focus(), Exp::EmptyHole(_)));
    }

    #[test]
    fn the_annotation_slot_commits_on_every_keystroke() {
        let state = type_chars("\\x0:n", AppState::empty());
        assert_eq!(render(&state.program()), "λx0:Num. ⦇⦈");
        let state = type_chars(">", state);
        assert_eq!(render(&state.program()), "λx0:Num -> ?. ⦇⦈");
        let state = type_chars("n", state);
        assert_eq!(render(&state.program()), "λx0:Num -> Num. ⦇⦈");
    }

    #[test]
    fn an_annotation_that_would_break_the_body_declines_visibly() {
        // λx0:?. x0 + 1 — annotating x0 as Bool leaves the body untypable,
        // and a type is not an expression, so there is nothing to
        // quarantine.
        let state = type_chars("\\x0:.x0+1", AppState::empty());
        assert_eq!(render(&state.program()), "λx0:?. x0 + 1");
        let state = handle_key(key(KeyCode::Up), state); // the `+`
        let state = handle_key(key(KeyCode::Up), state); // the lambda
        let state = type_chars(":b", state);
        assert_eq!(
            render(&state.program()),
            "λx0:?. x0 + 1",
            "the program is untouched"
        );
        assert_eq!(state.slot, Slot::Annotation, "the slot stays open");
        assert!(state.hint.unwrap().contains("Bool"));
    }

    #[test]
    fn a_let_names_then_binds_then_bodies() {
        let state = type_chars(";x0=1", AppState::empty());
        assert_eq!(render(&state.program()), "let x0 = 1 in ⦇⦈");
        // The bound expression, not the body: `=` moves into child 0.
        assert_eq!(index_path(state.zipper()), vec![0]);
    }

    #[test]
    fn a_character_a_slot_does_not_understand_exits_and_is_reprocessed() {
        // `+` in the annotation slot means nothing there, so it means what
        // it always means, one step out — and costs one keystroke, not two.
        // Entering the slot does not disturb the annotation already there.
        let state = type_chars("\\x0:n.1", AppState::empty());
        let state = handle_key(key(KeyCode::Up), state);
        let state = type_chars(":", state);
        let state = type_chars("+", state);
        assert_eq!(render(&state.program()), "λx0:Num. 1 + ⦇⦈");
    }

    // --- quarantine ---

    #[test]
    fn a_type_inconsistent_entry_is_quarantined_rather_than_refused() {
        // `<` on `true` and space on `1`: KEYS.md's own two examples.
        assert_eq!(typed("t<"), "⦇true⦈ < ⦇⦈");
        assert_eq!(typed("1 "), "⦇1⦈ ⦇⦈");
    }

    #[test]
    fn a_non_empty_hole_is_transparent_to_typing() {
        let state = type_chars("1!", AppState::empty());
        assert_eq!(render(&state.program()), "⦇1⦈");
        // A digit lands on the `1` inside, not on the wrapper — where the
        // ordinary append rule then applies to it.
        let state = type_chars("2", state);
        assert_eq!(render(&state.program()), "⦇12⦈");
        // ...but `!` addresses the wrapper.
        let state = type_chars("!", state);
        assert_eq!(render(&state.program()), "⦇⦇12⦈⦈");
    }

    #[test]
    fn enter_finishes_a_quarantined_expression_that_now_fits() {
        // `1 + ⦇true⦈`, edited until it fits, then finished.
        let state = AppState::new(examples::add_with_non_empty_hole());
        // The program has no *empty* hole; Tab walks both kinds, so it lands
        // on the quarantine, which is the only unfinished thing in it.
        let state = handle_key(key(KeyCode::Tab), state);
        assert!(matches!(state.focus(), Exp::NonEmptyHole(..)));

        let refused = handle_key(key(KeyCode::Enter), state.clone());
        assert!(
            matches!(refused.focus(), Exp::NonEmptyHole(..)),
            "true still does not fit"
        );
        assert!(refused.hint.unwrap().contains("does not fit yet"));

        let state = type_chars("2", state); // types inside the hole
        let state = handle_key(key(KeyCode::Up), state); // back onto the wrapper
        let state = handle_key(key(KeyCode::Enter), state);
        assert_eq!(render(&state.program()), "1 + 2");
    }

    #[test]
    fn enter_finishes_the_quarantine_the_cursor_is_inside() {
        // FRICTION.md #10: the keystroke that repairs a quarantined
        // expression leaves the cursor *in* the wrapper. Standing on the
        // wrapper to press Enter used to cost ↑ ↓ first.
        let state = AppState::new(examples::add_with_non_empty_hole());
        let state = handle_key(key(KeyCode::Tab), state); // onto ⦇true⦈
        let state = type_chars("2", state); // types inside: 1 + ⦇2⦈
        assert_eq!(render(&state.program()), "1 + ⦇2⦈");
        assert!(matches!(state.focus(), Exp::Num(2)), "inside the wrapper");

        let finished = handle_key(key(KeyCode::Enter), state.clone());
        assert_eq!(render(&finished.program()), "1 + 2", "one key, not three");
        assert!(matches!(finished.focus(), Exp::Num(2)), "cursor kept");

        // …and it is strictly cheaper than walking out to the wrapper first,
        // which is the same program by a longer road.
        let walked = handle_key(
            key(KeyCode::Enter),
            handle_key(key(KeyCode::Up), state.clone()),
        );
        assert_eq!(render(&walked.program()), render(&finished.program()));
        assert_eq!(finished.keystrokes() + 1, walked.keystrokes());
    }

    #[test]
    fn enter_inside_a_quarantine_that_does_not_fit_says_so_instead_of_jumping() {
        // The old fallback teleported to an unrelated empty hole, which is
        // how six keystrokes of the dogfooding session went into the wrong
        // subtree (FRICTION.md #11, #13).
        // if ⦇1⦈ then ⦇⦈ else ⦇⦈ — a Num quarantined at a Bool position,
        // with the cursor on the `1` inside the wrapper.
        let state = type_chars("1?", AppState::empty());
        let state = handle_key(key(KeyCode::Up), state); // the `if`
        let state = handle_key(key(KeyCode::Down), state); // the wrapper
        let state = handle_key(key(KeyCode::Down), state); // the `1` inside
        assert!(matches!(state.focus(), Exp::Num(1)));
        assert_eq!(state.enclosing_quarantine(), Some(1));

        let after = handle_key(key(KeyCode::Enter), state.clone());
        assert_eq!(after.program(), state.program(), "nothing changed");
        assert_eq!(
            index_path(after.zipper()),
            index_path(state.zipper()),
            "and the cursor did not teleport to an unrelated hole"
        );
        let hint = after.hint.expect("Enter must say why");
        assert!(hint.contains("does not fit yet"), "{hint}");
        assert!(hint.contains("Bool"), "and what it wanted: {hint}");
    }

    #[test]
    fn tab_walks_quarantines_too_and_says_when_nothing_is_left() {
        // FRICTION.md #12: `1 + ⦇true⦈` has no *empty* hole, and the editor
        // used to answer "no empty hole in this program" with the quarantine
        // on screen.
        let state = AppState::new(examples::add_with_non_empty_hole());
        let tabbed = handle_key(key(KeyCode::Tab), state);
        assert!(
            matches!(tabbed.focus(), Exp::NonEmptyHole(..)),
            "Tab must reach the one unfinished thing in the program"
        );
        assert_eq!(tabbed.hint, None);

        // …and a program with neither kind of hole says the true thing.
        let done = AppState::new(examples::increment_applied());
        let stuck = handle_key(key(KeyCode::Tab), done.clone());
        assert_eq!(stuck.program(), done.program());
        assert_eq!(stuck.hint.as_deref(), Some(NOTHING_UNFINISHED));
    }

    // --- binder identity (KEYS.md §"Which keys can decline") ---

    #[test]
    fn renaming_a_binder_onto_an_id_already_in_scope_is_declined() {
        // FRICTION.md #7, the one silent change of meaning the editor had:
        // λx0:Num. λx1:?. x0 + 1 — naming the *inner* binder x0 re-binds the
        // body's x0 to it, silently, and leaves the outer binder unreachable
        // from the keyboard.
        let state = type_chars("\\x0:n.\\x1:.x0+1", AppState::empty());
        assert_eq!(render(&state.program()), "λx0:Num. λx1:?. x0 + 1");

        // Back onto the inner lambda and into its binder-name slot.
        let state = handle_key(key(KeyCode::Up), state); // the `+`
        let state = handle_key(key(KeyCode::Up), state); // the inner lambda
        assert!(matches!(state.focus(), Exp::Lam(..)));
        let state = handle_key(key(KeyCode::Down), state);
        assert_eq!(state.slot, Slot::BinderName);

        let after = type_chars("x0", state.clone());
        assert_eq!(
            render(&after.program()),
            "λx0:Num. λx1:?. x0 + 1",
            "the program must be untouched"
        );
        assert_eq!(after.slot, Slot::BinderName, "the slot stays open");
        let hint = after.hint.expect("the warning is live, not silent");
        assert!(hint.contains("capture"), "{hint}");
        assert!(hint.contains("x0"), "{hint}");
    }

    #[test]
    fn naming_a_binder_what_it_is_already_called_is_not_a_capture() {
        // Every reference fixture types the name of a freshly minted binder,
        // which is the same id it already has: the check must not fire on
        // the identity rename, nor on an id nothing refers to.
        let state = type_chars("\\x0:n.\\x1:.x0+1", AppState::empty());
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Down), state);

        let same = type_chars("x1", state.clone());
        assert_eq!(render(&same.program()), "λx0:Num. λx1:?. x0 + 1");
        assert_eq!(same.hint, None, "renaming x1 to x1 changes nothing");

        let fresh = type_chars("x7", state);
        assert_eq!(
            render(&fresh.program()),
            "λx0:Num. λx7:?. x0 + 1",
            "an id nothing refers to is free to take"
        );
    }

    #[test]
    fn a_binder_whose_reference_would_be_orphaned_still_declines() {
        // The other half of KEYS.md's promise, which the calculus catches by
        // itself: x1 is referred to, so renaming it leaves that reference
        // unbound and the action does not apply.
        let state = type_chars("\\x1:n.x1", AppState::empty());
        let state = handle_key(key(KeyCode::Up), state);
        let state = handle_key(key(KeyCode::Down), state);
        let after = type_chars("x2", state);
        assert_eq!(render(&after.program()), "λx1:Num. x1");
        assert!(after.hint.unwrap().contains("unbound"));
    }

    // --- history ---

    #[test]
    fn one_undo_undoes_one_keystroke_however_many_actions_it_expanded_to() {
        // `?` here climbs out of the `<` before wrapping: three actions,
        // one keystroke.
        let state = type_chars("1<2", AppState::empty());
        let before = state.program();
        let with_if = type_chars("?", state);
        assert_eq!(render(&with_if.program()), "if 1 < 2 then ⦇⦈ else ⦇⦈");

        let undone = handle_key(ctrl(KeyCode::Char('z')), with_if.clone());
        assert_eq!(undone.program(), before, "one C-z, one keystroke");

        let redone = handle_key(ctrl(KeyCode::Char('r')), undone);
        assert_eq!(redone.program(), with_if.program());
    }

    #[test]
    fn undo_walks_back_to_the_program_the_session_opened_on() {
        let start = AppState::factorial();
        let mut state = type_chars("1+2", start.clone());
        for _ in 0..3 {
            state = handle_key(ctrl(KeyCode::Char('z')), state);
        }
        assert_eq!(state.program(), start.program());
        let stuck = handle_key(ctrl(KeyCode::Char('z')), state.clone());
        assert_eq!(stuck.program(), start.program());
        assert_eq!(stuck.hint.as_deref(), Some("nothing to undo"));
    }

    #[test]
    fn typing_after_an_undo_discards_the_redo_tail() {
        let state = type_chars("1+2", AppState::empty());
        let state = handle_key(ctrl(KeyCode::Char('z')), state);
        let state = type_chars("3", state);
        assert_eq!(render(&state.program()), "1 + 3");
        let state = handle_key(ctrl(KeyCode::Char('r')), state);
        assert_eq!(render(&state.program()), "1 + 3", "nothing to redo");
    }

    #[test]
    fn undo_restores_the_slot_the_keystroke_started_in() {
        let state = type_chars("\\x", AppState::empty());
        assert_eq!(state.slot, Slot::BinderName);
        let state = type_chars("0", state);
        let state = handle_key(ctrl(KeyCode::Char('z')), state);
        assert_eq!(state.slot, Slot::BinderName, "still naming the binder");
    }

    // --- the invariants every key must keep ---

    #[test]
    fn every_printable_key_leaves_a_well_typed_program() {
        use nothing_core::typing::is_well_typed;
        let alphabet: Vec<char> = "0123456789abnxtf_+-*<= \\?;,[]!~:.".chars().collect();
        // A deterministic walk over the alphabet from a few starting states.
        for start in [
            AppState::empty(),
            AppState::factorial(),
            AppState::new(examples::pair_and_project()),
        ] {
            for &c in &alphabet {
                let after = handle_key(key(KeyCode::Char(c)), start.clone());
                assert!(
                    is_well_typed(&after.program()),
                    "`{c}` produced {:?}",
                    after.program()
                );
            }
        }
    }

    #[test]
    fn no_key_ever_panics_on_any_example() {
        let alphabet: Vec<char> = "0123456789abxz_+-*<= \\?;,[]!~:.()>{}@#".chars().collect();
        let mut state = AppState::factorial();
        for (i, c) in alphabet.iter().cycle().take(200).enumerate() {
            state = handle_key(key(KeyCode::Char(*c)), state);
            if i % 7 == 0 {
                state = handle_key(key(KeyCode::Tab), state);
            }
            if i % 11 == 0 {
                state = handle_key(ctrl(KeyCode::Char('z')), state);
            }
        }
        assert!(nothing_core::typing::is_well_typed(&state.program()));
    }

    // --- the bindings inherited from the movement phase ---

    #[test]
    fn control_q_quits_and_nothing_else_does() {
        let state = AppState::factorial();
        assert!(handle_key(ctrl(KeyCode::Char('q')), state.clone()).quit);
        for code in [
            KeyCode::Char('q'),
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Tab,
            KeyCode::Esc,
            KeyCode::Char('x'),
        ] {
            assert!(!handle_key(key(code), state.clone()).quit, "{code:?} quit");
        }
    }

    #[test]
    fn the_arrows_walk_the_editor_tree() {
        let state = AppState::factorial();

        let name = handle_key(key(KeyCode::Down), state.clone());
        assert_eq!(name.slot, Slot::BinderName);

        let ann = handle_key(key(KeyCode::Right), name.clone());
        assert_eq!(ann.slot, Slot::Annotation);

        let body = handle_key(key(KeyCode::Right), ann);
        assert_eq!(body.slot, Slot::Node);
        assert_eq!(index_path(body.zipper()), vec![0]);

        let back = handle_key(key(KeyCode::Up), body);
        assert_eq!(index_path(back.zipper()), Vec::<usize>::new());
        assert_eq!(back.slot, Slot::Node);
    }

    #[test]
    fn a_declining_motion_changes_nothing_and_explains() {
        let root = AppState::factorial();
        let after = handle_key(key(KeyCode::Up), root.clone());
        assert_eq!(after.edit, root.edit);
        assert_eq!(after.slot, root.slot);
        assert_eq!(after.hint.as_deref(), Some("already at the root"));
    }

    #[test]
    fn tab_reaches_the_hole_and_shift_tab_comes_back() {
        let state = AppState::new(examples::add_with_empty_hole());
        let hole = handle_key(key(KeyCode::Tab), state.clone());
        assert_eq!(index_path(hole.zipper()), vec![1]);
        let same = handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT), hole);
        assert_eq!(index_path(same.zipper()), vec![1], "one hole, so it stays");
    }

    #[test]
    fn movement_never_changes_the_program() {
        let start = AppState::factorial();
        let program = start.program();
        let mut state = start;
        for code in [
            KeyCode::Down,
            KeyCode::Right,
            KeyCode::Right,
            KeyCode::Tab,
            KeyCode::Left,
            KeyCode::Up,
            KeyCode::Esc,
            KeyCode::BackTab,
        ] {
            state = handle_key(key(code), state);
            assert_eq!(state.program(), program, "{code:?} changed the program");
        }
    }

    #[test]
    fn an_unbound_key_is_inert_but_visible() {
        let state = AppState::factorial();
        let after = handle_key(key(KeyCode::Char('@')), state.clone());
        assert_eq!(after.edit, state.edit);
        assert_eq!(after.hint.as_deref(), Some("`@` is not bound here"));
    }

    #[test]
    fn a_key_release_is_ignored_entirely() {
        let state = AppState::factorial().with_hint("kept");
        let mut release = key(KeyCode::Down);
        release.kind = KeyEventKind::Release;
        let after = handle_key(release, state.clone());
        assert_eq!(after, state, "a release must not even clear the hint");
    }

    #[test]
    fn esc_ends_the_token_run() {
        let mut state = AppState::factorial();
        state.entry = "fac".to_string();
        let after = handle_key(key(KeyCode::Esc), state);
        assert!(after.entry.is_empty());
    }

    #[test]
    fn the_hint_lasts_exactly_one_keystroke() {
        let state = handle_key(key(KeyCode::Up), AppState::factorial());
        assert!(state.hint.is_some());
        let next = handle_key(key(KeyCode::Down), state);
        assert_eq!(next.hint, None);
    }
}
