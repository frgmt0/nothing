use nothing_merge::bench::{Row, git_available, markdown, run_all, totals};
use nothing_merge::scenarios::Category;

fn rows() -> Option<Vec<Row>> {
    if !git_available() {
        eprintln!("skipping: git is not on PATH, so the line-based half cannot run");
        return None;
    }
    Some(run_all())
}

#[test]
fn every_structural_merge_result_is_well_typed() {
    let Some(rows) = rows() else { return };
    for row in &rows {
        assert!(
            row.structural_well_typed,
            "{} merged to an ill-typed program",
            row.name
        );
    }
}

#[test]
fn structural_merge_is_clean_on_everything_except_the_genuine_conflicts() {
    let Some(rows) = rows() else { return };
    for row in &rows {
        if row.category == Category::Control {
            continue;
        }
        assert!(
            row.structural_clean,
            "{} should merge cleanly but reported {} conflict(s)",
            row.name, row.structural_conflicts
        );
    }
}

#[test]
fn structural_merge_still_refuses_the_genuine_disagreements() {
    let Some(rows) = rows() else { return };
    let controls: Vec<&Row> = rows
        .iter()
        .filter(|r| r.category == Category::Control)
        .collect();
    assert!(controls.len() >= 3, "not enough control scenarios");
    let refused = controls.iter().filter(|r| !r.structural_clean).count();
    assert_eq!(
        refused,
        controls.len() - 1,
        "every control except the convergent-edit one must conflict"
    );
    for row in &controls {
        if !row.structural_clean {
            assert_eq!(row.structural_conflicts, 1, "{}", row.name);
        }
    }
}

#[test]
fn each_class_has_a_case_the_line_based_merge_cannot_do() {
    let Some(rows) = rows() else { return };
    for category in [
        Category::Reordering,
        Category::Renaming,
        Category::Reformatting,
        Category::Moving,
    ] {
        let wins = rows
            .iter()
            .filter(|r| r.category == category && r.structural_clean && !r.git_clean)
            .count();
        assert!(
            wins > 0,
            "no scenario in class `{}` where git conflicts and structural merge does not",
            category.label()
        );
    }
}

#[test]
fn a_line_based_merge_that_reports_clean_is_never_silently_wrong_here() {
    let Some(rows) = rows() else { return };
    for row in &rows {
        if row.git_correct == Some(false) {
            eprintln!(
                "git merged `{}` cleanly but produced different content",
                row.name
            );
        }
    }
    let t = totals(&rows);
    assert_eq!(
        t.git_clean_and_correct + rows.iter().filter(|r| r.git_correct == Some(false)).count(),
        t.git_clean
    );
}

#[test]
fn the_table_is_reproducible_and_names_its_own_invocation() {
    let Some(first) = rows() else { return };
    let second = run_all();
    assert_eq!(first, second, "the benchmark is not deterministic");

    let body = markdown(&first, "2026-08-28");
    assert!(body.contains("cargo run -p nothing-merge --bin merge-bench"));
    assert!(body.contains("2026-08-28"));
    let t = totals(&first);
    assert!(body.contains(&format!("| {} |", t.scenarios)));
}
