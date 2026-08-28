use std::path::Path;

use nothing_core::typing::is_well_typed;

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

    let well_typed = is_well_typed(&doc.exp);
    let counts = count_holes(&doc.exp);
    println!("well-typed: {well_typed}");
    println!("empty holes: {}", counts.empty);
    println!("non-empty holes: {}", counts.non_empty);

    if well_typed { 0 } else { 1 }
}
