use std::process::Command;

use nothing_action::log::ActionLog;
use nothing_core::exp::Exp;
use nothing_core::names::NameTable;
use nothing_core::stack::on_deep_stack;
use nothing_store::{Document, encode_document};

const CI_STACK_BYTES: usize = 2 * 1024 * 1024;

fn on_a_ci_sized_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(CI_STACK_BYTES)
        .spawn(work)
        .expect("spawn the small-stack thread a CI runner would give a test")
        .join()
        .expect("the small-stack thread finished without overflowing")
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_nothing")
}

fn scratch_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("nothing-cli-deep-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_long_list(name: &str, len: i64) -> std::path::PathBuf {
    let path = scratch_dir().join(name);
    let bytes = on_deep_stack(move || {
        let doc = Document::new(
            Exp::list((0..len).map(Exp::num)),
            NameTable::new(),
            ActionLog::new(),
        );
        encode_document(&doc)
    });
    std::fs::write(&path, bytes).unwrap();
    path
}

fn run(args: &[&str]) -> (i32, String) {
    let output = Command::new(bin())
        .args(args)
        .output()
        .expect("the nothing binary runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
    )
}

fn run_with_a_small_main_stack(args: &[&str]) -> (i32, String) {
    let script = format!("ulimit -s 2048; exec {} {}", bin(), args.join(" "));
    let output = Command::new("sh")
        .arg("-c")
        .arg(script)
        .output()
        .expect("the shell runs the nothing binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
    )
}

#[test]
fn nothing_run_evaluates_a_list_far_deeper_than_a_ci_stack() {
    on_a_ci_sized_stack(|| {
        let path = write_long_list("run-deep-list.nothing", 5_000);
        let (code, stdout) = run(&["run", path.to_str().unwrap()]);
        assert_eq!(
            code,
            0,
            "stdout starts: {}",
            &stdout[..stdout.len().min(80)]
        );
        assert!(stdout.starts_with("0 :: 1 :: 2 :: "), "{}", &stdout[..40]);
        assert!(stdout.trim_end().ends_with(":: 4999 :: nil"));
    });
}

#[test]
fn nothing_run_evaluates_a_long_list_with_a_small_main_stack() {
    on_a_ci_sized_stack(|| {
        let path = write_long_list("run-small-stack.nothing", 5_000);
        let (code, stdout) = run_with_a_small_main_stack(&["run", path.to_str().unwrap()]);
        assert_eq!(
            code,
            0,
            "the run did not survive a two megabyte main stack: {}",
            &stdout[..stdout.len().min(80)]
        );
        assert!(stdout.trim_end().ends_with(":: 4999 :: nil"));
    });
}

#[test]
fn nothing_textconv_renders_a_long_list_with_a_small_main_stack() {
    on_a_ci_sized_stack(|| {
        let path = write_long_list("textconv-small-stack.nothing", 5_000);
        let (code, stdout) = run_with_a_small_main_stack(&["textconv", path.to_str().unwrap()]);
        assert_eq!(code, 0, "stdout: {}", &stdout[..stdout.len().min(80)]);
        assert!(stdout.starts_with("def main : "), "stdout: {stdout:.80}");
        assert!(stdout.contains("4999 :: nil"), "stdout: {stdout:.80}");
    });
}

#[test]
fn nothing_diff_driver_compares_two_long_lists_with_a_small_main_stack() {
    on_a_ci_sized_stack(|| {
        let old = write_long_list("diff-driver-old.nothing", 5_000);
        let new = write_long_list("diff-driver-new.nothing", 5_001);
        let (code, stdout) = run_with_a_small_main_stack(&[
            "diff-driver",
            "long.n",
            old.to_str().unwrap(),
            "0000000",
            "100644",
            new.to_str().unwrap(),
            "0000000",
            "100644",
        ]);
        assert_eq!(code, 0, "stdout: {}", &stdout[..stdout.len().min(80)]);
        assert!(stdout.contains("definition `main` edited"), "{stdout:.200}");
    });
}

#[test]
fn nothing_check_type_checks_a_long_list_with_a_small_main_stack() {
    on_a_ci_sized_stack(|| {
        let path = write_long_list("check-small-stack.nothing", 5_000);
        let (code, stdout) = run_with_a_small_main_stack(&["check", path.to_str().unwrap()]);
        assert_eq!(code, 0, "stdout: {stdout}");
        assert!(stdout.contains("well-typed: true"), "stdout: {stdout}");
    });
}
