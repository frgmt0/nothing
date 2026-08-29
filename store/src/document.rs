use nothing_action::log::ActionLog;
use nothing_core::doc::{Def, Doc, MAIN_ID, MAIN_NAME};
use nothing_core::exp::Exp;
use nothing_core::names::NameTable;

use crate::actionlog::{decode_log, encode_log};
use crate::codec::{decode_ty, encode_ty, read_id, read_u8, read_varint, write_id, write_varint};
use crate::error::DecodeError;
use crate::names::{decode_names, encode_names};
use crate::nodes::{NodeEntry, build_node_table, content_hash, decode_node_table};

pub const MAGIC: [u8; 4] = *b"NTHG";
pub const VERSION_MAJOR: u8 = 4;
pub const VERSION_MINOR: u8 = 0;
pub const VERSION_MAJOR_V1: u8 = 1;
pub const VERSION_MAJOR_V2: u8 = 2;
pub const VERSION_MAJOR_V3: u8 = 3;
pub const KIND_DOCUMENT: u8 = 1;

#[derive(Clone, PartialEq, Debug)]
pub struct Document {
    pub doc: Doc,
    pub names: NameTable,
    pub log: ActionLog,
}

impl Document {
    pub fn new(exp: Exp, names: NameTable, log: ActionLog) -> Document {
        Document::from_doc(Doc::single(exp), name_main(names, MAIN_ID), log)
    }

    pub fn from_doc(doc: Doc, names: NameTable, log: ActionLog) -> Document {
        Document { doc, names, log }
    }

    pub fn exp(&self) -> Exp {
        self.doc.defs()[0].body.clone()
    }

    pub fn main_id(&self) -> Option<nothing_core::exp::Id> {
        self.doc.main_id(&self.names)
    }
}

fn name_main(names: NameTable, id: nothing_core::exp::Id) -> NameTable {
    let mut names = names;
    if names.get(id).is_none() {
        names.set(id, MAIN_NAME);
    }
    names
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

fn decode_node_table_bytes(bytes: &[u8], pos: &mut usize) -> Result<Vec<NodeEntry>, DecodeError> {
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

pub(crate) fn encode_body(buf: &mut Vec<u8>, exp: &Exp) {
    let table = build_node_table(exp);
    encode_node_table(buf, &table);
    write_varint(buf, (table.len() - 1) as u64);
}

pub(crate) fn decode_body(bytes: &[u8], pos: &mut usize) -> Result<Exp, DecodeError> {
    let table = decode_node_table_bytes(bytes, pos)?;
    let root_index = read_varint(bytes, pos)? as usize;
    if table.is_empty() || root_index != table.len() - 1 {
        return Err(DecodeError::BadRootIndex);
    }
    let exp = decode_node_table(&table)?;
    if content_hash(&exp) != table[root_index].hash {
        return Err(DecodeError::HashMismatch);
    }
    Ok(exp)
}

pub(crate) fn read_header(bytes: &[u8], pos: &mut usize) -> Result<(u8, u8), DecodeError> {
    let magic = crate::codec::read_bytes(bytes, pos, 4)?;
    if magic != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let major = read_u8(bytes, pos)?;
    let minor = read_u8(bytes, pos)?;
    let kind = read_u8(bytes, pos)?;
    if kind != KIND_DOCUMENT {
        return Err(DecodeError::UnsupportedKind(kind));
    }
    Ok((major, minor))
}

pub(crate) fn encode_defs(buf: &mut Vec<u8>, doc: &Document) {
    write_varint(buf, doc.doc.len() as u64);
    for def in doc.doc.defs() {
        let mut body = Vec::new();
        write_id(&mut body, def.id);
        encode_ty(&mut body, &def.ann);
        encode_body(&mut body, &def.body);
        write_varint(buf, body.len() as u64);
        buf.extend_from_slice(&body);
    }

    encode_names(buf, &doc.names);
    encode_log(buf, &doc.log);
}

pub fn encode_document(doc: &Document) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&MAGIC);
    buf.push(VERSION_MAJOR);
    buf.push(VERSION_MINOR);
    buf.push(KIND_DOCUMENT);
    encode_defs(&mut buf, doc);
    buf
}

pub fn decode_document(bytes: &[u8]) -> Result<Document, DecodeError> {
    let mut pos = 0usize;
    let (major, minor) = read_header(bytes, &mut pos)?;
    match major {
        VERSION_MAJOR_V1 => return crate::v1::decode_document_v1(bytes),
        VERSION_MAJOR_V2 | VERSION_MAJOR_V3 | VERSION_MAJOR => {}
        _ => return Err(DecodeError::UnsupportedVersion(major, minor)),
    }

    decode_defs(bytes, &mut pos)
}

pub(crate) fn decode_defs(bytes: &[u8], pos: &mut usize) -> Result<Document, DecodeError> {
    let def_count = read_varint(bytes, pos)?;
    if def_count == 0 {
        return Err(DecodeError::EmptyDocument);
    }
    let mut defs = Vec::with_capacity(def_count as usize);
    for _ in 0..def_count {
        let def_len = read_varint(bytes, pos)? as usize;
        let start = *pos;
        let id = read_id(bytes, pos)?;
        let ann = decode_ty(bytes, pos)?;
        let body = decode_body(bytes, pos)?;
        if *pos - start != def_len {
            return Err(DecodeError::TrailingBytes);
        }
        defs.push(Def::new(id, ann, body));
    }
    let doc = Doc::new(defs).ok_or(DecodeError::DuplicateDefinition)?;

    let names = decode_names(bytes, pos)?;
    let log = decode_log(bytes, pos)?;

    if *pos != bytes.len() {
        return Err(DecodeError::TrailingBytes);
    }

    Ok(Document::from_doc(doc, names, log))
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

        assert_eq!(decoded.doc, doc.doc);
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
