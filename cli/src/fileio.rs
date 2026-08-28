use std::path::Path;

use nothing_store::{Document, decode_document, encode_document};

pub fn read_document(path: &Path) -> Result<Document, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    decode_document(&bytes).map_err(|e| format!("cannot decode {}: {e}", path.display()))
}

pub fn write_document(path: &Path, doc: &Document) -> Result<(), String> {
    let bytes = encode_document(doc);
    std::fs::write(path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))
}
