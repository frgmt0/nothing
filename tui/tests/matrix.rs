//! `KEYS.md`'s **printable-character matrix**, as a table.
//!
//! The matrix is normative: it says what every printable character does in
//! each of the seven contexts a cursor can be in, and it is the part of the
//! grammar most easily broken by a well-meaning change somewhere else. So it
//! is transcribed here as data — one row per (context, character) — and the
//! test drives the real key handler and compares the *observable* outcome:
//! the projection with the cursor in it, the slot, and the live buffer.
//!
//! Reading a row: `⦇⦈` is an empty hole, `⦇e⦈` a quarantined expression,
//! `»…«` the cursor, `name:` a binder slot, and `⟨…⟩` the live token run.
//! An unchanged row is a key that declined — which for a printable character
//! is only ever the handful `KEYS.md` marks "no-op, hint".

use nothing_tui::keys::{handle_key, key};
use nothing_tui::render::program_line;
use nothing_tui::{AppState, Slot};

use crossterm::event::KeyCode;

/// The seven contexts of the matrix, each built by typing into the editor
/// rather than by hand, so a context cannot drift from what the keys
/// actually produce.
fn context(name: &str) -> AppState {
    let typed = |text: &str| {
        text.chars().fold(AppState::empty(), |state, c| {
            handle_key(key(KeyCode::Char(c)), state)
        })
    };
    match name {
        // A: an empty hole, with one `Num` binder in scope.
        "A empty hole" => typed("\\x0:n."),
        // B: a written expression — a variable — under the cursor, with the
        // name run that wrote it ended (that is context D).
        "B written expr" => handle_key(key(KeyCode::Esc), typed("\\x0:n.x0")),
        // C: a focused number.
        "C focused Num" => typed("\\x0:n.12"),
        // D: mid-name run: `x` typed, `x0` committed, the run still live.
        "D mid-name run" => typed("\\x0:n.x"),
        // F: the binder-name slot, one character in.
        "F binder name" => typed("\\x"),
        // G: a non-empty hole — `x0` quarantined at a `Bool` position,
        // with the cursor left on the wrapper and the run ended.
        "G non-empty hole" => handle_key(key(KeyCode::Esc), typed("\\x0:n.?x")),
        other => panic!("unknown context {other}"),
    }
}

/// Context **E**, the annotation slot: reached by putting the cursor back on
/// the lambda and pressing `:`, which no single run of printable characters
/// does.
fn annotation_context() -> AppState {
    let state = context("A empty hole");
    let state = handle_key(key(KeyCode::Up), state);
    handle_key(key(KeyCode::Char(':')), state)
}

/// What the editor looks like: the projection, the slot, and the buffer.
fn observe(state: &AppState) -> String {
    let mut out = String::new();
    match state.slot {
        Slot::Node => {}
        Slot::BinderName => out.push_str("name: "),
        Slot::Annotation => out.push_str("ann: "),
    }
    out.push_str(&program_line(state));
    if !state.entry.is_empty() {
        out.push_str(&format!(" ⟨{}⟩", state.entry));
    }
    out
}

/// Press one character in one context and report what happened.
fn press(context: &AppState, c: char) -> String {
    observe(&handle_key(key(KeyCode::Char(c)), context.clone()))
}

/// Check a whole context's row of the matrix.
fn check(name: &str, state: AppState, rows: &[(char, &str)]) {
    let before = observe(&state);
    for (c, expected) in rows {
        let actual = press(&state, *c);
        assert_eq!(
            &actual, expected,
            "\n{name}, `{c}`:\n  before: {before}\n  wanted: {expected}\n  got:    {actual}\n"
        );
    }
}

/// Column **A** — an empty hole. The whole printable alphabet, including the
/// characters `KEYS.md` deliberately holds in reserve for Phase 6+.
#[test]
fn column_a_empty_hole() {
    check(
        "A empty hole",
        context("A empty hole"),
        &[
            // digits write a literal
            ('0', "λx0:Num. »0«"),
            ('7', "λx0:Num. »7«"),
            // letters start a name run, committed live
            ('x', "λx0:Num. »x0« ⟨x⟩"),
            ('t', "λx0:Num. »true« ⟨t⟩"),
            ('f', "λx0:Num. »false« ⟨f⟩"),
            // …and a run that matches nothing writes nothing
            ('z', "λx0:Num. »⦇⦈« ⟨z⟩"),
            ('_', "λx0:Num. »⦇⦈« ⟨_⟩"),
            ('n', "λx0:Num. »⦇⦈« ⟨n⟩"),
            // operators wrap the hole in place — a hole never climbs — and
            // the cursor lands on the form's first empty child
            ('+', "λx0:Num. »⦇⦈« + ⦇⦈"),
            ('-', "λx0:Num. »⦇⦈« - ⦇⦈"),
            ('*', "λx0:Num. »⦇⦈« * ⦇⦈"),
            ('<', "λx0:Num. »⦇⦈« < ⦇⦈"),
            ('=', "λx0:Num. »⦇⦈« == ⦇⦈"),
            // forms insert themselves; the binders land in their name slot
            (' ', "λx0:Num. »⦇⦈« ⦇⦈"),
            ('\\', "name: λx0:Num. λ»x1«:?. ⦇⦈"),
            ('?', "λx0:Num. if »⦇⦈« then ⦇⦈ else ⦇⦈"),
            (';', "name: λx0:Num. let »x1« = ⦇⦈ in ⦇⦈"),
            (',', "λx0:Num. (»⦇⦈«, ⦇⦈)"),
            ('[', "λx0:Num. fst »⦇⦈«"),
            (']', "λx0:Num. snd »⦇⦈«"),
            ('!', "λx0:Num. ⦇»⦇⦈«⦈"),
            // and the ones that mean nothing here: no-op, with a hint
            ('~', "λx0:Num. »⦇⦈«"),
            (':', "λx0:Num. »⦇⦈«"),
            ('.', "λx0:Num. »⦇⦈«"),
            ('>', "λx0:Num. »⦇⦈«"),
            ('(', "λx0:Num. »⦇⦈«"),
            (')', "λx0:Num. »⦇⦈«"),
            ('@', "λx0:Num. »⦇⦈«"),
        ],
    );
}

/// Column **B** — a written expression. Typing replaces it; operators and
/// forms climb, then wrap; anything that does not fit its new position is
/// quarantined rather than refused.
#[test]
fn column_b_written_expression() {
    check(
        "B written expr",
        context("B written expr"),
        &[
            ('0', "λx0:Num. »0«"),
            ('7', "λx0:Num. »7«"),
            ('x', "λx0:Num. »x0« ⟨x⟩"),
            ('t', "λx0:Num. »true« ⟨t⟩"),
            ('f', "λx0:Num. »false« ⟨f⟩"),
            // an unmatched run leaves the expression alone
            ('z', "λx0:Num. »x0« ⟨z⟩"),
            ('_', "λx0:Num. »x0« ⟨_⟩"),
            ('n', "λx0:Num. »x0« ⟨n⟩"),
            ('+', "λx0:Num. x0 + »⦇⦈«"),
            ('-', "λx0:Num. x0 - »⦇⦈«"),
            ('*', "λx0:Num. x0 * »⦇⦈«"),
            ('<', "λx0:Num. x0 < »⦇⦈«"),
            ('=', "λx0:Num. x0 == »⦇⦈«"),
            // `x0` is not a function, so applying it quarantines it
            (' ', "λx0:Num. ⦇x0⦈ »⦇⦈«"),
            ('\\', "name: λx0:Num. λ»x1«:?. x0"),
            ('?', "λx0:Num. if ⦇x0⦈ then »⦇⦈« else ⦇⦈"),
            (';', "name: λx0:Num. let »x1« = x0 in ⦇⦈"),
            (',', "λx0:Num. (x0, »⦇⦈«)"),
            // …and it is not a pair either; no empty child, so the cursor
            // stays on the projection
            ('[', "λx0:Num. »fst ⦇x0⦈«"),
            (']', "λx0:Num. »snd ⦇x0⦈«"),
            ('!', "λx0:Num. »⦇x0⦈«"),
            ('~', "λx0:Num. »x0«"),
            (':', "λx0:Num. »x0«"),
            ('.', "λx0:Num. »x0«"),
            ('>', "λx0:Num. »x0«"),
            ('(', "λx0:Num. »x0«"),
            (')', "λx0:Num. »x0«"),
            ('@', "λx0:Num. »x0«"),
        ],
    );
}

/// Column **C** — a focused number: the one place a digit appends instead of
/// replacing, and the only place `~` does anything.
#[test]
fn column_c_focused_number() {
    check(
        "C focused Num",
        context("C focused Num"),
        &[
            ('0', "λx0:Num. »120«"),
            ('3', "λx0:Num. »123«"),
            ('x', "λx0:Num. »x0« ⟨x⟩"),
            ('z', "λx0:Num. »12« ⟨z⟩"),
            ('+', "λx0:Num. 12 + »⦇⦈«"),
            (' ', "λx0:Num. ⦇12⦈ »⦇⦈«"),
            ('?', "λx0:Num. if ⦇12⦈ then »⦇⦈« else ⦇⦈"),
            ('[', "λx0:Num. »fst ⦇12⦈«"),
            ('!', "λx0:Num. »⦇12⦈«"),
            ('~', "λx0:Num. »-12«"),
            (':', "λx0:Num. »12«"),
            ('.', "λx0:Num. »12«"),
            ('@', "λx0:Num. »12«"),
        ],
    );
}

/// Column **D** — mid-name run. Identifier characters extend the run and
/// re-commit; everything else acts on what the run already committed, and
/// costs no keystroke for the ending.
#[test]
fn column_d_name_run() {
    check(
        "D mid-name run",
        context("D mid-name run"),
        &[
            ('0', "λx0:Num. »x0« ⟨x0⟩"),
            // `x7` names nothing, so the run's own commit is withdrawn and
            // the hole it started from comes back
            ('7', "λx0:Num. »⦇⦈« ⟨x7⟩"),
            ('x', "λx0:Num. »⦇⦈« ⟨xx⟩"),
            ('_', "λx0:Num. »⦇⦈« ⟨x_⟩"),
            ('+', "λx0:Num. x0 + »⦇⦈«"),
            (' ', "λx0:Num. ⦇x0⦈ »⦇⦈«"),
            ('?', "λx0:Num. if ⦇x0⦈ then »⦇⦈« else ⦇⦈"),
            ('[', "λx0:Num. »fst ⦇x0⦈«"),
            ('!', "λx0:Num. »⦇x0⦈«"),
            ('~', "λx0:Num. »x0«"),
            ('.', "λx0:Num. »x0«"),
            ('@', "λx0:Num. »x0«"),
        ],
    );
}

/// Column **E** — the annotation slot. `n`/`b`/`?` are types, `*` and `>`
/// are the type operators, `.` leaves for the body, and everything else
/// exits *and is reprocessed there* rather than being refused.
#[test]
fn column_e_annotation_slot() {
    check(
        "E annotation",
        annotation_context(),
        &[
            ('n', "ann: λx0:»Num«. ⦇⦈ ⟨n⟩"),
            ('b', "ann: λx0:»Bool«. ⦇⦈ ⟨b⟩"),
            ('?', "ann: λx0:»?«. ⦇⦈ ⟨?⟩"),
            ('*', "ann: λx0:»? * ?«. ⦇⦈ ⟨*⟩"),
            ('>', "ann: λx0:»? -> ?«. ⦇⦈ ⟨>⟩"),
            ('(', "ann: λx0:»?«. ⦇⦈ ⟨(⟩"),
            // a `)` with no `(` is the one inert character here
            (')', "ann: λx0:»Num«. ⦇⦈"),
            // …as is the key that reaches this slot in the first place
            (':', "ann: λx0:»Num«. ⦇⦈"),
            ('.', "λx0:Num. »⦇⦈«"),
            // exit → body, reprocess: one keystroke, not two
            ('0', "λx0:Num. »0«"),
            ('x', "λx0:Num. »x0« ⟨x⟩"),
            ('+', "λx0:Num. »⦇⦈« + ⦇⦈"),
            (' ', "λx0:Num. »⦇⦈« ⦇⦈"),
            ('!', "λx0:Num. ⦇»⦇⦈«⦈"),
            ('~', "λx0:Num. »⦇⦈«"),
            ('@', "λx0:Num. »⦇⦈«"),
        ],
    );
}

/// Column **F** — the binder-name slot. Alphanumerics name the binder (and
/// pre-Phase-5 its digits are its identity); `:`, `=` and `.` address the
/// binder's other parts; everything else exits to the body.
#[test]
fn column_f_binder_name_slot() {
    check(
        "F binder name",
        context("F binder name"),
        &[
            ('0', "name: λ»x0«:?. ⦇⦈ ⟨x0⟩"),
            ('7', "name: λ»x7«:?. ⦇⦈ ⟨x7⟩"),
            // a letter is part of the name but carries no identity yet
            ('y', "name: λ»x0«:?. ⦇⦈ ⟨xy⟩"),
            ('_', "name: λ»x0«:?. ⦇⦈ ⟨x_⟩"),
            (':', "ann: λx0:»?«. ⦇⦈"),
            ('.', "λx0:?. »⦇⦈«"),
            ('~', "name: λ»x0«:?. ⦇⦈ ⟨x⟩"),
            // `=` means "the bound expression" on a `let` only, so on a
            // lambda it exits and is reprocessed as the equality operator
            ('=', "λx0:?. »⦇⦈« == ⦇⦈"),
            ('+', "λx0:?. »⦇⦈« + ⦇⦈"),
            ('?', "λx0:?. if »⦇⦈« then ⦇⦈ else ⦇⦈"),
            ('[', "λx0:?. fst »⦇⦈«"),
            ('@', "λx0:?. »⦇⦈«"),
        ],
    );
}

/// Column **G** — a non-empty hole is transparent to typing: everything is
/// typed at the expression inside it, because you quarantined it to keep
/// editing it. Only `!` addresses the wrapper.
#[test]
fn column_g_non_empty_hole() {
    check(
        "G non-empty hole",
        context("G non-empty hole"),
        &[
            ('0', "λx0:Num. if ⦇»0«⦈ then ⦇⦈ else ⦇⦈"),
            ('t', "λx0:Num. if ⦇»true«⦈ then ⦇⦈ else ⦇⦈ ⟨t⟩"),
            ('x', "λx0:Num. if ⦇»x0«⦈ then ⦇⦈ else ⦇⦈ ⟨x⟩"),
            ('z', "λx0:Num. if ⦇»x0«⦈ then ⦇⦈ else ⦇⦈ ⟨z⟩"),
            ('+', "λx0:Num. if ⦇x0 + »⦇⦈«⦈ then ⦇⦈ else ⦇⦈"),
            ('<', "λx0:Num. if ⦇x0 < »⦇⦈«⦈ then ⦇⦈ else ⦇⦈"),
            (' ', "λx0:Num. if ⦇⦇x0⦈ »⦇⦈«⦈ then ⦇⦈ else ⦇⦈"),
            (',', "λx0:Num. if ⦇(x0, »⦇⦈«)⦈ then ⦇⦈ else ⦇⦈"),
            ('[', "λx0:Num. if ⦇»fst ⦇x0⦈«⦈ then ⦇⦈ else ⦇⦈"),
            // the exception: `!` wraps the wrapper
            ('!', "λx0:Num. if »⦇⦇x0⦈⦈« then ⦇⦈ else ⦇⦈"),
            ('~', "λx0:Num. if ⦇»x0«⦈ then ⦇⦈ else ⦇⦈"),
            ('.', "λx0:Num. if ⦇»x0«⦈ then ⦇⦈ else ⦇⦈"),
            ('@', "λx0:Num. if ⦇»x0«⦈ then ⦇⦈ else ⦇⦈"),
        ],
    );
}

/// The matrix's own summary rule: **no keystroke is ever spent purely on
/// leaving anything**. Every character that exits a slot must also do its
/// ordinary job in the same keystroke.
#[test]
fn exiting_a_slot_costs_no_keystroke() {
    for (slot_state, name) in [
        (annotation_context(), "annotation"),
        (context("F binder name"), "binder name"),
    ] {
        // The `+` that exits must also have wrapped something.
        let after = handle_key(key(KeyCode::Char('+')), slot_state.clone());
        assert_eq!(after.slot, Slot::Node, "{name}: still in the slot");
        assert!(
            program_line(&after).contains(" + "),
            "{name}: `+` exited but did not also construct: {}",
            program_line(&after)
        );
    }
}

/// Every row of the matrix leaves a well-typed program: the Phase 2 theorem,
/// seen from the keyboard. Nothing above can be "explained" by a broken
/// program, because there is no such state to reach.
#[test]
fn no_row_of_the_matrix_can_break_the_program() {
    let alphabet = "0123456789abnxtfyz_+-*<= \\?;,[]!~:.>()@";
    for name in [
        "A empty hole",
        "B written expr",
        "C focused Num",
        "D mid-name run",
        "F binder name",
        "G non-empty hole",
    ] {
        let state = context(name);
        for c in alphabet.chars() {
            let after = handle_key(key(KeyCode::Char(c)), state.clone());
            assert!(
                nothing_core::typing::is_well_typed(&after.program()),
                "{name}, `{c}`: {}",
                program_line(&after)
            );
        }
    }
    let annotation = annotation_context();
    for c in alphabet.chars() {
        let after = handle_key(key(KeyCode::Char(c)), annotation.clone());
        assert!(
            nothing_core::typing::is_well_typed(&after.program()),
            "E annotation, `{c}`: {}",
            program_line(&after)
        );
    }
}
