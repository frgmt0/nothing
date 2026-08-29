use std::path::Path;

use nothing_action::log::ActionLog;
use nothing_merge::{DocVersion, merge_documents};
use nothing_store::Document;

use crate::fileio::{read_document, write_document};

pub const HELP: &str = "\
nothing merge <base> <a> <b> [-o <out>]

Three-way structural merge of serialised files. On a clean merge, the
result is written to <out> if given, otherwise rendered to stdout.
On conflicts, each conflict is reported and nothing is written.

Exit status: 0 on a clean merge, 1 on conflicts or a file error.

Options:
  -o <out>     write the merged document to <out> instead of stdout
  -h, --help   print this help and exit";

pub fn run(base: &Path, a: &Path, b: &Path, out: Option<&Path>) -> i32 {
    let base_doc = match read_document(base) {
        Ok(doc) => doc,
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    };
    let a_doc = match read_document(a) {
        Ok(doc) => doc,
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    };
    let b_doc = match read_document(b) {
        Ok(doc) => doc,
        Err(err) => {
            eprintln!("error: {err}");
            return 1;
        }
    };

    let base_v = DocVersion::new(base_doc.doc, base_doc.names);
    let a_v = DocVersion::new(a_doc.doc, a_doc.names);
    let b_v = DocVersion::new(b_doc.doc, b_doc.names);

    let outcome = merge_documents(&base_v, &a_v, &b_v);

    if !outcome.is_clean() {
        for conflict in &outcome.conflicts {
            println!("{}", conflict.report());
            println!();
        }
        return 1;
    }

    match out {
        Some(path) => {
            let doc =
                Document::from_doc(outcome.merged.doc, outcome.merged.names, ActionLog::new());
            if let Err(err) = write_document(path, &doc) {
                eprintln!("error: {err}");
                return 1;
            }
        }
        None => println!("{}", outcome.merged.render()),
    }
    0
}
