//! Drawing the editor (Phase 4).
//!
//! Two layers, deliberately separated:
//!
//! 1. [`program_line`] and [`status_line`] turn an [`AppState`] into
//!    strings. They are pure and contain every interesting decision, so the
//!    tests assert on text rather than on terminal cells.
//! 2. [`draw`] lays those strings out in a `ratatui` frame. It contains no
//!    decisions at all.
//!
//! # The projection
//!
//! The program is the projection from `action::cursor_render` — the same
//! one the Phase 3 REPL prints — with the focus delimited by `»…«`. The
//! editor adds exactly one thing: when the cursor addresses a **binder
//! slot** ([`Slot::BinderName`], [`Slot::Annotation`]) the markers go
//! *inside* the node, around the name or the type itself, because a slot is
//! not a mode and the user must see the thing the next keystroke will
//! affect:
//!
//! ```text
//! node        »λx0:Num. x0 + 1«
//! name slot   λ»x0«:Num. x0 + 1
//! annotation  λx0:»Num«. x0 + 1
//! ```
//!
//! The slot rendering is a *splice*, not a second renderer: the ancestors'
//! text comes verbatim from `render_with_cursor`, and only the focus's own
//! text — the substring between the markers — is rebuilt with the markers
//! moved inwards, transcribing `core::render`'s `Lam`/`Let` arms. The test
//! `stripping_markers_reproduces_the_plain_projection` pins this to
//! `core::render::render` byte for byte at every position and every slot of
//! every example, so the two renderers cannot drift.

use nothing_action::cursor_render::{CURSOR_CLOSE, CURSOR_OPEN, render_with_cursor};
use nothing_core::exp::Exp;
use nothing_core::render::{PREC_BINDER, PREC_CMP, render_id, render_prec};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph, Wrap};

use crate::app::{AppState, Slot};
use crate::complete;

/// The program under edit, with the cursor (or the addressed binder slot)
/// delimited by `»…«`.
pub fn program_line(state: &AppState) -> String {
    slot_marked(state).unwrap_or_else(|| render_with_cursor(state.zipper()))
}

/// Render the program with the markers inside the focused binder node, or
/// `None` when the cursor is not addressing a binder slot (which includes
/// the impossible-by-construction case of a slot on a non-binder node —
/// falling back to the ordinary projection is the honest thing to draw).
fn slot_marked(state: &AppState) -> Option<String> {
    let focus_text = match (state.focus(), state.slot) {
        (Exp::Lam(id, ty, body), Slot::BinderName) => format!(
            "λ{CURSOR_OPEN}{}{CURSOR_CLOSE}:{ty}. {}",
            render_id(*id),
            render_prec(body, PREC_BINDER)
        ),
        (Exp::Lam(id, ty, body), Slot::Annotation) => format!(
            "λ{}:{CURSOR_OPEN}{ty}{CURSOR_CLOSE}. {}",
            render_id(*id),
            render_prec(body, PREC_BINDER)
        ),
        (Exp::Let(id, bound, body), Slot::BinderName) => format!(
            "let {CURSOR_OPEN}{}{CURSOR_CLOSE} = {} in {}",
            render_id(*id),
            render_prec(bound, PREC_CMP),
            render_prec(body, PREC_BINDER)
        ),
        _ => return None,
    };

    // Splice into the ancestors' text. `render_with_cursor` emits each
    // marker exactly once (they are outside the plain projection's
    // vocabulary), and the focus's own parentheses — if its position
    // demanded any — sit inside the markers, so they have to be re-applied
    // around the re-marked node.
    let full = render_with_cursor(state.zipper());
    let open = full.find(CURSOR_OPEN)?;
    let close = full.rfind(CURSOR_CLOSE)?;
    let marked = &full[open + CURSOR_OPEN.len()..close];
    let spliced = if marked.starts_with('(') {
        format!("({focus_text})")
    } else {
        focus_text
    };
    Some(format!(
        "{}{spliced}{}",
        &full[..open],
        &full[close + CURSOR_CLOSE.len()..]
    ))
}

// --- the viewport (FRICTION.md #3) -----------------------------------------
//
// The projection is one long line, so on any real terminal it wraps, and
// before this the wrapped text was handed to `Paragraph` and simply clipped
// at the bottom border: at 46×12 the cursor spent forty keystrokes off
// screen. An editor that cannot show you the thing you are typing is not an
// editor, so the wrap happens *here*, where the line the cursor is on is
// known, and the window onto those lines is chosen to contain it.
//
// Wrapping here rather than in `Paragraph` also makes both halves testable
// as values: `wrap_lines` and `scroll_offset` are pure functions over
// strings and numbers, and `draw` still contains no decisions.

/// How many terminal columns `text` occupies.
fn columns(text: &str) -> usize {
    Span::raw(text).width()
}

/// How many terminal columns one character occupies.
fn char_columns(c: char) -> usize {
    let mut buffer = [0u8; 4];
    columns(c.encode_utf8(&mut buffer))
}

/// Break `text` into lines of at most `width` columns, at spaces where it
/// can and mid-token where it must.
///
/// A trailing space is allowed to overhang: it is invisible at the end of a
/// line, and pushing it to the next one would indent the wrapped text by a
/// space that is not in the program.
pub fn wrap_lines(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut used = 0;
    for token in text.split_inclusive(' ') {
        let mut token = token;
        loop {
            if used + columns(token.trim_end_matches(' ')) <= width {
                line.push_str(token);
                used += columns(token);
                break;
            }
            if used > 0 {
                lines.push(std::mem::take(&mut line));
                used = 0;
                continue;
            }
            // A single token wider than the whole line: hard-split it, which
            // is what a projection with no spaces in it needs.
            let (head, tail) = split_at_width(token, width);
            lines.push(head.to_string());
            token = tail;
        }
    }
    lines.push(line);
    lines
}

/// Split `text` at `width` columns, always leaving at least one character in
/// the head so wrapping cannot fail to make progress.
fn split_at_width(text: &str, width: usize) -> (&str, &str) {
    let mut used = 0;
    for (i, c) in text.char_indices() {
        if i > 0 && used + char_columns(c) > width {
            return text.split_at(i);
        }
        used += char_columns(c);
    }
    (text, "")
}

/// Which wrapped line the cursor's opening marker is on.
pub fn cursor_line(lines: &[String]) -> Option<usize> {
    lines.iter().position(|line| line.contains(CURSOR_OPEN))
}

/// The first visible line: the smallest scroll that keeps the cursor's line
/// on screen, with one line of context below it where there is one.
///
/// Stateless on purpose — there is no scroll position to get out of step
/// with the cursor, because the cursor *is* the scroll position.
pub fn scroll_offset(total: usize, height: usize, cursor: Option<usize>) -> usize {
    let (Some(cursor), true) = (cursor, total > height) else {
        return 0;
    };
    let last = total - height;
    cursor
        .saturating_sub(height.saturating_sub(2))
        .min(last)
        .min(cursor)
}

/// The border title, which says where in the program the window is whenever
/// the program does not fit in it.
pub fn program_title(offset: usize, shown: usize, total: usize) -> String {
    if shown >= total {
        " nothing ".to_string()
    } else {
        format!(
            " nothing · lines {}-{} of {total} ",
            offset + 1,
            offset + shown
        )
    }
}

/// Style the focus — everything between the markers — as a highlighted span
/// (`FRICTION.md` #13, #19).
///
/// Two thin markers in a 120-character line do not say *how much* is
/// selected. That is what made `⦇»e«⦈` (the cursor on a quarantine's
/// contents) and `»⦇e⦈«` (the cursor on the wrapper itself) misreadable as
/// each other, and it is why one letter on a focused root — which replaces
/// the selection, correctly — can destroy a whole program that never looked
/// selected. The markers stay, because they are the projection's own text and
/// every test reads them; the highlight is what makes the extent visible.
///
/// The markers may land on different wrapped lines, so the "inside the focus"
/// flag runs across the whole list rather than being recomputed per line.
fn focus_spans(lines: &[String]) -> Vec<Line<'static>> {
    let focus = Style::default().add_modifier(Modifier::REVERSED);
    let mut inside = false;
    lines
        .iter()
        .map(|line| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut rest = line.as_str();
            loop {
                let marker = if inside { CURSOR_CLOSE } else { CURSOR_OPEN };
                let Some(at) = rest.find(marker) else {
                    if !rest.is_empty() {
                        spans.push(styled(rest, inside, focus));
                    }
                    break;
                };
                let (chunk, tail) = rest.split_at(at + marker.len());
                if inside {
                    // …everything up to and including the closing marker.
                    spans.push(styled(chunk, true, focus));
                } else {
                    let (before, open) = chunk.split_at(at);
                    if !before.is_empty() {
                        spans.push(styled(before, false, focus));
                    }
                    spans.push(styled(open, true, focus));
                }
                inside = !inside;
                rest = tail;
            }
            Line::from(spans)
        })
        .collect()
}

fn styled(text: &str, highlighted: bool, focus: Style) -> Span<'static> {
    if highlighted {
        Span::styled(text.to_string(), focus)
    } else {
        Span::raw(text.to_string())
    }
}

/// The status line `KEYS.md` §"What the status line must show" requires:
///
/// - **the expected type at the cursor**, always — it is what makes
///   candidate ranking legible rather than magic;
/// - **the current slot** when the focus is a binder part;
/// - **the candidate list** during a name run, the committed entry marked
///   with `‹›` and each candidate's type shown;
/// - **a quarantine marker** on a non-empty hole, reading "fits now — press
///   Enter" whenever `Finish` would succeed.
pub fn status_line(state: &AppState) -> String {
    let mut line = format!(
        "{} · expects {} · {}",
        state.slot.label(),
        state.expected_ty(),
        focus_label(state.focus())
    );
    if matches!(state.focus(), Exp::NonEmptyHole(..)) {
        line.push_str(" · ");
        line.push_str(if state.finishes() {
            "fits now — press Enter"
        } else {
            "does not fit yet"
        });
    } else if let Some(fits) = state.enclosing_finishes() {
        // Asked and answered from *inside* the wrapper, which is where the
        // keystroke that repaired the expression left the cursor
        // (`FRICTION.md` #10): the editor already knows, so it says so here
        // rather than after two navigation keys.
        line.push_str(" · ");
        line.push_str(if fits {
            "inside ⦇⦈ · fits now — press Enter"
        } else {
            "inside ⦇⦈ · does not fit yet"
        });
    }
    // "Is this program finished?" — an answer that counted empty holes only
    // was the reason two quarantines could sit on screen under the words "no
    // empty hole in this program" (`FRICTION.md` #12).
    match state.quarantines() {
        0 => {}
        1 => line.push_str(" · 1 quarantined"),
        n => line.push_str(&format!(" · {n} quarantined")),
    }
    if let Some(entry) = entry_line(state) {
        line.push_str(" · ");
        line.push_str(&entry);
    }
    if let Some(hint) = &state.hint {
        line.push_str(" · ");
        line.push_str(hint);
    }
    line
}

/// What is being typed, and — during a name run — what it could resolve to.
///
/// Entry must look like it is happening *to the program*, because it is: the
/// committed candidate is already in the projection above, and this line
/// says what else was on offer. The ranking behind the order lives in
/// [`crate::complete`].
fn entry_line(state: &AppState) -> Option<String> {
    if state.entry.is_empty() {
        return None;
    }
    let mut line = format!("typing `{}`", state.entry);
    if state.slot == Slot::Node {
        let expected = state.expected_ty();
        let offers: Vec<String> = complete::candidates(state, &state.entry)
            .iter()
            .take(4)
            .enumerate()
            .map(|(i, c)| {
                let fit = if c.fits(&expected) { "" } else { " ✗" };
                if i == 0 && state.entry_committed {
                    format!("‹{}:{}{fit}›", c.name, c.ty)
                } else {
                    format!("{}:{}{fit}", c.name, c.ty)
                }
            })
            .collect();
        line.push_str(&format!(
            " · {}",
            if offers.is_empty() {
                // KEYS.md: an unmatched buffer "renders in unresolved
                // styling"; the hint beside it says what is wrong.
                "unresolved".to_string()
            } else {
                offers.join("  ")
            }
        ));
    }
    Some(line)
}

/// The bottom line: the grammar, abbreviated to one row. `KEYS.md` is the
/// full reference; this is the reminder.
pub fn key_line() -> &'static str {
    "↑↓←→ move · Tab hole · 0-9 a-z literal · +-*<= op · space \\?;,[]! form · :. slot · \
     Enter fit · C-z undo · C-q quit"
}

/// What kind of node the cursor is on, for the status line.
fn focus_label(exp: &Exp) -> &'static str {
    match exp {
        Exp::Var(_) => "variable",
        Exp::Lam(..) => "lambda",
        Exp::Ap(..) => "application",
        Exp::Num(_) => "number",
        Exp::Bool(_) => "boolean",
        Exp::BinOp(..) => "operator",
        Exp::If(..) => "conditional",
        Exp::Let(..) => "let",
        Exp::Pair(..) => "pair",
        Exp::Proj(..) => "projection",
        Exp::EmptyHole(_) => "empty hole",
        Exp::NonEmptyHole(..) => "quarantined ⦇e⦈",
    }
}

/// Lay the three lines out in `frame`. No decisions live here.
pub fn draw(frame: &mut Frame, state: &AppState) {
    let [program_area, status_area, keys_area] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(1),
        // Two rows: the key line is a whole grammar and wraps on a narrow
        // terminal rather than being silently cut in half.
        Constraint::Length(2),
    ])
    .areas(frame.area());

    // Wrap to the *inside* of the border, then window those lines onto the
    // cursor. `Paragraph`'s own wrapping cannot be used here: it clips
    // whatever does not fit, and what did not fit was the cursor.
    let inner = Block::bordered()
        .padding(Padding::horizontal(1))
        .inner(program_area);
    let lines = wrap_lines(&program_line(state), inner.width as usize);
    let height = (inner.height as usize).max(1);
    let offset = scroll_offset(lines.len(), height, cursor_line(&lines));
    let end = (offset + height).min(lines.len());
    let visible = focus_spans(&lines)[offset..end].to_vec();

    let program = Paragraph::new(visible).block(
        Block::bordered()
            .title(program_title(offset, end - offset, lines.len()))
            .padding(Padding::horizontal(1)),
    );
    frame.render_widget(program, program_area);
    frame.render_widget(
        Paragraph::new(status_line(state)).style(Style::default().add_modifier(Modifier::DIM)),
        status_area,
    );
    frame.render_widget(
        Paragraph::new(key_line())
            .wrap(Wrap { trim: false })
            .style(Style::default().add_modifier(Modifier::DIM)),
        keys_area,
    );
}

/// Draw `state` into an off-screen `TestBackend` and return the terminal's
/// contents as text, one line per row.
///
/// The headless equivalent of looking at the screen: the Phase 4 tests use
/// it to assert that the focus is visibly updating, without a terminal.
pub fn render_to_string(state: &AppState, width: u16, height: u16) -> String {
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = ratatui::Terminal::new(backend).expect("TestBackend cannot fail");
    terminal
        .draw(|frame| draw(frame, state))
        .expect("TestBackend cannot fail");
    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::index_path;
    use nothing_action::zipper::all_positions;
    use nothing_core::examples;
    use nothing_core::render::render;

    fn all_examples() -> Vec<(&'static str, Exp)> {
        vec![
            ("let_identity", examples::let_identity()),
            ("increment_applied", examples::increment_applied()),
            ("clamp_to_one", examples::clamp_to_one()),
            ("pair_and_project", examples::pair_and_project()),
            ("pair_with_empty_hole", examples::pair_with_empty_hole()),
            ("add_with_empty_hole", examples::add_with_empty_hole()),
            ("square_and_compare", examples::square_and_compare()),
            (
                "identity_hole_annotated_applied",
                examples::identity_hole_annotated_applied(),
            ),
            (
                "add_with_non_empty_hole",
                examples::add_with_non_empty_hole(),
            ),
            (
                "if_over_pairs_with_hole",
                examples::if_over_pairs_with_hole(),
            ),
        ]
    }

    /// Every state whose cursor is somewhere in `exp`: every zipper
    /// position, plus the binder slots of every `Lam`/`Let`.
    fn all_states(exp: &Exp) -> Vec<AppState> {
        let mut out = Vec::new();
        for z in all_positions(exp) {
            let base = AppState::new(exp.clone());
            let state = base
                .apply_actions(
                    &index_path(&z)
                        .into_iter()
                        .map(nothing_action::act::Action::MoveChild)
                        .collect::<Vec<_>>(),
                )
                .expect("a path from all_positions must be walkable");
            let slots: &[Slot] = match state.focus() {
                Exp::Lam(..) => &[Slot::Node, Slot::BinderName, Slot::Annotation],
                Exp::Let(..) => &[Slot::Node, Slot::BinderName],
                _ => &[Slot::Node],
            };
            for slot in slots {
                let mut s = state.clone();
                s.slot = *slot;
                out.push(s);
            }
        }
        out
    }

    #[test]
    fn stripping_markers_reproduces_the_plain_projection() {
        for (name, exp) in all_examples() {
            let plain = render(&exp);
            for state in all_states(&exp) {
                let marked = program_line(&state);
                let stripped = marked.replace(CURSOR_OPEN, "").replace(CURSOR_CLOSE, "");
                assert_eq!(
                    stripped,
                    plain,
                    "{name}: the {} projection at {:?} disagrees with core::render",
                    state.slot.label(),
                    index_path(state.zipper())
                );
            }
        }
    }

    #[test]
    fn every_position_renders_differently() {
        for (name, exp) in all_examples() {
            let mut seen: Vec<String> = Vec::new();
            for state in all_states(&exp) {
                let line = program_line(&state);
                assert!(
                    !seen.contains(&line),
                    "{name}: two cursor positions render identically: {line}"
                );
                seen.push(line);
            }
        }
    }

    #[test]
    fn binder_slots_mark_the_name_and_the_type() {
        let lam = AppState::factorial();
        assert!(program_line(&lam).starts_with("»λx0:Num."));

        let name = lam.move_down().unwrap();
        assert!(program_line(&name).starts_with("λ»x0«:Num."));

        let ann = name.move_next().unwrap();
        assert!(program_line(&ann).starts_with("λx0:»Num«."));
    }

    #[test]
    fn a_slot_inside_parentheses_keeps_them() {
        // (λx0:?. x0) true — the lambda is parenthesised by its position.
        let state = AppState::new(examples::identity_hole_annotated_applied())
            .move_down()
            .unwrap() // the lambda
            .move_down()
            .unwrap(); // its binder name
        assert_eq!(state.slot, Slot::BinderName);
        assert_eq!(program_line(&state), "(λ»x0«:?. x0) true");
    }

    #[test]
    fn the_status_line_names_the_slot_and_the_expected_type() {
        let state = AppState::factorial();
        let status = status_line(&state);
        assert!(status.contains("node"), "{status}");
        assert!(status.contains("lambda"), "{status}");

        let ann = state.move_down().unwrap().move_next().unwrap();
        assert!(status_line(&ann).contains("annotation"));
    }

    #[test]
    fn the_status_line_lists_the_candidates_during_a_name_run() {
        use crate::keys::{handle_key, key};
        use ratatui::crossterm::event::KeyCode;

        let state = ['\\', 'x', '0', ':', 'n', '.', 'x']
            .into_iter()
            .fold(AppState::empty(), |state, c| {
                handle_key(key(KeyCode::Char(c)), state)
            });
        let status = status_line(&state);
        assert!(status.contains("typing `x`"), "{status}");
        assert!(
            status.contains("‹x0:Num›"),
            "the committed candidate is marked: {status}"
        );

        let unresolved = handle_key(key(KeyCode::Char('q')), state);
        assert!(status_line(&unresolved).contains("unresolved"));
    }

    /// The candidate list is the completion UI: `KEYS.md` §"What the status
    /// line must show" puts it beside the buffer, committed entry marked and
    /// every candidate's type visible, because the type ranking is only
    /// legible if the types are.
    ///
    /// The screen here is the payoff of bidirectional typing, drawn: the
    /// cursor is the argument of `x2 ⦇⦈`, which asks for `Num → Num`, and
    /// one keystroke — `x`, matching all three binders — has committed the
    /// binder of that type rather than the innermost one.
    #[test]
    fn the_candidate_list_is_ranked_by_the_expected_type_on_screen() {
        use crate::keys::{handle_key, key};
        use ratatui::crossterm::event::KeyCode;

        // λx0:Num→Num. λx1:Bool. λx2:(Num→Num)→Num. x2 »⦇⦈«, then `x`.
        let state = "\\x0:n>n.\\x1:b.\\x2:(n>n)>n.x2 x"
            .chars()
            .fold(AppState::empty(), |state, c| {
                handle_key(key(KeyCode::Char(c)), state)
            });

        let screen = render_to_string(&state, 120, 8);
        assert!(
            screen.contains("x2 »x0«"),
            "the top-ranked candidate is committed live into the program: {screen}"
        );
        assert!(
            screen.contains("expects Num -> Num"),
            "the expected type is always shown: {screen}"
        );
        assert!(
            screen.contains("typing `x`"),
            "the buffer is shown beside the list: {screen}"
        );
        assert!(
            screen.contains("‹x0:Num -> Num›"),
            "the committed candidate is marked, with its type: {screen}"
        );
        assert!(
            screen.contains("x1:Bool ✗"),
            "a candidate that does not fit is still offered, marked ✗: {screen}"
        );

        let function = screen.find("‹x0").expect("x0 on screen");
        // `x1:Bool ✗` with the marker, so this cannot match the `λx1:Bool.`
        // in the program line above.
        let boolean = screen.find("x1:Bool ✗").expect("x1 offered on screen");
        assert!(
            function < boolean,
            "the fitting function is listed before the unrelated Bool: {screen}"
        );
    }

    #[test]
    fn the_status_line_marks_a_quarantine_and_says_when_it_fits() {
        use crate::keys::{handle_key, key};
        use ratatui::crossterm::event::KeyCode;

        // 1 + ⦇true⦈ — the wrapper, then the same wrapper once its contents
        // have been edited into something that fits.
        let state = AppState::new(examples::add_with_non_empty_hole())
            .apply_actions(&[nothing_action::act::Action::MoveChild(1)])
            .expect("the right operand");
        assert!(status_line(&state).contains("does not fit yet"));

        let fixed = handle_key(key(KeyCode::Char('2')), state);
        let fixed = handle_key(key(KeyCode::Up), fixed);
        assert!(
            status_line(&fixed).contains("fits now — press Enter"),
            "{}",
            status_line(&fixed)
        );
    }

    #[test]
    fn the_hint_reaches_the_status_line() {
        let state = AppState::factorial().with_hint("no child here");
        assert!(status_line(&state).contains("no child here"));
    }

    #[test]
    fn the_factorial_example_is_on_screen() {
        let screen = render_to_string(&AppState::factorial(), 60, 8);
        assert!(
            screen.contains("»λx0:Num. if x0 == 0 then 1 else x0 * ⦇⦈«"),
            "{screen}"
        );
        assert!(screen.contains("C-q quit"), "{screen}");
    }

    #[test]
    fn long_programs_wrap_rather_than_vanish() {
        let screen = render_to_string(&AppState::factorial(), 24, 10);
        assert!(screen.contains("»λx0:Num. if x0"), "{screen}");
        assert!(screen.contains("⦇⦈«"), "{screen}");
    }

    // --- the viewport (FRICTION.md #3) ---

    #[test]
    fn wrapping_neither_adds_nor_loses_a_character() {
        let text = program_line(&AppState::factorial());
        for width in 1..60 {
            let lines = wrap_lines(&text, width);
            assert_eq!(lines.concat(), text, "width {width} changed the text");
            for line in &lines {
                // A trailing space may overhang: it is invisible at the end
                // of a line, and moving it would indent the next one.
                assert!(
                    columns(line.trim_end_matches(' ')) <= width.max(1),
                    "width {width}: `{line}` is too wide"
                );
            }
        }
    }

    #[test]
    fn a_token_wider_than_the_line_is_split_rather_than_dropped() {
        let lines = wrap_lines("λx0:Num.", 3);
        assert_eq!(lines.concat(), "λx0:Num.");
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|line| !line.is_empty()));
    }

    #[test]
    fn the_window_follows_the_cursor() {
        // Below the fold, above the fold, and the ends.
        assert_eq!(scroll_offset(3, 10, Some(2)), 0, "it all fits");
        assert_eq!(scroll_offset(40, 10, Some(0)), 0);
        assert_eq!(scroll_offset(40, 10, Some(20)), 12, "one line of context");
        assert_eq!(scroll_offset(40, 10, Some(39)), 30, "clamped at the end");
        assert_eq!(scroll_offset(40, 10, None), 0);
        for cursor in 0..40 {
            let offset = scroll_offset(40, 10, Some(cursor));
            assert!(
                (offset..offset + 10).contains(&cursor),
                "line {cursor} is off screen at offset {offset}"
            );
        }
    }

    #[test]
    fn the_cursor_is_on_screen_in_a_small_terminal() {
        use crate::keys::{handle_key, key};
        use ratatui::crossterm::event::KeyCode;

        // The dogfooding session's own case: a long arithmetic chain typed
        // in a 46×12 terminal, where the cursor spent forty keystrokes below
        // the bottom border.
        let mut state = "1".chars().fold(AppState::empty(), |state, c| {
            handle_key(key(KeyCode::Char(c)), state)
        });
        for _ in 0..40 {
            state = "+1"
                .chars()
                .fold(state, |state, c| handle_key(key(KeyCode::Char(c)), state));
            let screen = render_to_string(&state, 46, 12);
            assert!(
                screen.contains(CURSOR_OPEN),
                "the cursor scrolled off screen:\n{screen}"
            );
        }
        // In a box too short for the program, the cursor is still on screen
        // and the title says which part of it the window shows.
        let screen = render_to_string(&state, 46, 8);
        assert!(screen.contains(CURSOR_OPEN), "{screen}");
        assert!(screen.contains("lines "), "{screen}");
    }

    #[test]
    fn a_terminal_too_small_for_anything_still_draws() {
        // The viewport does its own arithmetic on the inside of a border, so
        // every degenerate size has to be a size, not a panic.
        for width in 1..14u16 {
            for height in 1..9u16 {
                let _ = render_to_string(&AppState::factorial(), width, height);
            }
        }
    }

    #[test]
    fn a_program_that_fits_is_not_scrolled_and_says_nothing_about_lines() {
        let screen = render_to_string(&AppState::factorial(), 60, 8);
        assert!(screen.contains(" nothing "), "{screen}");
        assert!(!screen.contains("lines "), "{screen}");
    }

    // --- the focus is a span, not two markers (FRICTION.md #13, #19) ---

    /// The rendered cell styles, one row per line — the part
    /// [`render_to_string`] throws away.
    fn render_styles(state: &AppState, width: u16, height: u16) -> ratatui::buffer::Buffer {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).expect("TestBackend cannot fail");
        terminal
            .draw(|frame| draw(frame, state))
            .expect("TestBackend cannot fail");
        terminal.backend().buffer().clone()
    }

    #[test]
    fn the_whole_focus_is_highlighted_not_just_its_markers() {
        let state = AppState::factorial();
        let buffer = render_styles(&state, 60, 8);
        let column = |marker: &str| {
            (0..60)
                .find(|&x| buffer[(x, 1)].symbol() == marker)
                .unwrap_or_else(|| panic!("`{marker}` is not on row 1"))
        };
        let (start, end) = (column(CURSOR_OPEN), column(CURSOR_CLOSE));

        let highlighted = |x: u16| {
            buffer[(x, 1)]
                .style()
                .add_modifier
                .contains(Modifier::REVERSED)
        };
        for x in start..=end {
            assert!(highlighted(x), "column {x} of the focus is not lit");
        }
        assert!(!highlighted(0), "the border is not part of the focus");
    }

    #[test]
    fn the_quarantine_wrapper_and_its_contents_look_different() {
        use crate::keys::{handle_key, key};
        use ratatui::crossterm::event::KeyCode;

        // `⦇»e«⦈` (the cursor on the contents) and `»⦇e⦈«` (on the wrapper)
        // differ by two brackets in a long line; the highlight must differ by
        // the brackets themselves.
        let wrapper = handle_key(
            key(KeyCode::Tab),
            AppState::new(examples::add_with_non_empty_hole()),
        );
        let contents = handle_key(key(KeyCode::Down), wrapper.clone());

        let lit = |state: &AppState| {
            let buffer = render_styles(state, 60, 8);
            (0..60)
                .filter(|&x| {
                    buffer[(x, 1)]
                        .style()
                        .add_modifier
                        .contains(Modifier::REVERSED)
                })
                .map(|x| buffer[(x, 1)].symbol().to_string())
                .collect::<String>()
        };
        assert!(lit(&wrapper).contains('⦇'), "{}", lit(&wrapper));
        assert!(!lit(&contents).contains('⦇'), "{}", lit(&contents));
    }

    // --- the status line answers "am I done?" (FRICTION.md #10, #12) ---

    #[test]
    fn the_status_line_answers_from_inside_a_quarantine() {
        use crate::keys::{handle_key, key};
        use ratatui::crossterm::event::KeyCode;

        let state = handle_key(
            key(KeyCode::Tab),
            AppState::new(examples::add_with_non_empty_hole()),
        );
        let inside = handle_key(key(KeyCode::Down), state);
        assert!(status_line(&inside).contains("inside ⦇⦈ · does not fit yet"));

        let repaired = handle_key(key(KeyCode::Char('2')), inside);
        assert!(
            status_line(&repaired).contains("inside ⦇⦈ · fits now — press Enter"),
            "{}",
            status_line(&repaired)
        );
    }

    #[test]
    fn the_status_line_counts_the_quarantines_left() {
        let state = AppState::new(examples::add_with_non_empty_hole());
        assert!(status_line(&state).contains("1 quarantined"));
        assert!(!status_line(&AppState::factorial()).contains("quarantined"));
    }
}
