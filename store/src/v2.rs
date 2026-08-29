use crate::document::{
    Document, KIND_DOCUMENT, MAGIC, VERSION_MAJOR_V2, decode_defs, encode_defs, read_header,
};
use crate::error::DecodeError;

pub const VERSION_MINOR_V2: u8 = 0;

pub fn encode_document_v2(doc: &Document) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&MAGIC);
    buf.push(VERSION_MAJOR_V2);
    buf.push(VERSION_MINOR_V2);
    buf.push(KIND_DOCUMENT);
    encode_defs(&mut buf, doc);
    buf
}

pub fn decode_document_v2(bytes: &[u8]) -> Result<Document, DecodeError> {
    let mut pos = 0usize;
    let (major, minor) = read_header(bytes, &mut pos)?;
    if major != VERSION_MAJOR_V2 {
        return Err(DecodeError::UnsupportedVersion(major, minor));
    }
    decode_defs(bytes, &mut pos)
}
