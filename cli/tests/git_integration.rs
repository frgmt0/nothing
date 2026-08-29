use std::path::{Path, PathBuf};
use std::process::Command;

use nothing_action::log::ActionLog;
use nothing_core::doc::{Def, Doc};
use nothing_core::exp::{Exp, Id, Op};
use nothing_core::names::NameTable;
use nothing_core::ty::Ty;
use nothing_store::{Document, encode_document};

const HELPER: Id = Id::from_u128(0x0011);
const MAIN: Id = Id::from_u128(0x0022);
const N: Id = Id::from_u128(0x0033);

const PROGRAM: &str = "program.n";

fn nothing_bin() -> &'static str {
    env!("CARGO_BIN_EXE_nothing")
}

fn git_is_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

struct Ran {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Ran {
    fn said(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }

    fn expect_success(self, what: &str) -> Ran {
        assert_eq!(self.code, 0, "`{what}` failed:\n{}", self.said());
        self
    }
}

struct Repo {
    dir: PathBuf,
}

impl Drop for Repo {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

impl Repo {
    fn create(name: &str) -> Repo {
        let dir = std::env::temp_dir().join(format!("nothing-git-integration-{name}"));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("the scratch repository directory is creatable");
        let repo = Repo { dir };
        repo.git(&["init"]).expect_success("git init");
        repo.git(&["checkout", "-b", "trunk"])
            .expect_success("git checkout -b trunk");
        for setting in [
            ("user.email", "tests@nothing.invalid"),
            ("user.name", "nothing tests"),
            ("core.autocrlf", "false"),
            ("commit.gpgsign", "false"),
        ] {
            repo.git(&["config", setting.0, setting.1])
                .expect_success("git config");
        }
        repo
    }

    fn git(&self, args: &[&str]) -> Ran {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_MERGE_AUTOEDIT", "no")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("HOME", &self.dir)
            .output()
            .expect("git runs");
        Ran {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        }
    }

    fn write(&self, name: &str, bytes: &[u8]) {
        std::fs::write(self.dir.join(name), bytes).expect("the scratch file is writable");
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn commit(&self, message: &str) {
        self.git(&["add", "-A"]).expect_success("git add -A");
        self.git(&["commit", "-m", message])
            .expect_success("git commit");
    }

    fn use_merge_driver(&self) {
        self.git(&["config", "merge.nothing.name", "nothing structural merge"])
            .expect_success("git config merge.nothing.name");
        self.git(&[
            "config",
            "merge.nothing.driver",
            &format!("'{}' merge-driver %O %A %B %L %P", nothing_bin()),
        ])
        .expect_success("git config merge.nothing.driver");
    }

    fn use_textconv(&self) {
        self.git(&[
            "config",
            "diff.nothing.textconv",
            &format!("'{}' textconv", nothing_bin()),
        ])
        .expect_success("git config diff.nothing.textconv");
    }

    fn use_diff_driver(&self) {
        self.git(&[
            "config",
            "diff.nothing.command",
            &format!("'{}' diff-driver", nothing_bin()),
        ])
        .expect_success("git config diff.nothing.command");
    }
}

fn names() -> NameTable {
    let mut names = NameTable::new();
    names.set(HELPER, "helper");
    names.set(MAIN, "main");
    names.set(N, "n");
    names
}

fn definitions(helper_body: Exp, main_body: Exp) -> Vec<Def> {
    vec![
        Def::new(
            HELPER,
            Ty::Arrow(Box::new(Ty::Num), Box::new(Ty::Num)),
            helper_body,
        ),
        Def::new(MAIN, Ty::Num, main_body),
    ]
}

fn encoded(defs: Vec<Def>, names: NameTable) -> Vec<u8> {
    let doc = Doc::new(defs).expect("the definitions have distinct ids");
    encode_document(&Document::from_doc(doc, names, ActionLog::new()))
}

fn program(helper_body: Exp, main_body: Exp) -> Vec<u8> {
    encoded(definitions(helper_body, main_body), names())
}

fn add(right: i64) -> Exp {
    Exp::lam(
        N,
        Ty::Num,
        Exp::bin_op(Op::Add, Exp::var(N), Exp::num(right)),
    )
}

fn call(argument: i64) -> Exp {
    Exp::ap(Exp::var(HELPER), Exp::num(argument))
}

fn nothing(args: &[&str]) -> Ran {
    let out = Command::new(nothing_bin())
        .args(args)
        .output()
        .expect("the nothing binary runs");
    Ran {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

fn has_conflict_markers(path: &Path) -> bool {
    let bytes = std::fs::read(path).expect("the merged file is readable");
    bytes.windows(7).any(|w| w == b"<<<<<<<")
}

fn two_branches(repo: &Repo, ours: Vec<u8>, theirs: Vec<u8>) {
    repo.write(".gitattributes", b"*.n -text merge=nothing diff=nothing\n");
    repo.write(PROGRAM, &program(add(1), call(2)));
    repo.commit("base");

    repo.git(&["checkout", "-b", "ours"])
        .expect_success("git checkout -b ours");
    repo.write(PROGRAM, &ours);
    repo.commit("ours");

    repo.git(&["checkout", "trunk"])
        .expect_success("git checkout trunk");
    repo.git(&["checkout", "-b", "theirs"])
        .expect_success("git checkout -b theirs");
    repo.write(PROGRAM, &theirs);
    repo.commit("theirs");

    repo.git(&["checkout", "ours"])
        .expect_success("git checkout ours");
}

#[test]
fn disjoint_definition_edits_conflict_as_text_and_merge_through_the_driver() {
    if !git_is_available() {
        eprintln!("skipping: git is not on PATH");
        return;
    }

    let repo = Repo::create("definitions");
    two_branches(&repo, program(add(10), call(2)), program(add(1), call(5)));

    let without = repo.git(&["merge", "--no-edit", "theirs"]);
    assert_ne!(
        without.code,
        0,
        "without the driver git merged a binary file cleanly, so this test proves nothing:\n{}",
        without.said()
    );
    assert!(
        without.said().to_lowercase().contains("conflict"),
        "expected a conflict without the driver:\n{}",
        without.said()
    );
    repo.git(&["merge", "--abort"])
        .expect_success("git merge --abort");

    repo.use_merge_driver();
    let with = repo.git(&["merge", "--no-edit", "theirs"]);
    assert_eq!(
        with.code,
        0,
        "the structural merge driver did not resolve the merge:\n{}",
        with.said()
    );
    assert!(
        !has_conflict_markers(&repo.path(PROGRAM)),
        "the merged file carries text conflict markers"
    );
    let status = repo.git(&["status", "--porcelain"]);
    assert_eq!(
        status.stdout.trim(),
        "",
        "the merge left the tree dirty:\n{}",
        status.said()
    );

    let checked = nothing(&["check", repo.path(PROGRAM).to_str().unwrap()]);
    assert_eq!(
        checked.code,
        0,
        "the merged file is not well-typed:\n{}",
        checked.said()
    );
    assert!(
        checked.stdout.contains("well-typed: true"),
        "{}",
        checked.stdout
    );
    assert!(
        checked.stdout.contains("definitions: 2"),
        "{}",
        checked.stdout
    );

    let rendered = nothing(&["textconv", repo.path(PROGRAM).to_str().unwrap()])
        .expect_success("nothing textconv");
    assert!(
        rendered.stdout.contains("n + 10"),
        "our edit to `helper` survived:\n{}",
        rendered.stdout
    );
    assert!(
        rendered.stdout.contains("helper 5"),
        "their edit to `main` survived:\n{}",
        rendered.stdout
    );
}

#[test]
fn disjoint_edits_inside_one_definition_also_merge_through_the_driver() {
    if !git_is_available() {
        eprintln!("skipping: git is not on PATH");
        return;
    }

    let scaled = |left: i64, right: i64| {
        Exp::lam(
            N,
            Ty::Num,
            Exp::bin_op(
                Op::Mul,
                Exp::bin_op(Op::Add, Exp::var(N), Exp::num(left)),
                Exp::bin_op(Op::Add, Exp::var(N), Exp::num(right)),
            ),
        )
    };

    let repo = Repo::create("one-definition");
    repo.write(".gitattributes", b"*.n -text merge=nothing diff=nothing\n");
    repo.write(PROGRAM, &program(scaled(1, 2), call(3)));
    repo.commit("base");

    repo.git(&["checkout", "-b", "ours"])
        .expect_success("git checkout -b ours");
    repo.write(PROGRAM, &program(scaled(100, 2), call(3)));
    repo.commit("ours");

    repo.git(&["checkout", "trunk"])
        .expect_success("git checkout trunk");
    repo.git(&["checkout", "-b", "theirs"])
        .expect_success("git checkout -b theirs");
    repo.write(PROGRAM, &program(scaled(1, 200), call(3)));
    repo.commit("theirs");

    repo.git(&["checkout", "ours"])
        .expect_success("git checkout ours");

    let without = repo.git(&["merge", "--no-edit", "theirs"]);
    assert_ne!(
        without.code,
        0,
        "without the driver this merge was expected to conflict:\n{}",
        without.said()
    );
    repo.git(&["merge", "--abort"])
        .expect_success("git merge --abort");

    repo.use_merge_driver();
    let with = repo.git(&["merge", "--no-edit", "theirs"]);
    assert_eq!(
        with.code,
        0,
        "two disjoint edits inside one definition did not merge:\n{}",
        with.said()
    );

    let checked = nothing(&["check", repo.path(PROGRAM).to_str().unwrap()]);
    assert_eq!(checked.code, 0, "{}", checked.said());

    let rendered = nothing(&["textconv", repo.path(PROGRAM).to_str().unwrap()])
        .expect_success("nothing textconv");
    assert!(
        rendered.stdout.contains("n + 100") && rendered.stdout.contains("n + 200"),
        "both edits survived:\n{}",
        rendered.stdout
    );
}

#[test]
fn a_rename_on_one_side_and_a_body_edit_on_the_other_merge() {
    if !git_is_available() {
        eprintln!("skipping: git is not on PATH");
        return;
    }

    let mut renamed = names();
    renamed.set(HELPER, "bump");

    let repo = Repo::create("rename");
    two_branches(
        &repo,
        encoded(definitions(add(1), call(2)), renamed),
        program(add(7), call(2)),
    );
    repo.use_merge_driver();

    let merged = repo.git(&["merge", "--no-edit", "theirs"]);
    assert_eq!(
        merged.code,
        0,
        "a rename and a body edit are disjoint and must merge:\n{}",
        merged.said()
    );
    let rendered = nothing(&["textconv", repo.path(PROGRAM).to_str().unwrap()])
        .expect_success("nothing textconv");
    assert!(
        rendered.stdout.contains("def bump : Num -> Num")
            && rendered.stdout.contains("n + 7")
            && rendered.stdout.contains("bump 2"),
        "the rename and the edit both survived:\n{}",
        rendered.stdout
    );
    assert_eq!(
        nothing(&["check", repo.path(PROGRAM).to_str().unwrap()]).code,
        0
    );
}

#[test]
fn an_added_definition_and_an_edit_elsewhere_merge() {
    if !git_is_available() {
        eprintln!("skipping: git is not on PATH");
        return;
    }

    let extra = Id::from_u128(0x0044);
    let mut with_extra = names();
    with_extra.set(extra, "extra");
    let mut defs = definitions(add(1), call(2));
    defs.push(Def::new(extra, Ty::Num, Exp::num(99)));

    let repo = Repo::create("addition");
    two_branches(&repo, encoded(defs, with_extra), program(add(1), call(5)));
    repo.use_merge_driver();

    let merged = repo.git(&["merge", "--no-edit", "theirs"]);
    assert_eq!(
        merged.code,
        0,
        "adding a definition never conflicts with editing another:\n{}",
        merged.said()
    );
    let rendered = nothing(&["textconv", repo.path(PROGRAM).to_str().unwrap()])
        .expect_success("nothing textconv");
    assert!(
        rendered.stdout.contains("def extra : Num") && rendered.stdout.contains("helper 5"),
        "the addition and the edit both survived:\n{}",
        rendered.stdout
    );
    assert_eq!(
        nothing(&["check", repo.path(PROGRAM).to_str().unwrap()]).code,
        0
    );
}

#[test]
fn a_conflicting_merge_is_reported_and_left_conflicted() {
    if !git_is_available() {
        eprintln!("skipping: git is not on PATH");
        return;
    }

    let repo = Repo::create("conflict");
    two_branches(&repo, program(add(10), call(2)), program(add(20), call(2)));
    repo.use_merge_driver();

    let merged = repo.git(&["merge", "--no-edit", "theirs"]);
    assert_ne!(
        merged.code,
        0,
        "two edits to the same node must not merge silently:\n{}",
        merged.said()
    );
    assert!(
        merged.said().contains("conflict"),
        "the driver's report reaches the user:\n{}",
        merged.said()
    );
    let status = repo.git(&["status", "--porcelain"]);
    assert!(
        status.stdout.contains(PROGRAM),
        "git records the path as conflicted:\n{}",
        status.said()
    );
}

#[test]
fn git_log_p_shows_the_structural_rendering_not_a_binary_notice() {
    if !git_is_available() {
        eprintln!("skipping: git is not on PATH");
        return;
    }

    let repo = Repo::create("textconv");
    repo.write(".gitattributes", b"*.n -text diff=nothing\n");
    repo.write(PROGRAM, &program(add(1), call(2)));
    repo.commit("base");
    repo.write(PROGRAM, &program(add(10), call(2)));
    repo.commit("edit helper");

    let raw = repo.git(&["log", "-p", "--", PROGRAM]);
    assert!(
        raw.stdout.contains("Binary files"),
        "without textconv git treats the document as binary:\n{}",
        raw.stdout
    );

    repo.use_textconv();
    let shown = repo
        .git(&["log", "-p", "--", PROGRAM])
        .expect_success("git log -p");
    assert!(
        !shown.stdout.contains("Binary files"),
        "textconv should have replaced the binary notice:\n{}",
        shown.stdout
    );
    assert!(
        shown.stdout.contains("def helper : Num -> Num"),
        "the rendering names the definition and its type:\n{}",
        shown.stdout
    );
    assert!(
        shown.stdout.contains("-    n + 1\n") && shown.stdout.contains("+    n + 10\n"),
        "the hunk is a small line diff of the body:\n{}",
        shown.stdout
    );
    assert!(
        shown.stdout.contains("def main : Num"),
        "every definition is rendered:\n{}",
        shown.stdout
    );
}

#[test]
fn the_external_diff_driver_prints_typed_operations() {
    if !git_is_available() {
        eprintln!("skipping: git is not on PATH");
        return;
    }

    let repo = Repo::create("diff-driver");
    repo.write(".gitattributes", b"*.n -text diff=nothing\n");
    repo.write(PROGRAM, &program(add(1), call(2)));
    repo.commit("base");
    repo.write(PROGRAM, &program(add(10), call(5)));
    repo.commit("edit both");

    repo.use_diff_driver();
    let shown = repo
        .git(&["diff", "HEAD~1", "HEAD", "--", PROGRAM])
        .expect_success("git diff with the external driver");
    assert!(
        shown.stdout.contains(&format!("--- a/{PROGRAM}")),
        "the driver labels the sides:\n{}",
        shown.stdout
    );
    assert!(
        shown.stdout.contains("definition `helper` edited")
            && shown.stdout.contains("definition `main` edited"),
        "both edited definitions are reported:\n{}",
        shown.stdout
    );
    assert!(
        shown.stdout.contains("[Replace]"),
        "the typed operation kind is on the line:\n{}",
        shown.stdout
    );
    assert!(
        shown.stdout.contains("now `10`") && shown.stdout.contains("now `5`"),
        "each operation reports its outcome:\n{}",
        shown.stdout
    );

    let logged = repo
        .git(&["log", "-p", "--", PROGRAM])
        .expect_success("git log -p");
    assert!(
        logged.stdout.contains("Binary files"),
        "git log ignores an external diff driver unless asked, and GIT.md says so:\n{}",
        logged.stdout
    );

    let asked = repo
        .git(&["log", "-p", "--ext-diff", "--", PROGRAM])
        .expect_success("git log -p --ext-diff");
    assert!(
        !asked.stdout.contains("Binary files") && asked.stdout.contains("[Replace]"),
        "`--ext-diff` reaches the driver from git log:\n{}",
        asked.stdout
    );
}

#[test]
fn a_new_file_and_an_undecodable_side_degrade_instead_of_failing() {
    let listed = nothing(&[
        "diff-driver",
        PROGRAM,
        "/dev/null",
        "0000000",
        ".",
        "/dev/null",
        "0000000",
        ".",
    ]);
    assert_eq!(listed.code, 0, "{}", listed.said());
    assert!(
        listed.stdout.contains("neither side is a nothing document"),
        "{}",
        listed.stdout
    );

    let unmerged = nothing(&["diff-driver", PROGRAM]);
    assert_eq!(unmerged.code, 0, "{}", unmerged.said());
    assert!(unmerged.stdout.contains("unmerged"), "{}", unmerged.stdout);
}
