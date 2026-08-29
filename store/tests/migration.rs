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
use nothing_store::v5::encode_document_v5;
use nothing_store::v6::encode_document_v6;
use nothing_store::v7::encode_document_v7;
use nothing_store::{decode_document, encode_document};

const FACTORIAL: &str = include_str!("../../bench/fixtures/factorial.actions");
const LIST_MAP: &str = include_str!("../../bench/fixtures/list_map.actions");
const RECORD: &str = include_str!("../../bench/fixtures/record.actions");
const STATE_MACHINE: &str = include_str!("../../bench/fixtures/state_machine.actions");
const NESTED_CONDITIONAL: &str = include_str!("../../bench/fixtures/nested_conditional.actions");
const GREETING: &str = include_str!("../../bench/fixtures/greeting.actions");
const GREETING_COMMAND: &str = include_str!("../../bench/fixtures/greeting_command.actions");

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

fn v5_fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/v5")
}

fn v6_fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/v6")
}

fn v7_fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/v7")
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
        .chain(v5_artifacts())
        .chain(v6_artifacts())
        .chain(v7_artifacts())
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

fn mentions(exp: &Exp, ty_hit: fn(&Ty) -> bool, exp_hit: fn(&Exp) -> bool) -> bool {
    fn ty_mentions(ty: &Ty, hit: fn(&Ty) -> bool) -> bool {
        if hit(ty) {
            return true;
        }
        match ty {
            Ty::Arrow(a, b) | Ty::Prod(a, b) => ty_mentions(a, hit) || ty_mentions(b, hit),
            Ty::List(a) | Ty::Cmd(a) => ty_mentions(a, hit),
            Ty::Record(fields) | Ty::Variant(fields) => {
                fields.iter().any(|(_, ty)| ty_mentions(ty, hit))
            }
            Ty::Num | Ty::Bool | Ty::Str | Ty::Hole => false,
        }
    }
    if exp_hit(exp) {
        return true;
    }
    let go = |e: &Exp| mentions(e, ty_hit, exp_hit);
    match exp {
        Exp::Lam(_, ty, body) => ty_mentions(ty, ty_hit) || go(body),
        Exp::Var(_)
        | Exp::Num(_)
        | Exp::Bool(_)
        | Exp::Str(_)
        | Exp::Nil
        | Exp::Readline
        | Exp::EmptyHole(_) => false,
        Exp::Proj(_, e)
        | Exp::Field(e, _)
        | Exp::Inj(_, e)
        | Exp::Print(e)
        | Exp::CmdPure(e)
        | Exp::NonEmptyHole(_, e) => go(e),
        Exp::Ap(a, b)
        | Exp::BinOp(_, a, b)
        | Exp::Let(_, a, b)
        | Exp::Pair(a, b)
        | Exp::CmdBind(a, _, b)
        | Exp::Cons(a, b) => go(a) || go(b),
        Exp::If(a, b, c) | Exp::Fold(a, b, c) => go(a) || go(b) || go(c),
        Exp::Record(fields) => fields.iter().any(|(_, value)| go(value)),
        Exp::Match(scrutinee, arms) => go(scrutinee) || arms.iter().any(|(_, _, body)| go(body)),
    }
}

fn mentions_a_record(exp: &Exp) -> bool {
    mentions(
        exp,
        |ty| matches!(ty, Ty::Record(_)),
        |e| matches!(e, Exp::Record(_) | Exp::Field(_, _)),
    )
}

fn mentions_a_variant(exp: &Exp) -> bool {
    mentions(
        exp,
        |ty| matches!(ty, Ty::Variant(_)),
        |e| matches!(e, Exp::Inj(_, _) | Exp::Match(_, _)),
    )
}

fn mentions_a_command(exp: &Exp) -> bool {
    mentions(
        exp,
        |ty| matches!(ty, Ty::Cmd(_)),
        |e| {
            matches!(
                e,
                Exp::Print(_) | Exp::Readline | Exp::CmdPure(_) | Exp::CmdBind(..)
            )
        },
    )
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

fn every_v5_program() -> Vec<(String, Doc, NameTable)> {
    every_v4_program()
        .into_iter()
        .filter(|(_, doc, _)| !doc.defs().iter().any(|def| mentions_a_variant(&def.body)))
        .collect()
}

fn ensure_v5_fixtures() {
    let dir = v5_fixture_dir();
    fs::create_dir_all(&dir).expect("the fixture directory is creatable");
    for (name, doc, names) in every_v5_program() {
        let path = dir.join(format!("{name}.v5.nothing"));
        if !path.exists() {
            let document = Document::from_doc(doc, names, sample_log());
            fs::write(&path, encode_document_v5(&document)).expect("the fixture is writable");
        }
    }
}

fn v5_artifacts() -> Vec<PathBuf> {
    ensure_v5_fixtures();
    let mut paths: Vec<PathBuf> = fs::read_dir(v5_fixture_dir())
        .expect("the fixture directory exists")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "nothing"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn there_are_version_five_artifacts_to_migrate() {
    let paths = v5_artifacts();
    assert!(
        paths.len() >= 17,
        "only {} v5 artifacts were found; the v5 migration path is barely exercised",
        paths.len()
    );
    for path in &paths {
        let bytes = fs::read(path).expect("the artifact is readable");
        assert_eq!(&bytes[0..4], b"NTHG", "{path:?} is not a nothing file");
        assert_eq!(
            bytes[4], 5,
            "{path:?} is not version 5, so it does not test migration"
        );
    }
}

#[test]
fn every_version_five_artifact_opens_under_version_six() {
    for path in v5_artifacts() {
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
fn a_version_five_artifact_still_carries_the_records_that_made_it_version_five() {
    let paths = v5_artifacts();
    let with_records = paths
        .iter()
        .filter(|path| {
            let bytes = fs::read(path).expect("the artifact is readable");
            let document = decode_document(&bytes).expect("the artifact decodes");
            document
                .doc
                .defs()
                .iter()
                .any(|d| mentions_a_record(&d.body))
        })
        .count();
    assert!(
        with_records > 0,
        "no v5 fixture contains a record, so the v5 corpus is indistinguishable from the v4 one"
    );
}

#[test]
fn no_version_five_artifact_contains_a_version_six_form() {
    for path in v5_artifacts() {
        let bytes = fs::read(&path).expect("the artifact is readable");
        let document = decode_document(&bytes).expect("the artifact decodes");
        for def in document.doc.defs() {
            assert!(
                !mentions_a_variant(&def.body),
                "{path:?} contains a variant, so it is not bytes a version-5 build could have \
                 written and it does not test the migration it claims to"
            );
        }
    }
}

#[test]
fn a_version_five_document_keeps_every_definition_it_had() {
    for (name, doc, names) in every_v5_program() {
        let before = Document::from_doc(doc, names, sample_log());
        let bytes = encode_document_v5(&before);
        assert_eq!(bytes[4], 5, "{name} was not written as version 5");
        let after = decode_document(&bytes).expect("the v5 bytes migrate");
        assert_eq!(after.doc, before.doc, "{name} lost a definition");
        assert_eq!(after.log, before.log, "{name} lost its action log");
    }
}

#[test]
fn a_version_six_file_carries_a_variant_no_earlier_version_could() {
    let red = nothing_core::exp::Id::from_u128(11);
    let green = nothing_core::exp::Id::from_u128(12);
    let x = nothing_core::exp::Id::from_u128(13);
    let y = nothing_core::exp::Id::from_u128(14);
    let program = Exp::match_(
        Exp::inj(red, Exp::num(1)),
        [(red, x, Exp::var(x)), (green, y, Exp::num(0))],
    );
    let document = Document::new(program.clone(), NameTable::new(), sample_log());
    let bytes = encode_document_v6(&document);
    assert_eq!(bytes[4], 6);
    let reopened = decode_document(&bytes).expect("a variant document opens");
    assert_eq!(reopened.exp(), program);
    assert_eq!(
        nothing_core::typing::syn(&nothing_core::ctx::Ctx::empty(), &program),
        Some(Ty::Num),
        "the fixture must be a well-typed match over a one-constructor variant"
    );
    assert!(
        encode_document_v5(&document) != bytes,
        "the v5 and v6 encoders must at least disagree about the version byte"
    );
}

fn every_v6_program() -> Vec<(String, Doc, NameTable)> {
    every_v4_program()
        .into_iter()
        .filter(|(_, doc, _)| !doc.defs().iter().any(|def| mentions_a_command(&def.body)))
        .collect()
}

fn ensure_v6_fixtures() {
    let dir = v6_fixture_dir();
    fs::create_dir_all(&dir).expect("the fixture directory is creatable");
    for (name, doc, names) in every_v6_program() {
        let path = dir.join(format!("{name}.v6.nothing"));
        if !path.exists() {
            let document = Document::from_doc(doc, names, sample_log());
            fs::write(&path, encode_document_v6(&document)).expect("the fixture is writable");
        }
    }
}

fn v6_artifacts() -> Vec<PathBuf> {
    ensure_v6_fixtures();
    let mut paths: Vec<PathBuf> = fs::read_dir(v6_fixture_dir())
        .expect("the fixture directory exists")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "nothing"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn there_are_version_six_artifacts_to_migrate() {
    let paths = v6_artifacts();
    assert!(
        paths.len() >= 17,
        "only {} v6 artifacts were found; the v6 migration path is barely exercised",
        paths.len()
    );
    for path in &paths {
        let bytes = fs::read(path).expect("the artifact is readable");
        assert_eq!(&bytes[0..4], b"NTHG", "{path:?} is not a nothing file");
        assert_eq!(
            bytes[4], 6,
            "{path:?} is not version 6, so it does not test migration"
        );
    }
}

#[test]
fn every_version_six_artifact_opens_under_version_seven() {
    for path in v6_artifacts() {
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
fn a_version_six_artifact_still_carries_the_variants_that_made_it_version_six() {
    let paths = v6_artifacts();
    let with_variants = paths
        .iter()
        .filter(|path| {
            let bytes = fs::read(path).expect("the artifact is readable");
            let document = decode_document(&bytes).expect("the artifact decodes");
            document
                .doc
                .defs()
                .iter()
                .any(|d| mentions_a_variant(&d.body))
        })
        .count();
    assert!(
        with_variants > 0,
        "no v6 fixture contains a variant, so the v6 corpus is indistinguishable from the v5 one"
    );
}

#[test]
fn no_version_six_artifact_contains_a_version_seven_form() {
    for path in v6_artifacts() {
        let bytes = fs::read(&path).expect("the artifact is readable");
        let document = decode_document(&bytes).expect("the artifact decodes");
        for def in document.doc.defs() {
            assert!(
                !mentions_a_command(&def.body),
                "{path:?} contains a command, so it is not bytes a version-6 build could have \
                 written and it does not test the migration it claims to"
            );
        }
    }
}

#[test]
fn a_version_six_document_keeps_every_definition_it_had() {
    for (name, doc, names) in every_v6_program() {
        let before = Document::from_doc(doc, names, sample_log());
        let bytes = encode_document_v6(&before);
        assert_eq!(bytes[4], 6, "{name} was not written as version 6");
        let after = decode_document(&bytes).expect("the v6 bytes migrate");
        assert_eq!(after.doc, before.doc, "{name} lost a definition");
        assert_eq!(after.log, before.log, "{name} lost its action log");
    }
}

fn every_v7_program() -> Vec<(String, Doc, NameTable)> {
    let mut out = every_v4_program();
    let replayed =
        replay_script(GREETING_COMMAND).expect("the greeting command fixture replays cleanly");
    out.push((
        "bench_greeting_command".to_string(),
        replayed.doc(),
        replayed.names.clone(),
    ));
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn ensure_v7_fixtures() {
    let dir = v7_fixture_dir();
    fs::create_dir_all(&dir).expect("the fixture directory is creatable");
    for (name, doc, names) in every_v7_program() {
        let path = dir.join(format!("{name}.v7.nothing"));
        if !path.exists() {
            let document = Document::from_doc(doc, names, sample_log());
            fs::write(&path, encode_document_v7(&document)).expect("the fixture is writable");
        }
    }
}

fn v7_artifacts() -> Vec<PathBuf> {
    ensure_v7_fixtures();
    let mut paths: Vec<PathBuf> = fs::read_dir(v7_fixture_dir())
        .expect("the fixture directory exists")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "nothing"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn there_are_version_seven_artifacts_to_migrate() {
    let paths = v7_artifacts();
    assert!(
        paths.len() >= 18,
        "only {} v7 artifacts were found; the v7 migration path is barely exercised",
        paths.len()
    );
    for path in &paths {
        let bytes = fs::read(path).expect("the artifact is readable");
        assert_eq!(&bytes[0..4], b"NTHG", "{path:?} is not a nothing file");
        assert_eq!(
            bytes[4], 7,
            "{path:?} is not version 7, so it does not test migration"
        );
    }
}

#[test]
fn a_version_seven_artifact_still_carries_the_commands_that_made_it_version_seven() {
    let paths = v7_artifacts();
    let with_commands = paths
        .iter()
        .filter(|path| {
            let bytes = fs::read(path).expect("the artifact is readable");
            let document = decode_document(&bytes).expect("the artifact decodes");
            document
                .doc
                .defs()
                .iter()
                .any(|d| mentions_a_command(&d.body))
        })
        .count();
    assert!(
        with_commands > 0,
        "no v7 fixture contains a command, so the v7 corpus is indistinguishable from the v6 one"
    );
}

#[test]
fn a_version_seven_file_carries_a_command_no_earlier_version_could() {
    let line = nothing_core::exp::Id::from_u128(21);
    let program = Exp::cmd_bind(
        Exp::readline(),
        line,
        Exp::print(Exp::bin_op(
            nothing_core::exp::Op::Concat,
            Exp::str_("hello, "),
            Exp::var(line),
        )),
    );
    let document = Document::new(program.clone(), NameTable::new(), sample_log());
    let bytes = encode_document_v7(&document);
    assert_eq!(bytes[4], 7);
    let reopened = decode_document(&bytes).expect("a command document opens");
    assert_eq!(reopened.exp(), program);
    assert_eq!(
        nothing_core::typing::syn(&nothing_core::ctx::Ctx::empty(), &program),
        Some(Ty::Cmd(Box::new(nothing_core::ty::unit()))),
        "the fixture must be a well-typed greeting command"
    );
    assert!(
        encode_document_v6(&document) != bytes,
        "the v6 and v7 encoders must at least disagree about the version byte"
    );
}

#[test]
fn no_version_seven_artifact_carries_a_doc_line() {
    for path in v7_artifacts() {
        let bytes = fs::read(&path).expect("the artifact is readable");
        let document = decode_document(&bytes).expect("the artifact decodes");
        assert!(
            document.docs.is_empty(),
            "{path:?} carries a doc line, so it is not bytes a version-7 build could have \
             written and it does not test the migration it claims to"
        );
    }
}

#[test]
fn a_version_seven_document_keeps_every_definition_it_had() {
    for (name, doc, names) in every_v7_program() {
        let before = Document::from_doc(doc, names, sample_log());
        let bytes = encode_document_v7(&before);
        assert_eq!(bytes[4], 7, "{name} was not written as version 7");
        let after = decode_document(&bytes).expect("the v7 bytes migrate");
        assert_eq!(after.doc, before.doc, "{name} lost a definition");
        assert_eq!(after.log, before.log, "{name} lost its action log");
        assert!(after.docs.is_empty(), "{name} invented a doc line");
    }
}

#[test]
fn a_version_eight_file_carries_a_doc_line_no_earlier_version_could() {
    let helper = nothing_core::exp::Id::from_u128(31);
    let doc = Doc::new(vec![nothing_core::doc::Def::new(
        helper,
        Ty::Num,
        Exp::num(7),
    )])
    .expect("one definition");
    let mut names = NameTable::new();
    names.set(helper, "seven");
    let mut docs = nothing_core::docs::DocTable::new();
    docs.set(helper, "the number seven, for want of a better example");

    let document = Document::documented(doc, names, docs.clone(), sample_log());
    let bytes = encode_document(&document);
    assert_eq!(bytes[4], VERSION_MAJOR);
    assert_eq!(VERSION_MAJOR, 8);

    let reopened = decode_document(&bytes).expect("a documented document opens");
    assert_eq!(
        reopened.docs.get(helper),
        Some("the number seven, for want of a better example")
    );
    assert_eq!(reopened.docs, docs);

    let seven = encode_document_v7(&document);
    assert!(
        seven != bytes,
        "the v7 and v8 encoders must at least disagree about the version byte"
    );
    assert!(
        decode_document(&seven)
            .expect("the v7 bytes still open")
            .docs
            .is_empty(),
        "a v7 file cannot carry the doc line, which is what makes v8 a new version"
    );
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
