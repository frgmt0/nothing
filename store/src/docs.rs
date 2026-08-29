use nothing_core::docs::DocTable;

use crate::codec::{read_id, read_string, read_varint, write_id, write_string, write_varint};
use crate::error::DecodeError;

pub fn encode_docs(buf: &mut Vec<u8>, docs: &DocTable) {
    let mut entries = docs.own().entries();
    entries.sort_by_key(|(id, _)| id.as_u128());
    write_varint(buf, entries.len() as u64);
    for (id, line) in &entries {
        write_id(buf, *id);
        write_string(buf, line);
    }
}

pub fn decode_docs(bytes: &[u8], pos: &mut usize) -> Result<DocTable, DecodeError> {
    let count = read_varint(bytes, pos)?;
    let mut docs = DocTable::new();
    for _ in 0..count {
        let id = read_id(bytes, pos)?;
        let line = read_string(bytes, pos)?;
        docs.set(id, line);
    }
    Ok(docs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::exp::Id;

    #[test]
    fn a_doc_table_round_trips() {
        let mut docs = DocTable::new();
        docs.set(Id::from_u128(2), "the larger of two numbers");
        docs.set(Id::from_u128(1), "the smaller of two numbers");

        let mut buf = Vec::new();
        encode_docs(&mut buf, &docs);
        let mut pos = 0;
        let decoded = decode_docs(&buf, &mut pos).unwrap();

        assert_eq!(pos, buf.len());
        assert_eq!(
            decoded.get(Id::from_u128(1)),
            Some("the smaller of two numbers")
        );
        assert_eq!(
            decoded.get(Id::from_u128(2)),
            Some("the larger of two numbers")
        );
    }

    #[test]
    fn an_empty_doc_table_is_one_byte() {
        let mut buf = Vec::new();
        encode_docs(&mut buf, &DocTable::new());
        assert_eq!(buf, vec![0u8]);
    }

    #[test]
    fn only_the_documents_own_doc_lines_are_written() {
        let mut base = DocTable::new();
        base.set(Id::from_u128(1), "from the stdlib");
        let mut mine = DocTable::overlay(&base);
        mine.set(Id::from_u128(2), "mine");

        let mut buf = Vec::new();
        encode_docs(&mut buf, &mine);
        let mut pos = 0;
        let decoded = decode_docs(&buf, &mut pos).unwrap();

        assert_eq!(decoded.get(Id::from_u128(1)), None);
        assert_eq!(decoded.get(Id::from_u128(2)), Some("mine"));
    }
}
