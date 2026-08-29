use std::path::Path;

use nothing_action::log::ActionLog;
use nothing_core::exp::Id;
use nothing_core::names::NameTable;
use nothing_merge::document::changes;
use nothing_merge::text::{CANONICAL, to_text};
use nothing_merge::version::Version;
use nothing_merge::{DefChange, DocVersion, diff, merge_documents};
use nothing_store::Document;

use crate::fileio::{read_document, write_document};

pub const MERGE_DRIVER_HELP: &str = "\
nothing merge-driver <base> <ours> <theirs> [<marker-size>] [<path>]

The git merge driver. Configure it once and `git merge` resolves `.n` files
through the structural three-way merge instead of by text:

  git config merge.nothing.name \"nothing structural merge\"
  git config merge.nothing.driver \"nothing merge-driver %O %A %B %L %P\"
  echo '*.n -text merge=nothing diff=nothing' >> .gitattributes

On a clean merge the result is written back over <ours>, which is the file
git reads, and the exit status is 0. On a conflict every conflict is
reported on standard error, <ours> is left exactly as git wrote it, and the
exit status is 1, which git records as a conflicted path. A file that will
not decode, or a merge whose result is not well-typed, is also a status of
1 and writes nothing. See GIT.md.

Exit status: 0 on a clean merge, 1 on conflicts, a file error, or an
ill-typed result.

Options:
  -h, --help   print this help and exit";

pub const TEXTCONV_HELP: &str = "\
nothing textconv <file>

Print a stable structural rendering of <file> on standard output. This is
git's `textconv` hook: it lets `git diff`, `git show` and `git log -p` show
line diffs of a `.n` document, which git otherwise sees as a binary blob.

  git config diff.nothing.textconv \"nothing textconv\"
  echo '*.n -text diff=nothing' >> .gitattributes

The rendering is one definition at a time — name, type, doc line, then the
body one syntactic group per line — so editing one part of one definition
produces a small hunk. It is a projection, not the file: it cannot be
applied with `git apply` and it is not a serialisation format. See GIT.md.

Exit status: 0 on success, 1 if <file> cannot be read or decoded.

Options:
  -h, --help   print this help and exit";

pub const DIFF_DRIVER_HELP: &str = "\
nothing diff-driver <path> <old-file> <old-hex> <old-mode> <new-file> <new-hex> <new-mode>

Git's external diff command for `.n` files. Instead of a line diff of a
rendering, it prints the typed operations the structural diff recovered:
which definitions were added, removed, renamed, re-annotated or moved, and
inside each edited definition one line per `Operation`.

  git config diff.nothing.command \"nothing diff-driver\"
  echo '*.n -text diff=nothing' >> .gitattributes

Setting `diff.nothing.command` replaces the whole diff, so it wins over
`diff.nothing.textconv` and ignores `-U`. `git diff` uses it directly;
`git log -p` and `git show` need `--ext-diff` as well. Git passes the seven
arguments above, or <path> alone for an unmerged file; both are handled. A
side that will not decode is reported, not fatal. See GIT.md.

Exit status: 0.

Options:
  -h, --help   print this help and exit";

fn read_three(paths: [&Path; 3]) -> Result<[Document; 3], String> {
    let [base, ours, theirs] = paths.map(read_document);
    Ok([base?, ours?, theirs?])
}

fn version_of(document: &Document) -> DocVersion {
    DocVersion::documented(
        document.doc.clone(),
        document.names.clone(),
        document.docs.clone(),
    )
}

fn display_names(document: &Document) -> NameTable {
    nothing_stdlib::prelude().names_for(&document.names)
}

pub fn run_merge_driver(base: &Path, ours: &Path, theirs: &Path, path: Option<&str>) -> i32 {
    let subject = path.unwrap_or("the file being merged");
    let [base_doc, ours_doc, theirs_doc] = match read_three([base, ours, theirs]) {
        Ok(documents) => documents,
        Err(err) => {
            eprintln!("nothing merge-driver: {subject}: {err}");
            return 1;
        }
    };

    let outcome = merge_documents(
        &version_of(&base_doc),
        &version_of(&ours_doc),
        &version_of(&theirs_doc),
    );

    if !outcome.is_clean() {
        eprintln!(
            "nothing merge-driver: {subject}: {} structural conflict(s)",
            outcome.conflicts.len()
        );
        for conflict in &outcome.conflicts {
            eprintln!("{}", conflict.report());
            eprintln!();
        }
        eprintln!("resolve them with `nothing edit {subject}`, then `git add {subject}`");
        return 1;
    }

    let prelude = nothing_stdlib::prelude();
    if !outcome.merged.doc.is_well_typed_in(prelude.ctx()) {
        eprintln!(
            "nothing merge-driver: {subject}: the merged document is not well-typed; \
             refusing to write it"
        );
        return 1;
    }

    let merged = Document::documented(
        outcome.merged.doc,
        outcome.merged.names,
        outcome.merged.docs,
        ActionLog::new(),
    );
    if let Err(err) = write_document(ours, &merged) {
        eprintln!("nothing merge-driver: {subject}: {err}");
        return 1;
    }
    0
}

pub fn structural_text(document: &Document) -> String {
    let names = display_names(document);
    let mut out = String::new();
    for def in document.doc.defs() {
        out.push_str("def ");
        out.push_str(&names.display(def.id));
        out.push_str(" : ");
        out.push_str(&nothing_core::render::render_ty(&def.ann, &names));
        out.push('\n');
        if let Some(line) = document.docs.get(def.id) {
            out.push_str("doc ");
            out.push_str(line);
            out.push('\n');
        }
        let body = to_text(&Version::new(def.body.clone(), names.clone()), CANONICAL);
        for line in body.lines() {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("end\n\n");
    }
    out
}

fn emit(text: &str) {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    if handle.write_all(text.as_bytes()).is_ok() {
        let _ = handle.flush();
    }
}

pub fn run_textconv(path: &Path) -> i32 {
    match read_document(path) {
        Ok(document) => {
            emit(&structural_text(&document));
            0
        }
        Err(err) => {
            eprintln!("nothing textconv: {err}");
            1
        }
    }
}

fn renamed_ids(old: &Document, new: &Document) -> Vec<(Id, String, String)> {
    let mut ids: Vec<Id> = old.names.ids();
    for id in new.names.ids() {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids.sort();
    ids.into_iter()
        .filter_map(|id| {
            let was = old.names.get(id);
            let now = new.names.get(id);
            if was == now {
                return None;
            }
            Some((
                id,
                was.unwrap_or("(unnamed)").to_string(),
                now.unwrap_or("(unnamed)").to_string(),
            ))
        })
        .collect()
}

fn redocumented_ids(old: &Document, new: &Document) -> Vec<(Id, String, String)> {
    let mut ids: Vec<Id> = old.docs.ids();
    for id in new.docs.ids() {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids.sort();
    ids.into_iter()
        .filter_map(|id| {
            let was = old.docs.get(id);
            let now = new.docs.get(id);
            if was == now {
                return None;
            }
            Some((
                id,
                was.unwrap_or("(undocumented)").to_string(),
                now.unwrap_or("(undocumented)").to_string(),
            ))
        })
        .collect()
}

fn definition_line(names: &NameTable, id: Id, tail: &str) -> String {
    format!("  definition `{}` {tail}\n", names.display(id))
}

fn typed_operations(old: &Document, new: &Document) -> String {
    let old_names = display_names(old);
    let new_names = display_names(new);
    let old_version = version_of(old);
    let new_version = version_of(new);

    let mut out = String::new();
    let mut said_anything = false;

    let renames = renamed_ids(old, new);
    if !renames.is_empty() {
        said_anything = true;
        out.push_str("names\n");
        for (_, was, now) in &renames {
            out.push_str(&format!("  `{was}` is now `{now}`\n"));
        }
    }

    let redocs = redocumented_ids(old, new);
    if !redocs.is_empty() {
        said_anything = true;
        out.push_str("documentation\n");
        for (id, was, now) in &redocs {
            out.push_str(&format!("  `{}`: {was} -> {now}\n", new_names.display(*id)));
        }
    }

    let mut definitions = String::new();
    for (id, change) in changes(&old_version, &new_version) {
        match change {
            DefChange::Untouched => {}
            DefChange::Added => {
                let Some(def) = new.doc.get(id) else { continue };
                definitions.push_str(&definition_line(&new_names, id, "added"));
                definitions.push_str(&format!(
                    "    {}\n",
                    nothing_core::doc::render_def(def, &new_names)
                ));
            }
            DefChange::Removed => {
                let Some(def) = old.doc.get(id) else { continue };
                definitions.push_str(&definition_line(&old_names, id, "removed"));
                definitions.push_str(&format!(
                    "    {}\n",
                    nothing_core::doc::render_def(def, &old_names)
                ));
            }
            DefChange::Renamed => {
                let was = old.names.get(id).unwrap_or("(unnamed)");
                definitions.push_str(&definition_line(
                    &new_names,
                    id,
                    &format!("renamed from `{was}`"),
                ));
            }
            DefChange::Reannotated => {
                let was = old.doc.get(id).map(|def| def.ann.to_string());
                let now = new.doc.get(id).map(|def| def.ann.to_string());
                definitions.push_str(&definition_line(
                    &new_names,
                    id,
                    &format!(
                        "re-annotated {} -> {}",
                        was.unwrap_or_default(),
                        now.unwrap_or_default()
                    ),
                ));
            }
            DefChange::Moved => {
                let was = old.doc.index_of(id);
                let now = new.doc.index_of(id);
                definitions.push_str(&definition_line(
                    &new_names,
                    id,
                    &format!(
                        "moved from position {} to position {}",
                        was.map(|i| i.to_string()).unwrap_or_default(),
                        now.map(|i| i.to_string()).unwrap_or_default()
                    ),
                ));
            }
            DefChange::Edited => {
                let (Some(was), Some(now)) = (old.doc.get(id), new.doc.get(id)) else {
                    continue;
                };
                definitions.push_str(&definition_line(&new_names, id, "edited"));
                let ops = diff(
                    &Version::new(was.body.clone(), new_names.clone()),
                    &Version::new(now.body.clone(), new_names.clone()),
                );
                if ops.is_empty() {
                    definitions.push_str("    (no typed operation recovered)\n");
                }
                for op in &ops {
                    definitions.push_str(&format!(
                        "    [{}] {} -- now `{}`\n",
                        op.kind(),
                        op.describe(&was.body, &new_names),
                        op.outcome(&new_names)
                    ));
                }
            }
        }
    }

    if !definitions.is_empty() {
        said_anything = true;
        out.push_str("definitions\n");
        out.push_str(&definitions);
    }

    if !said_anything {
        out.push_str("no structural change\n");
    }
    out
}

fn listing(document: &Document, mark: &str) -> String {
    let names = display_names(document);
    let mut out = String::new();
    out.push_str("definitions\n");
    for def in document.doc.defs() {
        out.push_str(&format!(
            "  definition `{}` {mark}\n    {}\n",
            names.display(def.id),
            nothing_core::doc::render_def(def, &names)
        ));
    }
    out
}

pub fn run_diff_driver(args: &[String]) -> i32 {
    let Some(path) = args.first() else {
        eprintln!("nothing diff-driver: expected at least <path>");
        eprintln!("{DIFF_DRIVER_HELP}");
        return 1;
    };

    if args.len() < 7 {
        emit(&format!(
            "--- a/{path}\n+++ b/{path}\nunmerged; git gave no blobs to compare\n"
        ));
        return 0;
    }

    let renamed_to = args.get(7).map(String::as_str);
    let mut out = format!("--- a/{path}\n+++ b/{}\n", renamed_to.unwrap_or(path));

    let old = read_document(Path::new(&args[1]));
    let new = read_document(Path::new(&args[4]));
    match (old, new) {
        (Ok(old), Ok(new)) => out.push_str(&typed_operations(&old, &new)),
        (Err(_), Ok(new)) => {
            out.push_str("the old side is not a nothing document; the whole document is new\n");
            out.push_str(&listing(&new, "added"));
        }
        (Ok(old), Err(_)) => {
            out.push_str("the new side is not a nothing document; the whole document is gone\n");
            out.push_str(&listing(&old, "removed"));
        }
        (Err(old_err), Err(new_err)) => {
            out.push_str("neither side is a nothing document\n");
            out.push_str(&format!("  old: {old_err}\n  new: {new_err}\n"));
        }
    }
    emit(&out);
    0
}
