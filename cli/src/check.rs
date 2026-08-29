use std::path::Path;

use nothing_core::prelude::Prelude;
use nothing_store::Document;

use crate::fileio::read_document;
use crate::holes::count_holes;

pub const HELP: &str = "\
nothing check <file>

Check that <file> is well-typed and report its hole counts. The standard
library is in scope, so references to its definitions are not dangling.

Exit status: 0 if well-typed, 1 otherwise (including file errors).

Options:
  -h, --help   print this help and exit";

pub struct DefinitionHoles {
    pub name: String,
    pub ann: String,
    pub empty: usize,
    pub non_empty: usize,
}

pub struct CheckReport {
    pub well_typed: bool,
    pub stdlib_definitions: usize,
    pub definitions: Vec<DefinitionHoles>,
}

impl CheckReport {
    pub fn empty_holes(&self) -> usize {
        self.definitions.iter().map(|def| def.empty).sum()
    }

    pub fn non_empty_holes(&self) -> usize {
        self.definitions.iter().map(|def| def.non_empty).sum()
    }

    pub fn complete(&self) -> bool {
        self.empty_holes() == 0 && self.non_empty_holes() == 0
    }
}

pub fn check_document(doc: &Document, prelude: &Prelude) -> CheckReport {
    CheckReport {
        well_typed: doc.doc.is_well_typed_in(prelude.ctx()),
        stdlib_definitions: prelude.len(),
        definitions: doc
            .doc
            .defs()
            .iter()
            .map(|def| {
                let counts = count_holes(&def.body);
                DefinitionHoles {
                    name: doc.names.display(def.id).to_string(),
                    ann: def.ann.to_string(),
                    empty: counts.empty,
                    non_empty: counts.non_empty,
                }
            })
            .collect(),
    }
}

pub fn run(path: &Path) -> i32 {
    let doc = match read_document(path) {
        Ok(doc) => doc,
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    };

    let report = check_document(&doc, &nothing_stdlib::prelude());
    println!("well-typed: {}", report.well_typed);
    println!("definitions: {}", report.definitions.len());
    println!("stdlib definitions in scope: {}", report.stdlib_definitions);
    println!("empty holes: {}", report.empty_holes());
    println!("non-empty holes: {}", report.non_empty_holes());

    if report.well_typed { 0 } else { 1 }
}
