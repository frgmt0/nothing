use nothing_action::log::ActionLog;
use nothing_core::doc::{Doc, MAIN_ID, MAIN_NAME};
use nothing_core::exp::Exp;
use nothing_core::names::NameTable;

use crate::actionlog::{decode_log, encode_log};
use crate::document::{
    Document, KIND_DOCUMENT, MAGIC, VERSION_MAJOR_V1, decode_body, encode_body, read_header,
};
use crate::error::DecodeError;
use crate::names::{decode_names, encode_names};

pub const VERSION_MINOR_V1: u8 = 0;

pub fn encode_document_v1(exp: &Exp, names: &NameTable, log: &ActionLog) -> Vec<u8> {
    nothing_core::stack::on_deep_stack(|| encode_document_v1_walk(exp, names, log))
}

fn encode_document_v1_walk(exp: &Exp, names: &NameTable, log: &ActionLog) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&MAGIC);
    buf.push(VERSION_MAJOR_V1);
    buf.push(VERSION_MINOR_V1);
    buf.push(KIND_DOCUMENT);

    encode_body(&mut buf, exp);
    encode_names(&mut buf, names);
    encode_log(&mut buf, log);

    buf
}

pub fn decode_document_v1(bytes: &[u8]) -> Result<Document, DecodeError> {
    nothing_core::stack::on_deep_stack(|| decode_document_v1_walk(bytes))
}

fn decode_document_v1_walk(bytes: &[u8]) -> Result<Document, DecodeError> {
    let mut pos = 0usize;
    let (major, minor) = read_header(bytes, &mut pos)?;
    if major != VERSION_MAJOR_V1 {
        return Err(DecodeError::UnsupportedVersion(major, minor));
    }

    let exp = decode_body(bytes, &mut pos)?;
    let mut names = decode_names(bytes, &mut pos)?;
    let log = decode_log(bytes, &mut pos)?;

    if pos != bytes.len() {
        return Err(DecodeError::TrailingBytes);
    }

    if names.get(MAIN_ID).is_none() {
        names.set(MAIN_ID, MAIN_NAME);
    }

    Ok(Document::from_doc(Doc::single(exp), names, log))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{decode_document, encode_document};
    use nothing_action::act::Action;
    use nothing_action::log::AuthorId;
    use nothing_core::examples;
    use nothing_core::ty::Ty;

    fn v1_bytes(exp: &Exp) -> Vec<u8> {
        let mut names = examples::names();
        names.set(examples::binder(0), "acc");
        let mut log = ActionLog::new();
        log.append(Action::ConstructNum(3), 11, AuthorId::new(2));
        log.append(Action::Finish, 12, AuthorId::new(2));
        encode_document_v1(exp, &names, &log)
    }

    #[test]
    fn a_version_one_file_declares_version_one_in_its_header() {
        let bytes = v1_bytes(&examples::let_identity());
        assert_eq!(&bytes[0..4], b"NTHG");
        assert_eq!(bytes[4], 1);
        assert_eq!(bytes[5], 0);
        assert_eq!(bytes[6], KIND_DOCUMENT);
    }

    #[test]
    fn the_current_reader_migrates_a_version_one_file() {
        let exp = examples::square_and_compare();
        let migrated = decode_document(&v1_bytes(&exp)).expect("v1 files still open");

        assert_eq!(migrated.doc.len(), 1);
        let def = &migrated.doc.defs()[0];
        assert_eq!(def.id, MAIN_ID);
        assert_eq!(def.ann, Ty::Hole);
        assert_eq!(def.body, exp);
        assert_eq!(migrated.names.get(MAIN_ID), Some(MAIN_NAME));
        assert_eq!(migrated.main_id(), Some(MAIN_ID));
    }

    #[test]
    fn migration_is_idempotent_and_re_encodes_as_the_current_version() {
        let exp = examples::pair_and_project();
        let once = decode_document(&v1_bytes(&exp)).expect("v1 opens");
        let twice = decode_document(&v1_bytes(&exp)).expect("v1 opens");
        assert_eq!(once, twice);

        let current = encode_document(&once);
        assert_eq!(current[4], crate::document::VERSION_MAJOR);
        let reopened = decode_document(&current).expect("the current version opens");
        assert_eq!(reopened.doc, once.doc);
        assert_eq!(reopened.log, once.log);
        assert_eq!(encode_document(&reopened), current);
    }

    #[test]
    fn a_version_one_action_log_survives_migration() {
        let bytes = v1_bytes(&examples::let_identity());
        let migrated = decode_document(&bytes).expect("v1 opens");
        assert_eq!(migrated.log.entries().len(), 2);
        assert_eq!(migrated.log.entries()[0].action, Action::ConstructNum(3));
        assert_eq!(migrated.log.entries()[1].action, Action::Finish);
    }

    #[test]
    fn an_unknown_major_version_is_still_refused() {
        let mut bytes = v1_bytes(&examples::let_identity());
        bytes[4] = 99;
        assert_eq!(
            decode_document(&bytes),
            Err(DecodeError::UnsupportedVersion(99, 0))
        );
    }

    #[test]
    fn a_version_one_file_that_already_names_main_keeps_that_name() {
        let mut names = NameTable::new();
        names.set(MAIN_ID, "entry");
        let bytes = encode_document_v1(&examples::let_identity(), &names, &ActionLog::new());
        let migrated = decode_document(&bytes).expect("v1 opens");
        assert_eq!(migrated.names.get(MAIN_ID), Some("entry"));
        assert_eq!(migrated.main_id(), None);
    }
}
