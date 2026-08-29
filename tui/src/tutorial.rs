use std::cmp::Ordering;

use nothing_core::doc::{Def, Doc, references};
use nothing_core::exp::Exp;
use nothing_core::ty::Ty;
use nothing_eval::main_type;

use crate::app::{AppState, children};
use crate::render::wrap_lines;

pub const DEFAULT_FILE: &str = "tutorial.n";

pub const PARAMETER_NAME: &str = "who";
pub const FUNCTION_NAME: &str = "greet";

pub struct Step {
    pub title: &'static str,
    pub keys: &'static str,
    pub instruction: &'static str,
    pub check: fn(&AppState) -> bool,
}

pub const STEPS: &[Step] = &[
    Step {
        title: "Write a function",
        keys: "\\",
        instruction: "Nothing here is typed as text and parsed: every key builds a piece of \
                      program.\nPress \\ to write a function. You get λx0:?. ⦇⦈: a parameter, \
                      a type nobody has said yet, and ⦇⦈, a hole where the body will go.",
        check: body_is_a_function,
    },
    Step {
        title: "Name the parameter",
        keys: "w h o",
        instruction: "The cursor is sitting in the parameter's name: the status line says \
                      `binder name`.\nType w h o. Each letter renames the parameter itself, so \
                      every use of it will follow.",
        check: parameter_is_named,
    },
    Step {
        title: "Give it a type",
        keys: ": s",
        instruction: "Press : to step from the name to the type, then s for Str.\nThe program \
                      now reads λwho:Str. ⦇⦈ and it was never, for one keystroke, ill-typed.",
        check: parameter_is_text,
    },
    Step {
        title: "Fill the hole",
        keys: ". \" h e l l o , space \" & w h o",
        instruction: "Press . to drop into the body.\nType \" to open a string, then hello, \
                      and a space, then \" to close it.\nPress & to join text on the end, then \
                      type w h o to name the parameter.",
        check: body_is_written,
    },
    Step {
        title: "Rename the definition",
        keys: "C-l g r e e t : s > s",
        instruction: "A definition has a name and a type of its own.\nPress C-l for its name \
                      and type g r e e t.\nPress : for its type, then s > s: Str -> Str.",
        check: definition_is_greet,
    },
    Step {
        title: "Start a second definition",
        keys: "C-n m a i n",
        instruction: "Press C-n for a new definition; the cursor lands in its name.\nType m a \
                      i n. A run always starts at the definition called main.",
        check: has_a_main,
    },
    Step {
        title: "Say main is a command",
        keys: ": c",
        instruction: "Press : for the type slot, then c: the Cmd prefix.\nmain : Cmd ? is a \
                      command: something to perform, not a value to print.",
        check: main_is_a_command,
    },
    Step {
        title: "Cause a quarantine",
        keys: ". $ g r e e t",
        instruction: "Press . for the body, then $ for print.\nNow type g r e e t. print wants \
                      text and greet is a function, so it does not fit, and the editor \
                      refuses nothing. It wraps it: print ⦇greet⦈. The ⦇⦈ holds the mistake so \
                      the document stays well-typed around it.",
        check: print_has_an_argument,
    },
    Step {
        title: "Repair it and finish",
        keys: "space \" w o r l d \" enter",
        instruction: "greet only needs its argument. Press space to apply it, then \" w o r l \
                      d \" for the text.\nThe quarantine now holds text after all. Press \
                      Enter and it closes.",
        check: main_performs,
    },
];

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Tutorial {
    pub step: usize,
    pub file: String,
}

impl Tutorial {
    pub fn start(state: &AppState, file: impl Into<String>) -> Tutorial {
        Tutorial {
            step: reached(state, 0),
            file: file.into(),
        }
    }

    pub fn finished(&self) -> bool {
        self.step >= STEPS.len()
    }
}

pub fn begin(mut state: AppState, file: impl Into<String>) -> AppState {
    let tutorial = Tutorial::start(&state, file);
    state.tutorial = Some(tutorial);
    state
}

pub fn advance(state: &mut AppState) {
    let Some(from) = state.tutorial.as_ref().map(|t| t.step) else {
        return;
    };
    let to = reached(state, from);
    if to != from
        && let Some(tutorial) = state.tutorial.as_mut()
    {
        tutorial.step = to;
    }
}

pub fn is_complete(state: &AppState) -> bool {
    state.tutorial.as_ref().is_some_and(Tutorial::finished)
}

fn reached(state: &AppState, from: usize) -> usize {
    let mut step = from;
    while step < STEPS.len() && (STEPS[step].check)(state) {
        step += 1;
    }
    step
}

pub fn pane_title(tutorial: &Tutorial) -> String {
    if tutorial.finished() {
        " tutorial · done ".to_string()
    } else {
        format!(" tutorial {}/{} ", tutorial.step + 1, STEPS.len())
    }
}

pub fn pane_lines(tutorial: &Tutorial, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    if tutorial.finished() {
        out.push(format!("All {} steps are done.", STEPS.len()));
        out.push(String::new());
        out.extend(wrap(
            "Press C-q to quit. The file is saved, and then performed.",
            width,
        ));
        out.push(String::new());
        out.push("Run it again with:".to_string());
        out.extend(wrap(&format!("nothing run {}", tutorial.file), width));
    } else {
        let step = &STEPS[tutorial.step];
        out.push(format!("Step {} of {}", tutorial.step + 1, STEPS.len()));
        out.extend(wrap(step.title, width));
        out.push(String::new());
        for paragraph in step.instruction.lines() {
            out.extend(wrap(paragraph, width));
        }
    }

    out.push(String::new());
    for (i, step) in STEPS.iter().enumerate() {
        let mark = match i.cmp(&tutorial.step) {
            Ordering::Less => '✓',
            Ordering::Equal => '▸',
            Ordering::Greater => '·',
        };
        out.push(format!("{mark} {}", step.title));
    }
    out
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    wrap_lines(text, width)
        .into_iter()
        .map(|line| line.trim_end().to_string())
        .collect()
}

fn hole_free(exp: &Exp) -> bool {
    let mut pending = vec![exp];
    while let Some(node) = pending.pop() {
        if matches!(node, Exp::EmptyHole(_) | Exp::NonEmptyHole(..)) {
            return false;
        }
        pending.extend(children(node));
    }
    true
}

fn on_first_def<T>(state: &AppState, f: impl FnOnce(&Def) -> T) -> Option<T> {
    let doc = state.edit.doc();
    let def = doc.defs().first()?;
    Some(f(def))
}

fn on_main<T>(state: &AppState, f: impl FnOnce(&Doc, &Def) -> T) -> Option<T> {
    let doc = state.edit.doc();
    let main = doc.main_id(state.names())?;
    let def = doc.get(main)?;
    Some(f(&doc, def))
}

fn body_is_a_function(state: &AppState) -> bool {
    on_first_def(state, |def| matches!(def.body, Exp::Lam(..))).unwrap_or(false)
}

fn parameter_is_named(state: &AppState) -> bool {
    let named = on_first_def(state, |def| match &def.body {
        Exp::Lam(id, _, _) => Some(*id),
        _ => None,
    });
    matches!(named.flatten(), Some(id) if state.display_name(id) == PARAMETER_NAME)
}

fn parameter_is_text(state: &AppState) -> bool {
    on_first_def(state, |def| matches!(&def.body, Exp::Lam(_, Ty::Str, _))).unwrap_or(false)
}

fn body_is_written(state: &AppState) -> bool {
    on_first_def(state, |def| match &def.body {
        Exp::Lam(id, _, body) => hole_free(body) && references(body, *id),
        _ => false,
    })
    .unwrap_or(false)
}

fn definition_is_greet(state: &AppState) -> bool {
    let named_and_typed = on_first_def(state, |def| (def.id, def.ann.clone()));
    match named_and_typed {
        Some((id, ann)) => {
            state.display_name(id) == FUNCTION_NAME
                && ann == Ty::Arrow(Box::new(Ty::Str), Box::new(Ty::Str))
        }
        None => false,
    }
}

fn has_a_main(state: &AppState) -> bool {
    let doc = state.edit.doc();
    doc.len() >= 2 && doc.main_id(state.names()).is_some()
}

fn main_is_a_command(state: &AppState) -> bool {
    has_a_main(state) && on_main(state, |_, def| matches!(def.ann, Ty::Cmd(_))).unwrap_or(false)
}

fn print_has_an_argument(state: &AppState) -> bool {
    on_main(state, |_, def| match &def.body {
        Exp::Print(text) => !matches!(**text, Exp::EmptyHole(_)),
        _ => false,
    })
    .unwrap_or(false)
}

fn main_performs(state: &AppState) -> bool {
    if state.quarantines() != 0 || !state.edit.is_well_typed() {
        return false;
    }
    on_main(state, |doc, def| {
        hole_free(&def.body) && matches!(main_type(doc, def.id), Ty::Cmd(_))
    })
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyscript::replay_keys;
    use nothing_action::act::EditState;

    fn script(upto: usize) -> String {
        STEPS[..upto]
            .iter()
            .flat_map(|step| step.keys.split_whitespace())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn start() -> AppState {
        begin(AppState::empty(), DEFAULT_FILE)
    }

    fn after(upto: usize) -> AppState {
        replay_keys(&script(upto), start()).expect("the step keys parse")
    }

    fn step_of(state: &AppState) -> usize {
        state.tutorial.as_ref().expect("a tutorial is running").step
    }

    #[test]
    fn a_fresh_document_starts_on_the_first_step() {
        let state = start();
        assert_eq!(step_of(&state), 0);
        assert!(!is_complete(&state));
    }

    #[test]
    fn step_1_writes_a_function() {
        let state = after(1);
        assert_eq!(step_of(&state), 1);
        assert_eq!(state.edit.render_document(), "main : ? = λx0:?. ⦇⦈");
    }

    #[test]
    fn step_2_names_the_parameter() {
        let state = after(2);
        assert_eq!(step_of(&state), 2);
        assert_eq!(state.edit.render_document(), "main : ? = λwho:?. ⦇⦈");
    }

    #[test]
    fn step_3_gives_the_parameter_a_type() {
        let state = after(3);
        assert_eq!(step_of(&state), 3);
        assert_eq!(state.edit.render_document(), "main : ? = λwho:Str. ⦇⦈");
    }

    #[test]
    fn step_4_fills_the_hole() {
        let state = after(4);
        assert_eq!(step_of(&state), 4);
        assert_eq!(
            state.edit.render_document(),
            "main : ? = λwho:Str. \"hello, \" ++ who"
        );
    }

    #[test]
    fn step_5_renames_the_definition() {
        let state = after(5);
        assert_eq!(step_of(&state), 5);
        assert_eq!(
            state.edit.render_document(),
            "greet : Str -> Str = λwho:Str. \"hello, \" ++ who"
        );
    }

    #[test]
    fn step_6_adds_a_second_definition() {
        let state = after(6);
        assert_eq!(step_of(&state), 6);
        assert_eq!(state.edit.def_count(), 2);
        assert!(
            state.edit.render_document().contains("main : ? = ⦇⦈"),
            "{}",
            state.edit.render_document()
        );
    }

    #[test]
    fn step_7_declares_main_a_command() {
        let state = after(7);
        assert_eq!(step_of(&state), 7);
        assert!(
            state.edit.render_document().contains("main : Cmd ? = ⦇⦈"),
            "{}",
            state.edit.render_document()
        );
    }

    #[test]
    fn step_8_causes_a_quarantine() {
        let state = after(8);
        assert_eq!(step_of(&state), 8);
        assert_eq!(
            state.quarantines(),
            1,
            "the point of the step is that the mismatch is held, not refused"
        );
        assert!(
            state.edit.is_well_typed(),
            "and the document is well-typed around it"
        );
        assert!(
            state
                .edit
                .render_document()
                .contains("main : Cmd ? = print ⦇greet⦈"),
            "{}",
            state.edit.render_document()
        );
    }

    #[test]
    fn step_9_repairs_the_quarantine_and_completes_the_tutorial() {
        let state = after(9);
        assert_eq!(step_of(&state), STEPS.len());
        assert!(is_complete(&state));
        assert_eq!(state.quarantines(), 0);
        assert_eq!(
            state.edit.render_document(),
            "greet : Str -> Str = λwho:Str. \"hello, \" ++ who\n\
             main : Cmd ? = print (greet \"world\")"
        );
    }

    #[test]
    fn every_step_needs_its_own_keys_and_no_step_is_skipped() {
        for k in 0..=STEPS.len() {
            assert_eq!(step_of(&after(k)), k, "the keys for steps 1..{k} reach {k}");
        }
    }

    #[test]
    fn progress_is_read_back_off_the_document_with_no_progress_file() {
        for k in 0..=STEPS.len() {
            let played = after(k);
            let doc = played.edit.doc();
            let reopened = AppState::from_edit(
                EditState::with_doc(&doc, played.names().clone(), 0)
                    .expect("a saved document has a first definition"),
            );
            assert_eq!(
                step_of(&begin(reopened, DEFAULT_FILE)),
                k,
                "reopening the document saved after step {k} resumed elsewhere"
            );
        }
    }

    #[test]
    fn the_pane_names_the_step_and_ticks_what_is_done() {
        let lines = pane_lines(
            after(3).tutorial.as_ref().expect("a tutorial is running"),
            30,
        );
        assert_eq!(lines[0], "Step 4 of 9");
        assert_eq!(lines[1], "Fill the hole");
        assert!(
            lines.contains(&"✓ Write a function".to_string()),
            "{lines:?}"
        );
        assert!(lines.contains(&"▸ Fill the hole".to_string()), "{lines:?}");
        assert!(
            lines.contains(&"· Repair it and finish".to_string()),
            "{lines:?}"
        );
        assert_eq!(
            pane_title(after(3).tutorial.as_ref().unwrap()),
            " tutorial 4/9 "
        );
    }

    #[test]
    fn the_finished_pane_names_the_command_that_runs_the_file() {
        let done = after(STEPS.len());
        let tutorial = done.tutorial.as_ref().expect("a tutorial is running");
        let lines = pane_lines(tutorial, 30);
        assert_eq!(pane_title(tutorial), " tutorial · done ");
        assert_eq!(lines[0], "All 9 steps are done.");
        assert!(
            lines.contains(&"nothing run tutorial.n".to_string()),
            "{lines:?}"
        );
        assert!(
            lines.iter().all(|line| !line.starts_with('▸')),
            "nothing is still in progress: {lines:?}"
        );
    }

    #[test]
    fn an_editor_session_with_no_tutorial_is_untouched_by_it() {
        let plain = replay_keys(&script(4), AppState::empty()).expect("the keys parse");
        assert!(plain.tutorial.is_none());
        assert_eq!(
            plain.edit.render_document(),
            after(4).edit.render_document()
        );
    }
}
