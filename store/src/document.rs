use nothing_action::log::ActionLog;
use nothing_core::exp::Exp;
use nothing_core::names::NameTable;

use crate::actionlog::{decode_log, encode_log};
use crate::codec::{read_u8, read_varint, write_varint};
use crate::error::DecodeError;
use crate::names::{decode_names, encode_names};
use crate::nodes::{build_node_table, content_hash, decode_node_table, NodeEntry};

pub const MAGIC: [u8; 4] = *b"NTHG";
pub const VERSION_MAJOR: u8 = 1;
pub const VERSION_MINOR: u8 = 0;
pub const KIND_DOCUMENT: u8 = 1;

#[derive(Clone, PartialEq, Debug)]
pub struct Document {
    pub exp: Exp,
    pub names: NameTable,
    pub log: ActionLog,
}

impl Document {
    pub fn new(exp: Exp, names: NameTable, log: ActionLog) -> Document {
        Document { exp, names, log }
    }
}

fn encode_node_table(buf: &mut Vec<u8>, table: &[NodeEntry]) {
    write_varint(buf, table.len() as u64);
    for entry in table {
        buf.extend_from_slice(&entry.hash);
        buf.push(entry.tag);
        write_varint(buf, entry.payload.len() as u64);
        buf.extend_from_slice(&entry.payload);
        write_varint(buf, entry.children.len() as u64);
        for child in &entry.children {
            write_varint(buf, *child as u64);
        }
    }
}

fn decode_node_table_bytes(
    bytes: &[u8],
    pos: &mut usize,
) -> Result<Vec<NodeEntry>, DecodeError> {
    let count = read_varint(bytes, pos)?;
    let mut table = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let hash_slice = crate::codec::read_bytes(bytes, pos, 32)?;
        let mut hash = [0u8; 32];
        hash.copy_from_slice(hash_slice);
        let tag = read_u8(bytes, pos)?;
        let payload_len = read_varint(bytes, pos)? as usize;
        let payload = crate::codec::read_bytes(bytes, pos, payload_len)?.to_vec();
        let children_count = read_varint(bytes, pos)?;
        let mut children = Vec::with_capacity(children_count as usize);
        for _ in 0..children_count {
            children.push(read_varint(bytes, pos)? as u32);
        }
        table.push(NodeEntry {
            hash,
            tag,
            payload,
            children,
        });
    }
    Ok(table)
}

pub fn encode_document(doc: &Document) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&MAGIC);
    buf.push(VERSION_MAJOR);
    buf.push(VERSION_MINOR);
    buf.push(KIND_DOCUMENT);

    let table = build_node_table(&doc.exp);
    encode_node_table(&mut buf, &table);
    write_varint(&mut buf, (table.len() - 1) as u64);

    encode_names(&mut buf, &doc.names);
    encode_log(&mut buf, &doc.log);

    buf
}

pub fn decode_document(bytes: &[u8]) -> Result<Document, DecodeError> {
    let mut pos = 0usize;

    let magic = crate::codec::read_bytes(bytes, &mut pos, 4)?;
    if magic != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let major = read_u8(bytes, &mut pos)?;
    let minor = read_u8(bytes, &mut pos)?;
    if major != VERSION_MAJOR {
        return Err(DecodeError::UnsupportedVersion(major, minor));
    }
    let kind = read_u8(bytes, &mut pos)?;
    if kind != KIND_DOCUMENT {
        return Err(DecodeError::UnsupportedKind(kind));
    }

    let table = decode_node_table_bytes(bytes, &mut pos)?;
    let root_index = read_varint(bytes, &mut pos)? as usize;
    if table.is_empty() || root_index != table.len() - 1 {
        return Err(DecodeError::BadRootIndex);
    }

    let exp = decode_node_table(&table)?;
    if content_hash(&exp) != table[root_index].hash {
        return Err(DecodeError::HashMismatch);
    }

    let names = decode_names(bytes, &mut pos)?;
    let log = decode_log(bytes, &mut pos)?;

    if pos != bytes.len() {
        return Err(DecodeError::TrailingBytes);
    }

    Ok(Document::new(exp, names, log))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_action::act::Action;
    use nothing_action::log::AuthorId;
    use nothing_core::examples;

    #[test]
    fn a_document_round_trips() {
        let mut names = examples::names();
        names.set(examples::binder(0), "acc");
        let mut log = ActionLog::new();
        log.append(Action::ConstructNum(1), 10, AuthorId::new(1));
        log.append(Action::Finish, 20, AuthorId::new(1));

        let doc = Document::new(examples::square_and_compare(), names, log);
        let bytes = encode_document(&doc);
        let decoded = decode_document(&bytes).unwrap();

        assert_eq!(decoded.exp, doc.exp);
        assert_eq!(decoded.log, doc.log);
        let mut decoded_entries = decoded.names.entries();
        let mut expected_entries = doc.names.flatten().entries();
        decoded_entries.sort_by_key(|(id, _)| id.as_u128());
        expected_entries.sort_by_key(|(id, _)| id.as_u128());
        assert_eq!(decoded_entries, expected_entries);
    }

    #[test]
    fn corrupted_bytes_are_rejected() {
        let doc = Document::new(examples::let_identity(), NameTable::new(), ActionLog::new());
        let mut bytes = encode_document(&doc);
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        assert!(decode_document(&bytes).is_err());
    }

    #[test]
    fn bad_magic_is_rejected() {
        let doc = Document::new(examples::let_identity(), NameTable::new(), ActionLog::new());
        let mut bytes = encode_document(&doc);
        bytes[0] = 0;
        assert_eq!(decode_document(&bytes), Err(DecodeError::BadMagic));
    }
}
