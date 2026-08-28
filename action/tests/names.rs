use nothing_action::act::{Action, EditState};
use nothing_action::log::{ActionLog, AuthorId, EditSession};
use nothing_core::exp::{Exp, Id, Op};
use nothing_core::names::NameTable;
use nothing_core::render::render;
use nothing_core::ty::Ty;
use nothing_core::typing::{is_well_typed, syn};

const USES: usize = 40;

fn binder_of(exp: &Exp) -> Id {
    match exp {
        Exp::Let(id, _, _) | Exp::Lam(id, _, _) => *id,
        other => panic!("expected a binder, got {other:?}"),
    }
}

fn session_with_a_binder_used_forty_times() -> (EditSession, Id) {
    let mut session = EditSession::new();
    let mut clock = 0u64;
    let author = AuthorId::new(1);
    let mut apply = |session: &mut EditSession, action: Action| {
        clock += 1;
        assert!(
            session.apply(action.clone(), clock, author),
            "{action:?} did not apply"
        );
    };

    apply(&mut session, Action::ConstructLet);
    apply(&mut session, Action::ConstructNum(1));
    apply(&mut session, Action::MoveNextSibling);

    let id = binder_of(&session.exp());

    apply(&mut session, Action::ConstructVar(id));
    for _ in 1..USES {
        apply(&mut session, Action::ConstructBinOp(Op::Add));
        apply(&mut session, Action::ConstructVar(id));
    }

    (session, id)
}

fn occurrences(text: &str, name: &str) -> usize {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| *word == name)
        .count()
}

#[test]
fn renaming_a_binder_used_forty_times_is_one_action_that_cannot_fail() {
    let (mut session, id) = session_with_a_binder_used_forty_times();

    let before = session.state().render();
    assert_eq!(
        occurrences(&before, "x0"),
        USES + 1,
        "the fixture must really use the binder forty times: {before}"
    );

    let tree_before = session.exp();
    let entries_before = session.log().len();

    assert!(
        session.apply(
            Action::Rename(id, "total".to_string()),
            9_000,
            AuthorId::new(2)
        ),
        "a rename is a name-table write: it cannot fail"
    );

    let after = session.state().render();
    assert_eq!(
        occurrences(&after, "total"),
        USES + 1,
        "one rename must reach every occurrence: {after}"
    );
    assert_eq!(occurrences(&after, "x0"), 0, "{after}");
    assert_eq!(
        after,
        before.replace("x0", "total"),
        "nothing but the name changed"
    );

    assert_eq!(
        session.log().len(),
        entries_before + 1,
        "forty renamed occurrences, one action-log entry"
    );
    assert_eq!(
        session.log().entries().last().map(|e| e.action.clone()),
        Some(Action::Rename(id, "total".to_string()))
    );
    assert_eq!(
        session.exp(),
        tree_before,
        "the AST is untouched by a rename"
    );
}

#[test]
fn a_rename_never_fails_whatever_the_name() {
    let (mut session, id) = session_with_a_binder_used_forty_times();
    let tree = session.exp();

    for (i, name) in ["total", "x0", "", "total", "items"]
        .into_iter()
        .enumerate()
    {
        assert!(
            session.apply(
                Action::Rename(id, name.to_string()),
                i as u64,
                AuthorId::new(3)
            ),
            "renaming to `{name}` was refused"
        );
        assert_eq!(session.state().names().get(id), Some(name));
        assert_eq!(session.exp(), tree);
    }
}

#[test]
fn undo_walks_a_rename_back_and_redo_replays_it() {
    let (mut session, id) = session_with_a_binder_used_forty_times();
    let before = session.state().render();

    assert!(session.apply(Action::Rename(id, "total".to_string()), 1, AuthorId::new(4)));
    let after = session.state().render();

    assert!(session.undo());
    assert_eq!(
        session.state().render(),
        before,
        "undo restores the old name"
    );

    assert!(session.redo());
    assert_eq!(session.state().render(), after, "and redo writes it again");
}

#[test]
fn the_log_replays_names_as_well_as_structure() {
    let (session, id) = session_with_a_binder_used_forty_times();
    let mut log = ActionLog::new();
    for entry in session.log().entries() {
        log.append(entry.action.clone(), entry.timestamp, entry.author);
    }
    log.append(Action::Rename(id, "total".to_string()), 1, AuthorId::new(5));

    let replayed = log.replay();
    assert_eq!(replayed.exp(), session.exp(), "the tree replays exactly");
    assert_eq!(
        occurrences(&replayed.render(), "total"),
        USES + 1,
        "and so does the name"
    );
}

fn shadowing_program() -> (Exp, NameTable, Id, Id) {
    let outer = Id::from_u128(0x0001);
    let inner = Id::from_u128(0x0002);
    let program = Exp::let_(
        outer,
        Exp::num(1),
        Exp::let_(inner, Exp::num(2), Exp::var(inner)),
    );

    let mut names = NameTable::new();
    names.set(outer, "x");
    names.set(inner, "x");

    (program, names, outer, inner)
}

#[test]
fn two_bindings_displayed_as_x_typecheck_and_stay_distinguishable_by_id() {
    let (program, names, outer, inner) = shadowing_program();

    assert!(is_well_typed(&program));
    assert_eq!(
        syn(&nothing_core::ctx::Ctx::empty(), &program),
        Some(Ty::Num)
    );

    assert_eq!(render(&program, &names), "let x = 1 in let x = 2 in x");
    assert_eq!(names.display(outer), names.display(inner));
    assert_ne!(outer, inner, "one display name, two identities");

    let mut env: Vec<(Id, i64)> = Vec::new();
    let mut cur = &program;
    let value = loop {
        match cur {
            Exp::Let(id, bound, body) => {
                match **bound {
                    Exp::Num(n) => env.push((*id, n)),
                    ref other => panic!("expected a literal, got {other:?}"),
                }
                cur = body;
            }
            Exp::Var(id) => {
                break env
                    .iter()
                    .rev()
                    .find(|(bound, _)| bound == id)
                    .map(|(_, n)| *n)
                    .expect("the variable is bound by identity, not by name");
            }
            other => panic!("unexpected form {other:?}"),
        }
    };

    assert_eq!(
        value, 2,
        "`x` is the inner binding, so evaluation must yield 2"
    );
}

#[test]
fn renaming_one_of_two_shadowed_bindings_tells_them_apart() {
    let (program, names, outer, _) = shadowing_program();
    let mut state = EditState::with_names(program, names);

    assert!(state.apply_mut(Action::Rename(outer, "shadowed".to_string())));
    assert_eq!(state.render(), "let shadowed = 1 in let x = 2 in x");
}

fn overlay_program() -> (Exp, NameTable, Id, Id) {
    let f = Id::from_u128(0x00f0);
    let xs = Id::from_u128(0x00f1);
    let program = Exp::lam(
        f,
        Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Num)),
        Exp::lam(
            xs,
            Ty::Num,
            Exp::ap(Exp::var(f), Exp::bin_op(Op::Add, Exp::var(xs), Exp::num(1))),
        ),
    );

    let mut shared = NameTable::new();
    shared.set(f, "f");
    shared.set(xs, "xs");

    (program, shared, f, xs)
}

#[test]
fn two_overlays_render_one_ast_under_two_vocabularies_and_both_round_trip() {
    let (program, shared, f, xs) = overlay_program();

    let mine = NameTable::overlay(&shared);

    let mut theirs = NameTable::overlay(&shared);
    theirs.set(xs, "items");
    theirs.set(f, "apply");

    let my_text = render(&program, &mine);
    let their_text = render(&program, &theirs);

    assert_eq!(my_text, "λf:Num -> Num. λxs:Num. f (xs + 1)");
    assert_eq!(
        their_text,
        "λapply:Num -> Num. λitems:Num. apply (items + 1)"
    );
    assert_ne!(my_text, their_text, "one AST, two vocabularies");

    assert_eq!(
        shared.get(xs),
        Some("xs"),
        "neither overlay wrote through to the shared table"
    );
    assert_eq!(mine.get(xs), Some("xs"));
    assert_eq!(theirs.get(xs), Some("items"));

    for (overlay, text) in [(&mine, &my_text), (&theirs, &their_text)] {
        let flat = overlay.flatten();
        assert_eq!(&render(&program, &flat), text, "flattening changed a name");
        let restacked = NameTable::overlay(&flat);
        assert_eq!(
            &render(&program, &restacked),
            text,
            "re-layering changed a name"
        );
        for id in [f, xs] {
            assert_eq!(flat.display(id), overlay.display(id));
        }
    }
}

#[test]
fn an_overlay_survives_editing_and_keeps_the_other_overlays_names() {
    let (program, shared, _, xs) = overlay_program();

    let mut theirs = NameTable::overlay(&shared);
    theirs.set(xs, "items");

    let mut state = EditState::with_names(program.clone(), theirs);
    assert!(state.apply_mut(Action::MoveChild(0)));
    assert!(state.apply_mut(Action::MoveChild(0)));
    assert!(state.apply_mut(Action::MoveChild(1)));
    assert!(state.apply_mut(Action::ConstructBinOp(Op::Mul)));
    assert!(state.apply_mut(Action::ConstructNum(2)));

    assert_eq!(
        state.render(),
        "λf:Num -> Num. λitems:Num. f ((items + 1) * 2)"
    );
    assert_eq!(
        render(&state.exp(), &shared),
        "λf:Num -> Num. λxs:Num. f ((xs + 1) * 2)",
        "the same edited AST still reads as `xs` under the shared table"
    );
}
