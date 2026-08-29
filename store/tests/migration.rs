use std::fs;
use std::path::{Path, PathBuf};

use nothing_action::act::Action;
use nothing_action::log::{ActionLog, AuthorId};
use nothing_action::script::replay_script;
use nothing_core::doc::Doc;
use nothing_core::doc::{MAIN_ID, MAIN_NAME};
use nothing_core::exp::Exp;
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;
use nothing_store::document::{Document, VERSION_MAJOR};
use nothing_store::v1::encode_document_v1;
use nothing_store::v2::encode_document_v2;
use nothing_store::v3::encode_document_v3;
use nothing_store::v4::encode_document_v4;
use nothing_store::{decode_document, encode_document};

const FACTORIAL: &str = include_str!("../../bench/fixtures/factorial.actions");
const LIST_MAP: &str = include_str!("../../bench/fixtures/list_map.actions");
const RECORD: &str = include_str!("../../bench/fixtures/record.actions");
const STATE_MACHINE: &str = include_str!("../../bench/fixtures/state_machine.actions");
const NESTED_CONDITIONAL: &str = include_str!("../../bench/fixtures/nested_conditional.actions");
const GREETING: &str = include_str!("../../bench/fixtures/greeting.actions");

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/v1")
}

fn v2_fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/v2")
}

fn v3_fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/v3")
}

fn v4_fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/v4")
}

fn sample_log() -> ActionLog {
    let mut log = ActionLog::new();
    log.append(
        Action::ConstructNum(41),
        1_700_000_000_000,
        AuthorId::new(7),
    );
    log.append(Action::ConstructAp, 1_700_000_000_001, AuthorId::new(7));
    log.append(Action::Finish, 1_700_000_000_002, AuthorId::new(7));
    log
}

fn every_v1_program() -> Vec<(String, Exp, NameTable)> {
    use nothing_core::examples;
    let mut out: Vec<(String, Exp, NameTable)> = vec![
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
    .into_iter()
    .map(|(name, exp)| (name.to_string(), exp, examples::names()))
    .collect();

    for (name, script) in [
        ("bench_factorial", FACTORIAL),
        ("bench_list_map", LIST_MAP),
        ("bench_record", RECORD),
        ("bench_state_machine", STATE_MACHINE),
        ("bench_nested_conditional", NESTED_CONDITIONAL),
    ] {
        let replayed = replay_script(script).expect("the bench fixtures replay cleanly");
        out.push((name.to_string(), replayed.exp(), replayed.names.clone()));
    }

    out
}

fn ensure_fixtures() -> Vec<PathBuf> {
    let dir = fixture_dir();
    fs::create_dir_all(&dir).expect("the fixture directory is creatable");
    let mut paths = Vec::new();
    for (name, exp, names) in every_v1_program() {
        let path = dir.join(format!("{name}.v1.nothing"));
        if !path.exists() {
            let bytes = encode_document_v1(&exp, &names, &sample_log());
            fs::write(&path, bytes).expect("the fixture is writable");
        }
        paths.push(path);
    }
    paths
}

fn v1_artifacts() -> Vec<PathBuf> {
    ensure_fixtures();
    let mut paths: Vec<PathBuf> = fs::read_dir(fixture_dir())
        .expect("the fixture directory exists")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "nothing"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn there_are_version_one_artifacts_to_migrate() {
    let paths = v1_artifacts();
    assert!(
        paths.len() >= 15,
        "only {} v1 artifacts were found; the migration path is barely exercised",
        paths.len()
    );
    for path in &paths {
        let bytes = fs::read(path).expect("the artifact is readable");
        assert_eq!(&bytes[0..4], b"NTHG", "{path:?} is not a nothing file");
        assert_eq!(
            bytes[4], 1,
            "{path:?} is not version 1, so it does not test migration"
        );
    }
}

#[test]
fn every_version_one_artifact_opens_under_version_two() {
    for path in v1_artifacts() {
        let bytes = fs::read(&path).expect("the artifact is readable");
        let doc =
            decode_document(&bytes).unwrap_or_else(|e| panic!("{path:?} failed to migrate: {e:?}"));

        assert_eq!(
            doc.doc.len(),
            1,
            "{path:?} migrated to more than one definition"
        );
        let def = &doc.doc.defs()[0];
        assert_eq!(def.id, MAIN_ID, "{path:?} did not migrate onto the main id");
        assert_eq!(
            def.ann,
            Ty::Hole,
            "{path:?} gained an annotation from nowhere"
        );
        assert_eq!(
            doc.names.get(MAIN_ID),
            Some(MAIN_NAME),
            "{path:?} did not name its definition main"
        );
        assert!(
            doc.doc.is_well_typed(),
            "{path:?} migrated to an ill-typed document"
        );
    }
}

#[test]
fn every_migrated_artifact_round_trips_at_the_current_version() {
    for path in v1_artifacts()
        .into_iter()
        .chain(v2_artifacts())
        .chain(v3_artifacts())
        .chain(v4_artifacts())
    {
        let bytes = fs::read(&path).expect("the artifact is readable");
        let migrated = decode_document(&bytes).expect("the artifact migrates");

        let current = encode_document(&migrated);
        assert_eq!(
            current[4], VERSION_MAJOR,
            "{path:?} did not re-encode at the current version"
        );

        let reopened = decode_document(&current).expect("the re-encoded artifact opens");
        assert_eq!(
            reopened, migrated,
            "{path:?} did not survive the round trip"
        );
        assert_eq!(
            encode_document(&reopened),
            current,
            "{path:?} does not re-encode byte-identically"
        );
    }
}

fn every_v2_program() -> Vec<(String, Doc, NameTable)> {
    every_v1_program()
        .into_iter()
        .map(|(name, exp, names)| (name, Doc::single(exp), names))
        .collect()
}

fn ensure_v2_fixtures() {
    let dir = v2_fixture_dir();
    fs::create_dir_all(&dir).expect("the fixture directory is creatable");
    for (name, doc, names) in every_v2_program() {
        let path = dir.join(format!("{name}.v2.nothing"));
        if !path.exists() {
            let document = Document::from_doc(doc, names, sample_log());
            fs::write(&path, encode_document_v2(&document)).expect("the fixture is writable");
        }
    }
}

fn v2_artifacts() -> Vec<PathBuf> {
    ensure_v2_fixtures();
    let mut paths: Vec<PathBuf> = fs::read_dir(v2_fixture_dir())
        .expect("the fixture directory exists")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "nothing"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn there_are_version_two_artifacts_to_migrate() {
    let paths = v2_artifacts();
    assert!(
        paths.len() >= 15,
        "only {} v2 artifacts were found; the v2 migration path is barely exercised",
        paths.len()
    );
    for path in &paths {
        let bytes = fs::read(path).expect("the artifact is readable");
        assert_eq!(&bytes[0..4], b"NTHG", "{path:?} is not a nothing file");
        assert_eq!(
            bytes[4], 2,
            "{path:?} is not version 2, so it does not test migration"
        );
    }
}

#[test]
fn every_version_two_artifact_opens_under_version_three() {
    for path in v2_artifacts() {
        let bytes = fs::read(&path).expect("the artifact is readable");
        let doc =
            decode_document(&bytes).unwrap_or_else(|e| panic!("{path:?} failed to migrate: {e:?}"));
        assert!(
            doc.doc.is_well_typed(),
            "{path:?} migrated to an ill-typed document"
        );
        assert!(!doc.doc.is_empty(), "{path:?} migrated to no definitions");
    }
}

#[test]
fn a_version_two_document_keeps_every_definition_it_had() {
    for (name, doc, names) in every_v2_program() {
        let before = Document::from_doc(doc, names, sample_log());
        let bytes = encode_document_v2(&before);
        assert_eq!(bytes[4], 2, "{name} was not written as version 2");
        let after = decode_document(&bytes).expect("the v2 bytes migrate");
        assert_eq!(after.doc, before.doc, "{name} lost a definition");
        assert_eq!(after.log, before.log, "{name} lost its action log");
    }
}

fn every_v3_program() -> Vec<(String, Doc, NameTable)> {
    let mut out = every_v2_program();
    let replayed = replay_script(GREETING).expect("the greeting fixture replays cleanly");
    out.push((
        "bench_greeting".to_string(),
        replayed.doc(),
        replayed.names.clone(),
    ));
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn ensure_v3_fixtures() {
    let dir = v3_fixture_dir();
    fs::create_dir_all(&dir).expect("the fixture directory is creatable");
    for (name, doc, names) in every_v3_program() {
        let path = dir.join(format!("{name}.v3.nothing"));
        if !path.exists() {
            let document = Document::from_doc(doc, names, sample_log());
            fs::write(&path, encode_document_v3(&document)).expect("the fixture is writable");
        }
    }
}

fn v3_artifacts() -> Vec<PathBuf> {
    ensure_v3_fixtures();
    let mut paths: Vec<PathBuf> = fs::read_dir(v3_fixture_dir())
        .expect("the fixture directory exists")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "nothing"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn there_are_version_three_artifacts_to_migrate() {
    let paths = v3_artifacts();
    assert!(
        paths.len() >= 16,
        "only {} v3 artifacts were found; the v3 migration path is barely exercised",
        paths.len()
    );
    for path in &paths {
        let bytes = fs::read(path).expect("the artifact is readable");
        assert_eq!(&bytes[0..4], b"NTHG", "{path:?} is not a nothing file");
        assert_eq!(
            bytes[4], 3,
            "{path:?} is not version 3, so it does not test migration"
        );
    }
}

#[test]
fn every_version_three_artifact_opens_under_version_four() {
    for path in v3_artifacts() {
        let bytes = fs::read(&path).expect("the artifact is readable");
        let doc =
            decode_document(&bytes).unwrap_or_else(|e| panic!("{path:?} failed to migrate: {e:?}"));
        assert!(
            doc.doc.is_well_typed(),
            "{path:?} migrated to an ill-typed document"
        );
        assert!(!doc.doc.is_empty(), "{path:?} migrated to no definitions");
    }
}

#[test]
fn a_version_three_document_keeps_every_definition_it_had() {
    for (name, doc, names) in every_v3_program() {
        let before = Document::from_doc(doc, names, sample_log());
        let bytes = encode_document_v3(&before);
        assert_eq!(bytes[4], 3, "{name} was not written as version 3");
        let after = decode_document(&bytes).expect("the v3 bytes migrate");
        assert_eq!(after.doc, before.doc, "{name} lost a definition");
        assert_eq!(after.log, before.log, "{name} lost its action log");
    }
}

fn every_v4_program() -> Vec<(String, Doc, NameTable)> {
    let mut out = every_v3_program();
    out.push((
        "list_sum".to_string(),
        Doc::single(list_sum_program()),
        NameTable::new(),
    ));
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn ensure_v4_fixtures() {
    let dir = v4_fixture_dir();
    fs::create_dir_all(&dir).expect("the fixture directory is creatable");
    for (name, doc, names) in every_v4_program() {
        if doc.defs().iter().any(|def| mentions_a_record(&def.body)) {
            continue;
        }
        let path = dir.join(format!("{name}.v4.nothing"));
        if !path.exists() {
            let document = Document::from_doc(doc, names, sample_log());
            fs::write(&path, encode_document_v4(&document)).expect("the fixture is writable");
        }
    }
}

fn v4_artifacts() -> Vec<PathBuf> {
    ensure_v4_fixtures();
    let mut paths: Vec<PathBuf> = fs::read_dir(v4_fixture_dir())
        .expect("the fixture directory exists")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "nothing"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn there_are_version_four_artifacts_to_migrate() {
    let paths = v4_artifacts();
    assert!(
        paths.len() >= 16,
        "only {} v4 artifacts were found; the v4 migration path is barely exercised",
        paths.len()
    );
    for path in &paths {
        let bytes = fs::read(path).expect("the artifact is readable");
        assert_eq!(&bytes[0..4], b"NTHG", "{path:?} is not a nothing file");
        assert_eq!(
            bytes[4], 4,
            "{path:?} is not version 4, so it does not test migration"
        );
    }
}

#[test]
fn every_version_four_artifact_opens_under_version_five() {
    for path in v4_artifacts() {
        let bytes = fs::read(&path).expect("the artifact is readable");
        let doc =
            decode_document(&bytes).unwrap_or_else(|e| panic!("{path:?} failed to migrate: {e:?}"));
        assert!(
            doc.doc.is_well_typed(),
            "{path:?} migrated to an ill-typed document"
        );
        assert!(!doc.doc.is_empty(), "{path:?} migrated to no definitions");
    }
}

fn mentions_a_record(exp: &Exp) -> bool {
    fn ty_mentions(ty: &Ty) -> bool {
        match ty {
            Ty::Record(_) => true,
            Ty::Arrow(a, b) | Ty::Prod(a, b) => ty_mentions(a) || ty_mentions(b),
            Ty::List(a) => ty_mentions(a),
            Ty::Num | Ty::Bool | Ty::Str | Ty::Hole => false,
        }
    }
    match exp {
        Exp::Record(_) | Exp::Field(_, _) => true,
        Exp::Lam(_, ty, body) => ty_mentions(ty) || mentions_a_record(body),
        Exp::Var(_) | Exp::Num(_) | Exp::Bool(_) | Exp::Str(_) | Exp::Nil | Exp::EmptyHole(_) => {
            false
        }
        Exp::Proj(_, e) | Exp::NonEmptyHole(_, e) => mentions_a_record(e),
        Exp::Ap(a, b)
        | Exp::BinOp(_, a, b)
        | Exp::Let(_, a, b)
        | Exp::Pair(a, b)
        | Exp::Cons(a, b) => mentions_a_record(a) || mentions_a_record(b),
        Exp::If(a, b, c) | Exp::Fold(a, b, c) => {
            mentions_a_record(a) || mentions_a_record(b) || mentions_a_record(c)
        }
    }
}

#[test]
fn no_version_four_artifact_contains_a_version_five_form() {
    for path in v4_artifacts() {
        let bytes = fs::read(&path).expect("the artifact is readable");
        let document = decode_document(&bytes).expect("the artifact decodes");
        for def in document.doc.defs() {
            assert!(
                !mentions_a_record(&def.body),
                "{path:?} contains a record, so it is not bytes a version-4 build could have \
                 written and it does not test the migration it claims to"
            );
        }
    }
}

#[test]
fn a_version_four_document_keeps_every_definition_it_had() {
    for (name, doc, names) in every_v4_program() {
        let before = Document::from_doc(doc, names, sample_log());
        let bytes = encode_document_v4(&before);
        assert_eq!(bytes[4], 4, "{name} was not written as version 4");
        let after = decode_document(&bytes).expect("the v4 bytes migrate");
        assert_eq!(after.doc, before.doc, "{name} lost a definition");
        assert_eq!(after.log, before.log, "{name} lost its action log");
    }
}

fn list_sum_program() -> Exp {
    Exp::fold(
        Exp::list([Exp::num(1), Exp::num(2), Exp::num(3)]),
        Exp::num(0),
        Exp::lam(
            nothing_core::exp::Id::from_u128(1),
            Ty::Num,
            Exp::lam(
                nothing_core::exp::Id::from_u128(2),
                Ty::Num,
                Exp::bin_op(
                    nothing_core::exp::Op::Add,
                    Exp::var(nothing_core::exp::Id::from_u128(1)),
                    Exp::var(nothing_core::exp::Id::from_u128(2)),
                ),
            ),
        ),
    )
}

#[test]
fn a_version_four_file_carries_a_list_no_earlier_version_could() {
    let program = list_sum_program();
    let document = Document::new(program.clone(), NameTable::new(), sample_log());
    let bytes = encode_document(&document);
    assert_eq!(bytes[4], VERSION_MAJOR);
    let reopened = decode_document(&bytes).expect("a list document opens");
    assert_eq!(reopened.exp(), program);
    assert_eq!(
        nothing_core::typing::syn(&nothing_core::ctx::Ctx::empty(), &program),
        Some(Ty::Num),
        "the fixture must be the well-typed sum of a literal list"
    );
}

#[test]
fn a_version_three_file_carries_a_string_no_earlier_version_could() {
    use nothing_core::exp::{Exp, Op};
    let program = Exp::bin_op(
        Op::Concat,
        Exp::str_("hello, "),
        Exp::bin_op(Op::Concat, Exp::str_("wor\"ld"), Exp::str_("\\")),
    );
    let document = Document::new(program.clone(), NameTable::new(), sample_log());
    let bytes = encode_document(&document);
    assert_eq!(bytes[4], VERSION_MAJOR);
    let reopened = decode_document(&bytes).expect("a string document opens");
    assert_eq!(reopened.exp(), program);
}

#[test]
fn the_migrated_body_is_exactly_the_version_one_expression() {
    for (name, exp, names) in every_v1_program() {
        let bytes = encode_document_v1(&exp, &names, &sample_log());
        let migrated = decode_document(&bytes).expect("the bytes migrate");
        assert_eq!(migrated.exp(), exp, "{name} lost its expression");
        assert_eq!(migrated.log, sample_log(), "{name} lost its action log");
        for (id, display) in names.flatten().entries() {
            assert_eq!(
                migrated.names.get(id),
                Some(display.as_str()),
                "{name} lost the name of {id}"
            );
        }
    }
}
