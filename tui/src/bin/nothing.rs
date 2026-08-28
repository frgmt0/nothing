
use std::io;

use nothing_tui::AppState;

fn main() -> io::Result<()> {
    nothing_tui::term::run(AppState::factorial())
}