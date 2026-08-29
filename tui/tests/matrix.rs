use nothing_tui::keys::{handle_key, key};
use nothing_tui::render::program_line;
use nothing_tui::{AppState, Slot};

use crossterm::event::KeyCode;

fn context(name: &str) -> AppState {
    let typed = |text: &str| {
        text.chars().fold(AppState::empty(), |state, c| {
            handle_key(key(KeyCode::Char(c)), state)
        })
    };
    match name {
        "A empty hole" => typed("\\x0:n."),

        "B written expr" => handle_key(key(KeyCode::Esc), typed("\\x0:n.x0")),

        "C focused Num" => typed("\\x0:n.12"),

        "D mid-name run" => typed("\\x0:n.x"),

        "F binder name" => typed("\\x"),

        "G non-empty hole" => handle_key(key(KeyCode::Esc), typed("\\x0:n.?x")),

        "H inside a string" => typed("\\x0:s.\"ab"),
        other => panic!("unknown context {other}"),
    }
}

fn annotation_context() -> AppState {
    let state = context("A empty hole");
    let state = handle_key(key(KeyCode::Up), state);
    handle_key(key(KeyCode::Char(':')), state)
}

fn observe(state: &AppState) -> String {
    let mut out = String::new();
    match state.slot {
        Slot::Node => {}
        Slot::BinderName => out.push_str("name: "),
        Slot::Annotation => out.push_str("ann: "),
        Slot::DefName => out.push_str("def: "),
        Slot::DefAnn => out.push_str("def ann: "),
        Slot::FieldName => out.push_str("field: "),
        Slot::FieldPick => out.push_str("pick: "),
        Slot::ConstructorName => out.push_str("ctor: "),
        Slot::ConstructorPick => out.push_str("tag: "),
    }
    out.push_str(&program_line(state));
    if !state.entry.is_empty() {
        out.push_str(&format!(" ⟨{}⟩", state.entry));
    }
    out
}

fn press(context: &AppState, c: char) -> String {
    observe(&handle_key(key(KeyCode::Char(c)), context.clone()))
}

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

#[test]
fn column_a_empty_hole() {
    check(
        "A empty hole",
        context("A empty hole"),
        &[
            ('0', "λx0:Num. »0«"),
            ('7', "λx0:Num. »7«"),
            ('x', "λx0:Num. »x0« ⟨x⟩"),
            ('t', "λx0:Num. »true« ⟨t⟩"),
            ('f', "λx0:Num. »false« ⟨f⟩"),
            ('z', "λx0:Num. »⦇⦈« ⟨z⟩"),
            ('_', "λx0:Num. »⦇⦈« ⟨_⟩"),
            ('n', "λx0:Num. »nil« ⟨n⟩"),
            ('+', "λx0:Num. »⦇⦈« + ⦇⦈"),
            ('-', "λx0:Num. »⦇⦈« - ⦇⦈"),
            ('*', "λx0:Num. »⦇⦈« * ⦇⦈"),
            ('<', "λx0:Num. »⦇⦈« < ⦇⦈"),
            ('=', "λx0:Num. »⦇⦈« == ⦇⦈"),
            ('&', "λx0:Num. »⦇⦈« ++ ⦇⦈"),
            ('"', "λx0:Num. »\"\"«"),
            (' ', "λx0:Num. »⦇⦈« ⦇⦈"),
            ('\\', "name: λx0:Num. λ»x1«:?. ⦇⦈"),
            ('?', "λx0:Num. if »⦇⦈« then ⦇⦈ else ⦇⦈"),
            (';', "name: λx0:Num. let »x1« = ⦇⦈ in ⦇⦈"),
            (',', "λx0:Num. (»⦇⦈«, ⦇⦈)"),
            ('[', "λx0:Num. fst »⦇⦈«"),
            (']', "λx0:Num. snd »⦇⦈«"),
            ('/', "λx0:Num. fold »⦇⦈« ⦇⦈ ⦇⦈"),
            ('!', "λx0:Num. ⦇»⦇⦈«⦈"),
            ('~', "λx0:Num. »⦇⦈«"),
            (':', "λx0:Num. »⦇⦈« :: ⦇⦈"),
            ('.', "λx0:Num. »⦇⦈«"),
            ('{', "field: λx0:Num. {f0 = »⦇⦈«}"),
            ('`', "tag: λx0:Num. »`C0 ⦇⦈«"),
            ('|', "λx0:Num. match »⦇⦈« {}"),
            ('>', "λx0:Num. »⦇⦈«"),
            ('(', "λx0:Num. »⦇⦈«"),
            (')', "λx0:Num. »⦇⦈«"),
            ('@', "λx0:Num. »⦇⦈«"),
        ],
    );
}

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
            ('z', "λx0:Num. »x0« ⟨z⟩"),
            ('_', "λx0:Num. »x0« ⟨_⟩"),
            ('n', "λx0:Num. »nil« ⟨n⟩"),
            ('+', "λx0:Num. x0 + »⦇⦈«"),
            ('-', "λx0:Num. x0 - »⦇⦈«"),
            ('*', "λx0:Num. x0 * »⦇⦈«"),
            ('<', "λx0:Num. x0 < »⦇⦈«"),
            ('=', "λx0:Num. x0 == »⦇⦈«"),
            ('&', "λx0:Num. ⦇x0⦈ ++ »⦇⦈«"),
            ('"', "λx0:Num. »\"\"«"),
            (' ', "λx0:Num. ⦇x0⦈ »⦇⦈«"),
            ('\\', "name: λx0:Num. λ»x1«:?. x0"),
            ('?', "λx0:Num. if ⦇x0⦈ then »⦇⦈« else ⦇⦈"),
            (';', "name: λx0:Num. let »x1« = x0 in ⦇⦈"),
            (',', "λx0:Num. (x0, »⦇⦈«)"),
            ('[', "λx0:Num. »fst ⦇x0⦈«"),
            (']', "λx0:Num. »snd ⦇x0⦈«"),
            ('/', "λx0:Num. fold ⦇x0⦈ »⦇⦈« ⦇⦈"),
            ('!', "λx0:Num. »⦇x0⦈«"),
            ('~', "λx0:Num. »x0«"),
            (':', "λx0:Num. x0 :: »⦇⦈«"),
            ('.', "λx0:Num. »x0«"),
            ('{', "field: λx0:Num. {f0 = »x0«}"),
            ('`', "tag: λx0:Num. »`C0 x0«"),
            ('|', "λx0:Num. »match ⦇x0⦈ {}«"),
            ('>', "λx0:Num. »x0«"),
            ('(', "λx0:Num. »x0«"),
            (')', "λx0:Num. »x0«"),
            ('@', "λx0:Num. »x0«"),
        ],
    );
}

#[test]
fn column_c_focused_number() {
    check(
        "C focused Num",
        context("C focused Num"),
        &[
            ('0', "λx0:Num. »120«"),
            ('3', "λx0:Num. »123«"),
            ('x', "λx0:Num. »x0« ⟨x⟩"),
            ('n', "λx0:Num. »nil« ⟨n⟩"),
            ('z', "λx0:Num. »12« ⟨z⟩"),
            ('+', "λx0:Num. 12 + »⦇⦈«"),
            ('&', "λx0:Num. ⦇12⦈ ++ »⦇⦈«"),
            ('"', "λx0:Num. »\"\"«"),
            (' ', "λx0:Num. ⦇12⦈ »⦇⦈«"),
            ('?', "λx0:Num. if ⦇12⦈ then »⦇⦈« else ⦇⦈"),
            ('[', "λx0:Num. »fst ⦇12⦈«"),
            ('/', "λx0:Num. fold ⦇12⦈ »⦇⦈« ⦇⦈"),
            ('!', "λx0:Num. »⦇12⦈«"),
            ('~', "λx0:Num. »-12«"),
            (':', "λx0:Num. 12 :: »⦇⦈«"),
            ('.', "λx0:Num. »12«"),
            ('{', "field: λx0:Num. {f0 = »12«}"),
            ('`', "tag: λx0:Num. »`C0 12«"),
            ('|', "λx0:Num. »match ⦇12⦈ {}«"),
            ('@', "λx0:Num. »12«"),
        ],
    );
}

#[test]
fn column_d_name_run() {
    check(
        "D mid-name run",
        context("D mid-name run"),
        &[
            ('0', "λx0:Num. »x0« ⟨x0⟩"),
            ('7', "λx0:Num. »⦇⦈« ⟨x7⟩"),
            ('x', "λx0:Num. »⦇⦈« ⟨xx⟩"),
            ('n', "λx0:Num. »⦇⦈« ⟨xn⟩"),
            ('_', "λx0:Num. »⦇⦈« ⟨x_⟩"),
            ('+', "λx0:Num. x0 + »⦇⦈«"),
            ('&', "λx0:Num. ⦇x0⦈ ++ »⦇⦈«"),
            ('"', "λx0:Num. »\"\"«"),
            (' ', "λx0:Num. ⦇x0⦈ »⦇⦈«"),
            ('?', "λx0:Num. if ⦇x0⦈ then »⦇⦈« else ⦇⦈"),
            ('[', "λx0:Num. »fst ⦇x0⦈«"),
            ('/', "λx0:Num. fold ⦇x0⦈ »⦇⦈« ⦇⦈"),
            ('!', "λx0:Num. »⦇x0⦈«"),
            ('~', "λx0:Num. »x0«"),
            (':', "λx0:Num. x0 :: »⦇⦈«"),
            ('.', "λx0:Num. »x0«"),
            ('{', "field: λx0:Num. {f0 = »x0«}"),
            ('`', "tag: λx0:Num. »`C0 x0«"),
            ('|', "λx0:Num. »match ⦇x0⦈ {}«"),
            ('@', "λx0:Num. »x0«"),
        ],
    );
}

#[test]
fn column_e_annotation_slot() {
    check(
        "E annotation",
        annotation_context(),
        &[
            ('n', "ann: λx0:»Num«. ⦇⦈ ⟨n⟩"),
            ('b', "ann: λx0:»Bool«. ⦇⦈ ⟨b⟩"),
            ('s', "ann: λx0:»Str«. ⦇⦈ ⟨s⟩"),
            ('?', "ann: λx0:»?«. ⦇⦈ ⟨?⟩"),
            ('*', "ann: λx0:»? * ?«. ⦇⦈ ⟨*⟩"),
            ('>', "ann: λx0:»? -> ?«. ⦇⦈ ⟨>⟩"),
            ('(', "ann: λx0:»?«. ⦇⦈ ⟨(⟩"),
            ('[', "ann: λx0:»List ?«. ⦇⦈ ⟨[⟩"),
            (')', "ann: λx0:»Num«. ⦇⦈"),
            (':', "ann: λx0:»Num«. ⦇⦈"),
            ('.', "λx0:Num. »⦇⦈«"),
            ('{', "field: λx0:Num. {f0 = »⦇⦈«}"),
            ('`', "tag: λx0:Num. »`C0 ⦇⦈«"),
            ('|', "λx0:Num. match »⦇⦈« {}"),
            ('0', "λx0:Num. »0«"),
            ('x', "λx0:Num. »x0« ⟨x⟩"),
            ('+', "λx0:Num. »⦇⦈« + ⦇⦈"),
            ('&', "λx0:Num. »⦇⦈« ++ ⦇⦈"),
            ('"', "λx0:Num. »\"\"«"),
            (' ', "λx0:Num. »⦇⦈« ⦇⦈"),
            ('/', "λx0:Num. fold »⦇⦈« ⦇⦈ ⦇⦈"),
            ('!', "λx0:Num. ⦇»⦇⦈«⦈"),
            ('~', "λx0:Num. »⦇⦈«"),
            ('@', "λx0:Num. »⦇⦈«"),
        ],
    );
}

#[test]
fn column_f_binder_name_slot() {
    check(
        "F binder name",
        context("F binder name"),
        &[
            ('0', "name: λ»x0«:?. ⦇⦈ ⟨x0⟩"),
            ('7', "name: λ»x7«:?. ⦇⦈ ⟨x7⟩"),
            ('y', "name: λ»xy«:?. ⦇⦈ ⟨xy⟩"),
            ('_', "name: λ»x_«:?. ⦇⦈ ⟨x_⟩"),
            (':', "ann: λx:»?«. ⦇⦈"),
            ('.', "λx:?. »⦇⦈«"),
            ('{', "field: λx:?. {f0 = »⦇⦈«}"),
            ('`', "tag: λx:?. »`C0 ⦇⦈«"),
            ('|', "λx:?. match »⦇⦈« {}"),
            ('~', "name: λ»x«:?. ⦇⦈ ⟨x⟩"),
            ('=', "λx:?. »⦇⦈« == ⦇⦈"),
            ('&', "λx:?. »⦇⦈« ++ ⦇⦈"),
            ('"', "λx:?. »\"\"«"),
            ('+', "λx:?. »⦇⦈« + ⦇⦈"),
            ('?', "λx:?. if »⦇⦈« then ⦇⦈ else ⦇⦈"),
            ('[', "λx:?. fst »⦇⦈«"),
            ('/', "λx:?. fold »⦇⦈« ⦇⦈ ⦇⦈"),
            ('@', "λx:?. »⦇⦈«"),
        ],
    );
}

#[test]
fn column_g_non_empty_hole() {
    check(
        "G non-empty hole",
        context("G non-empty hole"),
        &[
            ('0', "λx0:Num. if ⦇»0«⦈ then ⦇⦈ else ⦇⦈"),
            ('t', "λx0:Num. if ⦇»true«⦈ then ⦇⦈ else ⦇⦈ ⟨t⟩"),
            ('n', "λx0:Num. if ⦇»nil«⦈ then ⦇⦈ else ⦇⦈ ⟨n⟩"),
            ('x', "λx0:Num. if ⦇»x0«⦈ then ⦇⦈ else ⦇⦈ ⟨x⟩"),
            ('z', "λx0:Num. if ⦇»x0«⦈ then ⦇⦈ else ⦇⦈ ⟨z⟩"),
            ('+', "λx0:Num. if ⦇x0 + »⦇⦈«⦈ then ⦇⦈ else ⦇⦈"),
            ('<', "λx0:Num. if ⦇x0 < »⦇⦈«⦈ then ⦇⦈ else ⦇⦈"),
            ('&', "λx0:Num. if ⦇⦇x0⦈ ++ »⦇⦈«⦈ then ⦇⦈ else ⦇⦈"),
            ('"', "λx0:Num. if ⦇»\"\"«⦈ then ⦇⦈ else ⦇⦈"),
            (' ', "λx0:Num. if ⦇⦇x0⦈ »⦇⦈«⦈ then ⦇⦈ else ⦇⦈"),
            (',', "λx0:Num. if ⦇(x0, »⦇⦈«)⦈ then ⦇⦈ else ⦇⦈"),
            ('[', "λx0:Num. if ⦇»fst ⦇x0⦈«⦈ then ⦇⦈ else ⦇⦈"),
            ('/', "λx0:Num. if ⦇fold ⦇x0⦈ »⦇⦈« ⦇⦈⦈ then ⦇⦈ else ⦇⦈"),
            (':', "λx0:Num. if ⦇x0 :: »⦇⦈«⦈ then ⦇⦈ else ⦇⦈"),
            ('!', "λx0:Num. if »⦇⦇x0⦈⦈« then ⦇⦈ else ⦇⦈"),
            ('~', "λx0:Num. if ⦇»x0«⦈ then ⦇⦈ else ⦇⦈"),
            ('.', "λx0:Num. if ⦇»x0«⦈ then ⦇⦈ else ⦇⦈"),
            ('{', "field: λx0:Num. if ⦇{f0 = »x0«}⦈ then ⦇⦈ else ⦇⦈"),
            ('`', "tag: λx0:Num. if ⦇»`C0 x0«⦈ then ⦇⦈ else ⦇⦈"),
            ('|', "λx0:Num. if ⦇»match ⦇x0⦈ {}«⦈ then ⦇⦈ else ⦇⦈"),
            ('@', "λx0:Num. if ⦇»x0«⦈ then ⦇⦈ else ⦇⦈"),
        ],
    );
}

#[test]
fn column_h_inside_a_string() {
    check(
        "H inside a string",
        context("H inside a string"),
        &[
            ('0', "λx0:Str. »\"ab0\"«"),
            ('z', "λx0:Str. »\"abz\"«"),
            ('+', "λx0:Str. »\"ab+\"«"),
            ('&', "λx0:Str. »\"ab&\"«"),
            (' ', "λx0:Str. »\"ab \"«"),
            ('?', "λx0:Str. »\"ab?\"«"),
            (';', "λx0:Str. »\"ab;\"«"),
            (',', "λx0:Str. »\"ab,\"«"),
            ('[', "λx0:Str. »\"ab[\"«"),
            (']', "λx0:Str. »\"ab]\"«"),
            ('/', "λx0:Str. »\"ab/\"«"),
            ('!', "λx0:Str. »\"ab!\"«"),
            ('~', "λx0:Str. »\"ab~\"«"),
            (':', "λx0:Str. »\"ab:\"«"),
            ('.', "λx0:Str. »\"ab.\"«"),
            ('{', "λx0:Str. »\"ab{\"«"),
            ('`', "λx0:Str. »\"ab`\"«"),
            ('|', "λx0:Str. »\"ab|\"«"),
            ('>', "λx0:Str. »\"ab>\"«"),
            ('(', "λx0:Str. »\"ab(\"«"),
            (')', "λx0:Str. »\"ab)\"«"),
            ('@', "λx0:Str. »\"ab@\"«"),
            ('\\', "λx0:Str. »\"ab\\\"«"),
            ('"', "λx0:Str. »\"ab\"«"),
        ],
    );

    let open = context("H inside a string");
    assert!(open.string_open, "the run is open before the closing quote");
    assert!(
        !handle_key(key(KeyCode::Char('"')), open.clone()).string_open,
        "a quote closes the run"
    );
    assert!(
        handle_key(key(KeyCode::Char('z')), open.clone()).string_open,
        "an ordinary character leaves the run open"
    );
    assert!(
        handle_key(key(KeyCode::Char('\\')), open).escape_armed,
        "a backslash arms the escape"
    );
}

#[test]
fn exiting_a_slot_costs_no_keystroke() {
    for (slot_state, name) in [
        (annotation_context(), "annotation"),
        (context("F binder name"), "binder name"),
    ] {
        let after = handle_key(key(KeyCode::Char('+')), slot_state.clone());
        assert_eq!(after.slot, Slot::Node, "{name}: still in the slot");
        assert!(
            program_line(&after).contains(" + "),
            "{name}: `+` exited but did not also construct: {}",
            program_line(&after)
        );
    }
}

#[test]
fn no_row_of_the_matrix_can_break_the_program() {
    let alphabet = "0123456789abnsxtfyz_+-*<=& \\?;,[]/!~:.>()@{}\"`|";
    for name in [
        "A empty hole",
        "B written expr",
        "C focused Num",
        "D mid-name run",
        "F binder name",
        "G non-empty hole",
        "H inside a string",
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
