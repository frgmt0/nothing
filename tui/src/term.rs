
use std::io::{self, Stdout, stdout};

use crossterm::event::{self, Event};
use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use crossterm::{cursor, execute};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::AppState;
use crate::keys::handle_key;
use crate::render::draw;

pub fn run(state: AppState) -> io::Result<()> {
    install_panic_hook();
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, state);

    restore();
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    mut state: AppState,
) -> io::Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, &state))?;


        if let Event::Key(key) = event::read()? {
            state = handle_key(key, state);
        }
        if state.quit {
            return Ok(());
        }
    }
}

pub fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen, cursor::Show);
}

pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
}