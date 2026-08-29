use nothing_action::generate::well_typed_exp;
use nothing_action::log::ActionLog;
use nothing_core::names::NameTable;
use nothing_store::content_hash;
use nothing_store::document::{Document, decode_document, encode_document};

#[test]
fn ten_thousand_random_well_typed_programs_round_trip_through_serialisation() {
    for seed in 0..10_000u64 {
        let exp = well_typed_exp(seed);
        let doc = Document::new(exp.clone(), NameTable::new(), ActionLog::new());

        let bytes = encode_document(&doc);
        let decoded = decode_document(&bytes).unwrap_or_else(|e| {
            panic!("seed {seed} failed to decode: {e:?}");
        });

        assert_eq!(
            decoded.exp(),
            exp,
            "seed {seed} did not round-trip structurally"
        );
        assert_eq!(
            content_hash(&decoded.exp()),
            content_hash(&exp),
            "seed {seed} changed content hash across the round trip"
        );

        let re_encoded = encode_document(&decoded);
        assert_eq!(bytes, re_encoded, "seed {seed} was not byte-stable");
    }
}
