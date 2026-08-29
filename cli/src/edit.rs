use std::path::Path;

use nothing_action::log::{ActionLog, AuthorId, now_millis};
use nothing_store::Document;
use nothing_tui::AppState;

use crate::fileio::{read_document, write_document};

pub const HELP: &str = "\
nothing edit <file>

Open <file> in the TUI editor. If <file> does not exist, editing starts
from the empty program. Quit with ctrl-q; the program, its name table and
its doc lines are written back to <file> on exit. The standard library is in
scope and offered in completion, marked `std·`; it is never written into
<file>.

Options:
  -h, --help   print this help and exit";

pub fn load(path: &Path) -> Result<(AppState, ActionLog), String> {
    match read_document(path) {
        Ok(doc) => {
            let edit = nothing_action::act::EditState::with_doc(&doc.doc, doc.names, 0)
                .expect("a decoded document always has a first definition")
                .with_docs(doc.docs)
                .under(nothing_stdlib::prelude());
            Ok((AppState::from_edit(edit), doc.log))
        }
        Err(_) if !path.exists() => Ok((
            AppState::from_edit(
                nothing_action::act::EditState::empty().under(nothing_stdlib::prelude()),
            ),
            ActionLog::new(),
        )),
        Err(err) => Err(err),
    }
}

pub fn save(path: &Path, final_state: &AppState, base_log: ActionLog) -> Result<(), String> {
    let mut log = base_log;
    let author = AuthorId::new(1);
    for action in final_state.actions() {
        log.append(action.clone(), now_millis(), author);
    }
    let doc = Document::documented(
        final_state.edit.doc(),
        final_state.names().own(),
        final_state.edit.docs.own(),
        log,
    );
    write_document(path, &doc)
}

pub fn run(path: &Path) -> i32 {
    let (initial, base_log) = match load(path) {
        Ok(pair) => pair,
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    };

    let final_state = match nothing_tui::term::run(initial) {
        Ok(state) => state,
        Err(err) => {
            eprintln!("error: terminal session failed: {err}");
            return 1;
        }
    };

    if let Err(err) = save(path, &final_state, base_log) {
        eprintln!("error: {err}");
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_action::act::Action;
    use nothing_core::examples;
    use nothing_core::render::render;

    #[test]
    fn loading_a_missing_file_starts_from_the_empty_program() {
        let dir = std::env::temp_dir().join("nothing-cli-edit-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("does-not-exist.nothing");
        std::fs::remove_file(&path).ok();

        let (state, log) = load(&path).expect("a missing file is not an error");
        assert_eq!(state.program(), AppState::empty().program());
        assert!(log.is_empty());
    }

    #[test]
    fn loading_a_document_written_via_store_reproduces_its_program_and_names() {
        let dir = std::env::temp_dir().join("nothing-cli-edit-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("square-and-compare.nothing");

        let names = examples::names();
        let doc = Document::new(
            examples::square_and_compare(),
            names.clone(),
            ActionLog::new(),
        );
        write_document(&path, &doc).expect("the fixture document writes");

        let (state, log) = load(&path).expect("the fixture document loads");
        assert_eq!(state.program(), examples::square_and_compare());
        assert_eq!(
            render(&state.program(), state.names()),
            render(&examples::square_and_compare(), &names)
        );
        assert!(log.is_empty());
    }

    #[test]
    fn saving_then_loading_round_trips_actions_applied_in_a_session() {
        let dir = std::env::temp_dir().join("nothing-cli-edit-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("round-trip.nothing");
        std::fs::remove_file(&path).ok();

        let (initial, base_log) = load(&path).expect("a missing file is not an error");
        let mut state = initial;
        state = state
            .apply_actions(&[Action::ConstructNum(7)])
            .expect("constructing a number applies to the empty program");

        save(&path, &state, base_log).expect("saving the session writes the file");

        let (reloaded, log) = load(&path).expect("the saved file loads back");
        assert_eq!(reloaded.program(), state.program());
        assert_eq!(log.len(), 1);
    }
}
