use std::path::Path;

use crate::fileio::read_document;
use crate::holes::count_holes;

pub const HELP: &str = "\
nothing check <file>

Check that <file> is well-typed and report its hole counts.

Exit status: 0 if well-typed, 1 otherwise (including file errors).

Options:
  -h, --help   print this help and exit";

pub fn run(path: &Path) -> i32 {
    let doc = match read_document(path) {
        Ok(doc) => doc,
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    };

    let well_typed = doc.doc.is_well_typed();
    let mut empty = 0usize;
    let mut non_empty = 0usize;
    for def in doc.doc.defs() {
        let counts = count_holes(&def.body);
        empty += counts.empty;
        non_empty += counts.non_empty;
    }
    println!("well-typed: {well_typed}");
    println!("definitions: {}", doc.doc.len());
    println!("empty holes: {empty}");
    println!("non-empty holes: {non_empty}");

    if well_typed { 0 } else { 1 }
}
