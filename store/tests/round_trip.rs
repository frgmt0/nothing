use nothing_action::act::Action;
use nothing_action::log::{ActionLog, AuthorId};
use nothing_core::examples;
use nothing_core::exp::Exp;
use nothing_core::names::NameTable;
use nothing_store::document::{decode_document, encode_document, Document};

fn sample_log() -> ActionLog {
    let mut log = ActionLog::new();
    log.append(Action::ConstructNum(1), 1_000, AuthorId::new(1));
    log.append(Action::ConstructBool(true), 2_000, AuthorId::new(2));
    log.append(Action::Finish, 3_000, AuthorId::new(1));
    log
}

fn all_examples() -> Vec<(&'static str, Exp)> {
    vec![
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
}

#[test]
fn every_example_program_round_trips_byte_identically() {
    assert_eq!(all_examples().len(), 10);

    for (name, exp) in all_examples() {
        let names = examples::names();
        let log = sample_log();
        let doc = Document::new(exp, names, log);

        let first = encode_document(&doc);
        let decoded = decode_document(&first).expect(name);
        let second = encode_document(&decoded);

        assert_eq!(first, second, "{name} did not round-trip byte-identically");
        assert_eq!(decoded.exp, doc.exp, "{name} lost structure across the round trip");
    }
}

#[test]
fn a_program_with_an_empty_name_table_and_log_round_trips() {
    let doc = Document::new(examples::let_identity(), NameTable::new(), ActionLog::new());
    let first = encode_document(&doc);
    let decoded = decode_document(&first).unwrap();
    let second = encode_document(&decoded);
    assert_eq!(first, second);
}
