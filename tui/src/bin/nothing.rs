//! The editor binary: `cargo run -p nothing-tui --bin nothing`.
//!
//! A shim, deliberately. It picks the program to open — the factorial
//! reference program, replayed from the benchmark fixture — and hands it to
//! [`nothing_tui::term::run`]. Every decision about what a key means or
//! what the screen says lives in the library, where it is tested without a
//! terminal.

use std::io;

use nothing_tui::AppState;

fn main() -> io::Result<()> {
    nothing_tui::term::run(AppState::factorial())
}
