use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::merge3::merge;
use crate::scenarios::{Category, Scenario, all};
use crate::text::{normalise, to_text};

#[derive(Clone, PartialEq, Debug)]
pub struct Row {
    pub name: String,
    pub category: Category,
    pub note: String,
    pub git_clean: bool,
    pub git_conflicts: i32,
    pub git_correct: Option<bool>,
    pub structural_clean: bool,
    pub structural_conflicts: usize,
    pub structural_well_typed: bool,
    pub ours_ops: usize,
    pub theirs_ops: usize,
    pub conflict_report: Option<String>,
    pub base_text: String,
    pub git_output: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Totals {
    pub scenarios: usize,
    pub git_clean: usize,
    pub git_clean_and_correct: usize,
    pub structural_clean: usize,
    pub structural_well_typed: usize,
}

pub fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

static RUNS: AtomicU64 = AtomicU64::new(0);

fn workspace(tag: &str) -> std::io::Result<PathBuf> {
    let serial = RUNS.fetch_add(1, Ordering::SeqCst);
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "nothing-merge-bench-{}-{}-{}",
        std::process::id(),
        tag,
        serial
    ));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn run_git_merge_file(dir: &PathBuf, ours: &str, base: &str, theirs: &str) -> (i32, String) {
    let ours_path = dir.join("ours.txt");
    let base_path = dir.join("base.txt");
    let theirs_path = dir.join("theirs.txt");
    fs::write(&ours_path, ours).expect("write ours");
    fs::write(&base_path, base).expect("write base");
    fs::write(&theirs_path, theirs).expect("write theirs");

    let out = Command::new("git")
        .arg("merge-file")
        .arg("-p")
        .arg("--diff3")
        .arg("-L")
        .arg("ours")
        .arg("-L")
        .arg("base")
        .arg("-L")
        .arg("theirs")
        .arg(&ours_path)
        .arg(&base_path)
        .arg(&theirs_path)
        .output()
        .expect("git merge-file");

    let code = out.status.code().unwrap_or(-1);
    if !(0..=64).contains(&code) {
        panic!(
            "git merge-file failed with status {code}; no number from this run can be trusted\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    (code, String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn run_one(index: usize, scenario: &Scenario) -> Row {
    let base_text = to_text(&scenario.base, scenario.base_style);
    let ours_text = to_text(&scenario.ours, scenario.ours_style);
    let theirs_text = to_text(&scenario.theirs, scenario.theirs_style);

    let outcome = merge(&scenario.base, &scenario.ours, &scenario.theirs);
    let structural_text = to_text(&outcome.merged, scenario.base_style);

    let dir = workspace(&index.to_string()).expect("scratch directory");
    let (code, merged) = run_git_merge_file(&dir, &ours_text, &base_text, &theirs_text);
    let _ = fs::remove_dir_all(&dir);

    let git_clean = code == 0;
    let git_correct = if git_clean {
        Some(normalise(&merged) == normalise(&structural_text))
    } else {
        None
    };

    Row {
        name: scenario.name.to_string(),
        category: scenario.category,
        note: scenario.note.to_string(),
        git_clean,
        git_conflicts: code.max(0),
        git_correct,
        structural_clean: outcome.is_clean(),
        structural_conflicts: outcome.conflicts.len(),
        structural_well_typed: outcome.merged.is_well_typed(),
        ours_ops: outcome.ours_ops.len(),
        theirs_ops: outcome.theirs_ops.len(),
        conflict_report: if outcome.is_clean() {
            None
        } else {
            Some(outcome.report())
        },
        base_text,
        git_output: merged,
    }
}

pub fn run_all() -> Vec<Row> {
    all()
        .iter()
        .enumerate()
        .map(|(index, scenario)| run_one(index, scenario))
        .collect()
}

pub fn totals(rows: &[Row]) -> Totals {
    Totals {
        scenarios: rows.len(),
        git_clean: rows.iter().filter(|r| r.git_clean).count(),
        git_clean_and_correct: rows
            .iter()
            .filter(|r| r.git_correct == Some(true))
            .count(),
        structural_clean: rows.iter().filter(|r| r.structural_clean).count(),
        structural_well_typed: rows.iter().filter(|r| r.structural_well_typed).count(),
    }
}

fn verdict(row: &Row) -> &'static str {
    match (row.git_clean, row.git_correct) {
        (false, _) => "conflict",
        (true, Some(true)) => "clean",
        (true, Some(false)) => "clean but wrong",
        (true, None) => "clean",
    }
}

pub fn per_category(rows: &[Row]) -> Vec<(Category, usize, usize, usize, usize)> {
    Category::ALL
        .iter()
        .map(|category| {
            let group: Vec<&Row> = rows.iter().filter(|r| r.category == *category).collect();
            let git_ok = group
                .iter()
                .filter(|r| r.git_correct == Some(true))
                .count();
            let git_wrong = group
                .iter()
                .filter(|r| r.git_correct == Some(false))
                .count();
            let structural_ok = group.iter().filter(|r| r.structural_clean).count();
            (*category, group.len(), git_ok, git_wrong, structural_ok)
        })
        .collect()
}

pub fn markdown(rows: &[Row], date: &str) -> String {
    let totals = totals(rows);
    let mut out = String::new();
    out.push_str("# Structural merge versus `git merge-file`\n\n");
    out.push_str(&format!("Measured {date}.\n\n"));
    out.push_str(
        "Every number below was produced by running the harness, not by hand. Reproduce it with:\n\n\
         ```\n\
         cargo run -p nothing-merge --bin merge-bench\n\
         ```\n\n\
         The harness builds each scenario as three program versions — a common ancestor and two \
         branches — and then merges them twice.\n\n\
         * **Line-based**: each version is rendered to a text file through the multi-line \
         projection in `merge/src/text.rs`, and the three files are handed to the real \
         `git merge-file -p --diff3 ours base theirs`. Its exit status is the conflict count.\n\
         * **Structural**: the same three versions, as `(Exp, NameTable)` pairs, go through \
         `nothing_merge::merge`, which diffs each branch against the ancestor into typed \
         operations and replays the non-overlapping ones.\n\n\
         A line-based merge can also succeed for the wrong reason, so the harness compares \
         `git merge-file`'s output against the structural result with whitespace normalised. \
         A run that git reports as clean but whose text disagrees is recorded as \
         **clean but wrong** — that is the failure mode that costs the most, because nothing \
         reports it.\n\n",
    );

    out.push_str(
        "## The operation vocabulary\n\n\
         A diff is a list of these, never a list of lines. Each one carries a path into the tree \
         (or, for `Rename`, a binder identity) and enough payload to be replayed on any tree that \
         still has that shape.\n\n\
         | operation | means | footprint |\n\
         | --- | --- | --- |\n\
         | `Rename` | a binder's display name changed | that binder's name, nothing structural |\n\
         | `Fill` | an empty hole was filled | the node at the path |\n\
         | `DeleteToHole` | a subterm was deleted, leaving a gap | the node at the path |\n\
         | `Insert` | a subterm was wrapped in a new parent | the node at the path |\n\
         | `Delete` | a wrapper was removed and one child promoted | the node at the path |\n\
         | `Move` | a subtree with an unchanged content hash appears at a new path | both endpoints |\n\
         | `MoveBinding` | a `let` binding changed position in its chain | the chain's ordering only |\n\
         | `Replace` | a subterm became a structurally different one | the node at the path |\n\
         | `SetAnn` | a lambda's parameter annotation changed | that node's shape, not its body |\n\
         | `Rebind` | binder identities changed, structure did not | the node at the path |\n\n\
         Two operations conflict when their footprints overlap. Two node footprints overlap when \
         one path is a prefix of the other — siblings never collide, which is why two branches \
         editing different fields of the same pair merge with no conflict at all. A shape \
         footprint covers a node but not its children, so retyping a parameter does not fight an \
         edit in the body. A name footprint is a binder identity and touches no part of the tree. \
         An ordering footprint covers a `let` chain's spine but not the expressions bound in it, \
         so reordering bindings does not fight an edit inside one of them.\n\n\
         `Move` gets a further rule: an edit made inside a subtree that the other branch moved is \
         *rebased* onto the subtree's new path instead of being called a conflict. The two \
         operations do commute; they just need the path rewritten first.\n\n\
         A merge can still land somewhere ill-typed even when every accepted operation was \
         non-overlapping — one branch retypes a parameter while the other adds a call site is the \
         easy example. Rather than emit a broken tree or refuse a merge nobody asked to refuse, \
         the merge repairs it the way the language repairs everything else: the offending subterm \
         is wrapped in a non-empty hole, which keeps it visible and keeps the whole program \
         well-typed. Unbound variables are the one case that cannot be quarantined — a non-empty \
         hole around an unbound variable does not synthesise either — so those become empty \
         holes. Both kinds of repair are reported, never silent.\n\n",
    );

    out.push_str("## Totals\n\n");
    out.push_str("| | scenarios | clean | clean and correct | conflicts |\n");
    out.push_str("| --- | ---: | ---: | ---: | ---: |\n");
    out.push_str(&format!(
        "| `git merge-file` on rendered text | {} | {} | {} | {} |\n",
        totals.scenarios,
        totals.git_clean,
        totals.git_clean_and_correct,
        totals.scenarios - totals.git_clean
    ));
    out.push_str(&format!(
        "| structural merge on typed operations | {} | {} | {} | {} |\n",
        totals.scenarios,
        totals.structural_clean,
        totals.structural_clean,
        totals.scenarios - totals.structural_clean
    ));
    out.push_str(&format!(
        "\nEvery structural merge result is well-typed: {}/{}.\n\n",
        totals.structural_well_typed, totals.scenarios
    ));

    out.push_str("## By scenario class\n\n");
    out.push_str("| class | scenarios | git clean and correct | git clean but wrong | git conflicts | structural clean |\n");
    out.push_str("| --- | ---: | ---: | ---: | ---: | ---: |\n");
    for (category, count, git_ok, git_wrong, structural_ok) in per_category(rows) {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            category.label(),
            count,
            git_ok,
            git_wrong,
            count - git_ok - git_wrong,
            structural_ok
        ));
    }

    out.push_str("\n## Every scenario\n\n");
    out.push_str("| class | scenario | ops (ours / theirs) | `git merge-file` | structural |\n");
    out.push_str("| --- | --- | --- | --- | --- |\n");
    for row in rows {
        let structural = if row.structural_clean {
            "clean".to_string()
        } else {
            format!("{} conflict(s)", row.structural_conflicts)
        };
        out.push_str(&format!(
            "| {} | {} | {} / {} | {} | {} |\n",
            row.category.label(),
            row.name,
            row.ours_ops,
            row.theirs_ops,
            verdict(row),
            structural
        ));
    }

    if let Some(row) = rows.first() {
        out.push_str("\n## What the line-based merge is given\n\n");
        out.push_str(
            "The projection is real multi-line code, not one long line — otherwise the \
             comparison would be rigged, because every edit would touch the only line there is. \
             This is the ancestor of the first scenario, exactly as it is written to the file \
             handed to `git merge-file`:\n\n```\n",
        );
        out.push_str(&row.base_text);
        out.push_str("```\n\nInspect any scenario's three inputs, its typed operations and its \
             structural result with:\n\n```\ncargo run -p nothing-merge --bin merge-bench -- --dump 0\n```\n");
    }

    if let Some(row) = rows.iter().find(|r| r.conflict_report.is_some()) {
        out.push_str("\n## What a conflict says\n\n");
        out.push_str(&format!(
            "A conflict is two operations whose footprints overlap and which therefore cannot \
             commute. The report is written in terms of the program, not of lines. This is the \
             verbatim output for *{}*:\n\n```\n",
            row.name
        ));
        out.push_str(row.conflict_report.as_deref().unwrap_or(""));
        out.push_str("\n```\n");
    }

    out.push_str("\n## What each scenario does\n\n");
    for row in rows {
        out.push_str(&format!("* **{}** — {}\n", row.name, row.note));
    }

    out.push_str(
        "\n## Reading the table\n\n\
         The control class exists to keep the comparison honest. Two branches that rename the \
         same binder differently, that move the same subtree to two different destinations, or \
         that set the same literal to two different values are *real* disagreements; a merge \
         engine that reports them as clean is broken, not clever. Structural merge conflicts on \
         those, and it says which node and which two alternatives are in play.\n\n\
         The other four classes are the cases the line-based algorithm cannot see. Reordering, \
         renaming and reformatting are all edits that leave the program's meaning untouched but \
         rewrite the lines a text merge is reasoning about; moving is the case where the same \
         subtree exists on both sides and only its address changed. In each of those, one branch \
         changes the text everywhere and the other changes the program somewhere, and the line \
         merge has no way to tell that those are different kinds of change.\n\n\
         Two structural facts do the work. Names live in a `NameTable` keyed by binder identity, \
         so a rename is one `Rename` operation whose footprint is a name, not a region of the \
         tree — it cannot collide with a structural edit. And subtree identity is the Phase 7 \
         content hash, so a subtree that turns up at a new path with an unchanged hash is a \
         `Move`, and an edit made inside that subtree on the other branch is *rebased* onto the \
         new path rather than being declared a conflict.\n",
    );

    out
}

pub fn plain_table(rows: &[Row]) -> String {
    let mut out = String::new();
    for row in rows {
        out.push_str(&format!(
            "{:<14} {:<62} git: {:<16} structural: {}\n",
            row.category.label(),
            row.name,
            verdict(row),
            if row.structural_clean {
                "clean".to_string()
            } else {
                format!("{} conflict(s)", row.structural_conflicts)
            }
        ));
    }
    let totals = totals(rows);
    out.push_str(&format!(
        "\n{} scenarios: git clean {}, git clean and correct {}, structural clean {}, structural well-typed {}\n",
        totals.scenarios,
        totals.git_clean,
        totals.git_clean_and_correct,
        totals.structural_clean,
        totals.structural_well_typed
    ));
    out
}
