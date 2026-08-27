//! The terminal shim (Phase 4).
//!
//! Everything interesting is elsewhere: [`crate::keys::handle_key`] decides
//! what a keystroke means and [`crate::render`] decides what the screen
//! says. This module only owns the three things that genuinely need a
//! terminal — entering raw mode, reading events, and *always* getting out
//! again.
//!
//! # Restoring the terminal
//!
//! Two paths lead out of raw mode and the alternate screen, and both are
//! covered:
//!
//! - the normal one: [`run`] restores after the loop, whatever the loop
//!   returned, including an `Err`;
//! - the panicking one: [`install_panic_hook`] restores *before* the
//!   default hook prints, so a backtrace lands on a usable terminal rather
//!   than a raw-mode staircase.
//!
//! The hook is installed before `ratatui::try_init`, as ratatui's docs
//! require of any additional hook, so ours runs after ratatui's own restore
//! and the two are idempotent with respect to each other — `disable_raw_mode`
//! and `LeaveAlternateScreen` are both safe to issue twice, and both are
//! issued with their errors ignored because a panicking process has nowhere
//! to report them.

use std::io::{self, Stdout, stdout};

use crossterm::event::{self, Event};
use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use crossterm::{cursor, execute};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::AppState;
use crate::keys::handle_key;
use crate::render::draw;

/// Run the editor to completion: set up the terminal, loop until the quit
/// key, restore.
///
/// The terminal is restored on every exit path — clean, `Err`, or panic.
pub fn run(state: AppState) -> io::Result<()> {
    install_panic_hook();
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, state);
    // Restore before propagating, so an error message is readable.
    restore();
    result
}

/// Draw, read one key, apply it, repeat. The only stateful thing here is
/// `state`, and every transition is [`handle_key`].
fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    mut state: AppState,
) -> io::Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, &state))?;
        // Resizes redraw on the next pass; nothing but a key is a
        // keystroke.
        if let Event::Key(key) = event::read()? {
            state = handle_key(key, state);
        }
        if state.quit {
            return Ok(());
        }
    }
}

/// Leave raw mode and the alternate screen, ignoring errors.
///
/// Idempotent, so it is safe to call from both the normal exit path and the
/// panic hook.
pub fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen, cursor::Show);
}

/// Install a panic hook that restores the terminal before the previous hook
/// runs.
///
/// Must be installed *before* `ratatui::try_init` (which installs its own),
/// so that ours is the outer one and the terminal is restored no matter
/// which of the two ran first.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
}
