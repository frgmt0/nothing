use nothing_action::log::ActionLog;
use nothing_core::exp::Exp;
use nothing_core::names::NameTable;
use nothing_core::stack::on_deep_stack;
use nothing_store::document::{Document, decode_document, encode_document};
use nothing_store::nodes::{build_node_table, content_hash, decode_node_table};

const CI_STACK_BYTES: usize = 2 * 1024 * 1024;

fn on_a_ci_sized_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(CI_STACK_BYTES)
        .spawn(work)
        .expect("spawn the small-stack thread a CI runner would give a test")
        .join()
        .expect("the small-stack thread finished without overflowing")
}

fn long_list(n: i64) -> Exp {
    Exp::list((0..n).map(Exp::num))
}

#[test]
fn a_long_list_round_trips_through_the_node_table() {
    on_a_ci_sized_stack(|| {
        let (entries, same, hashes_match) = on_deep_stack(|| {
            let list = long_list(50_000);
            let table = build_node_table(&list);
            let hash = content_hash(&list);
            let decoded = decode_node_table(&table).expect("the table decodes");
            let same = decoded == list;
            (table.len(), same, content_hash(&decoded) == hash)
        });
        assert_eq!(entries, 100_001, "one node per cell and one per literal");
        assert!(same, "the decoded program is the one that was encoded");
        assert!(hashes_match, "and it hashes to the same content hash");
    });
}

#[test]
fn a_long_list_round_trips_through_a_saved_document() {
    on_a_ci_sized_stack(|| {
        let (bytes, same) = on_deep_stack(|| {
            let doc = Document::new(long_list(50_000), NameTable::new(), ActionLog::new());
            let bytes = encode_document(&doc);
            let decoded = decode_document(&bytes).expect("the document decodes");
            let same = decoded.exp() == doc.exp();
            (bytes.len(), same)
        });
        assert!(
            bytes > 100_000,
            "a fifty thousand cell program is not small"
        );
        assert!(same, "what was saved is what comes back");
    });
}

#[test]
fn a_document_deep_enough_to_have_overflowed_saves_and_loads_on_a_ci_sized_stack() {
    on_a_ci_sized_stack(|| {
        let doc = Document::new(long_list(2_000), NameTable::new(), ActionLog::new());
        let bytes = encode_document(&doc);
        let decoded = decode_document(&bytes).expect("the document decodes");
        assert_eq!(
            content_hash(&decoded.doc.defs()[0].body),
            content_hash(&doc.doc.defs()[0].body)
        );
    });
}
