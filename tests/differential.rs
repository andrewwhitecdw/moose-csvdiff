//! Differential tests: run the Rust port and the Python reference
//! (`python/mooseutils/csvdiff.py` from the MOOSE checkout) on identical
//! inputs and require byte-identical stdout and equal exit codes.
//!
//! The tests skip silently when the Python reference is not available
//! (e.g. in CI), so they serve as local verification during development.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const PYTHON_REFERENCE: &str = "/home/andrewh/code/moose/python/mooseutils/csvdiff.py";

struct Case {
    dir: PathBuf,
}

impl Case {
    fn new(name: &str) -> Case {
        let dir = std::env::temp_dir().join(format!("csvdiff-diff-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("case dir");
        Case { dir }
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.dir.join(name);
        fs::write(&path, content).expect("write fixture");
        path
    }

    /// Run both implementations with the same arguments (paths relative to
    /// the case dir so messages carry identical basenames).
    fn compare(&self, args: &[&str]) {
        let python = Command::new("python3")
            .arg(PYTHON_REFERENCE)
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("run python reference");
        let rust = Command::new(env!("CARGO_BIN_EXE_moose-csvdiff"))
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("run rust port");

        assert_eq!(
            rust.status.code(),
            python.status.code(),
            "exit codes differ for {args:?}\n--- rust stdout ---\n{}\n--- python stdout ---\n{}",
            String::from_utf8_lossy(&rust.stdout),
            String::from_utf8_lossy(&python.stdout)
        );
        assert_eq!(
            rust.stdout,
            python.stdout,
            "stdout differs for {args:?}\n--- rust ---\n{}\n--- python ---\n{}",
            String::from_utf8_lossy(&rust.stdout),
            String::from_utf8_lossy(&python.stdout)
        );
    }
}

fn reference_available() -> bool {
    Path::new(PYTHON_REFERENCE).is_file()
        && Command::new("python3")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

fn with_case(name: &str, f: impl FnOnce(&Case)) {
    if !reference_available() {
        eprintln!("python reference unavailable; skipping {name}");
        return;
    }
    let case = Case::new(name);
    f(&case);
    fs::remove_dir_all(&case.dir).ok();
}

#[test]
fn identical_files() {
    with_case("identical_files", |case| {
        case.write("a.csv", "time,u,v\n0,1,2\n1,3,4\n");
        case.write("b.csv", "time,u,v\n0,1,2\n1,3,4\n");
        case.compare(&["a.csv", "b.csv"]);
    });
}

#[test]
fn value_diff_with_time() {
    with_case("value_diff_with_time", |case| {
        case.write("a.csv", "time,u\n0,1.0\n1,2.0\n");
        case.write("b.csv", "time,u\n0,1.0\n1,2.001\n");
        case.compare(&["a.csv", "b.csv"]);
    });
}

#[test]
fn extra_column_in_second_file() {
    with_case("extra_column_in_second_file", |case| {
        case.write("a.csv", "time,u\n0,1\n");
        case.write("b.csv", "time,u,w\n0,1,2\n");
        case.compare(&["a.csv", "b.csv"]);
    });
}

#[test]
fn extra_column_in_first_file() {
    with_case("extra_column_in_first_file", |case| {
        case.write("a.csv", "time,u,w\n0,1,2\n");
        case.write("b.csv", "time,u\n0,1\n");
        case.compare(&["a.csv", "b.csv"]);
    });
}

#[test]
fn same_count_but_missing_column() {
    with_case("same_count_but_missing_column", |case| {
        // Equal column counts skip the missing-header check in the reference;
        // the value loop then reports against table1's keys.
        case.write("a.csv", "time,u,w\n0,1,2\n");
        case.write("b.csv", "time,u,x\n0,1,2\n");
        case.compare(&["a.csv", "b.csv"]);
    });
}

#[test]
fn column_length_mismatch() {
    with_case("column_length_mismatch", |case| {
        case.write("a.csv", "time,u\n0,1\n1,2\n");
        case.write("b.csv", "time,u\n0,1\n");
        case.compare(&["a.csv", "b.csv"]);
    });
}

#[test]
fn abs_zero_floors_small_values() {
    with_case("abs_zero_floors_small_values", |case| {
        case.write("a.csv", "time,u\n0,1e-12\n");
        case.write("b.csv", "time,u\n0,2e-12\n");
        case.compare(&["--abs-zero", "1e-10", "a.csv", "b.csv"]);
    });
}

#[test]
fn nan_is_rejected() {
    with_case("nan_is_rejected", |case| {
        case.write("a.csv", "time,u\n0,nan\n");
        case.write("b.csv", "time,u\n0,1\n");
        case.compare(&["a.csv", "b.csv"]);
    });
}

#[test]
fn inf_is_rejected() {
    with_case("inf_is_rejected", |case| {
        case.write("a.csv", "time,u\n0,inf\n");
        case.write("b.csv", "time,u\n0,1\n");
        case.compare(&["a.csv", "b.csv"]);
    });
}

#[test]
fn ignore_fields() {
    with_case("ignore_fields", |case| {
        case.write("a.csv", "time,u,w\n0,1,2\n");
        case.write("b.csv", "time,u,w\n0,1,99\n");
        case.compare(&["-i", "w", "a.csv", "b.csv"]);
    });
}

#[test]
fn custom_column_triple() {
    with_case("custom_column_triple", |case| {
        case.write("a.csv", "time,u,w\n0,1,2\n1,3,4\n");
        case.write("b.csv", "time,u,w\n0,1,2\n1,3,4.1\n");
        case.compare(&[
            "--custom-columns",
            "w",
            "--custom-abs-zero",
            "1e-10",
            "--custom-rel-err",
            "1e-1",
            "a.csv",
            "b.csv",
        ]);
    });
}

#[test]
fn custom_column_mismatch_reports() {
    with_case("custom_column_mismatch_reports", |case| {
        case.write("a.csv", "time,u\n0,1\n");
        case.write("b.csv", "time,u\n0,1.5\n");
        case.compare(&[
            "--custom-columns",
            "u",
            "--custom-abs-zero",
            "1e-10",
            "--custom-rel-err",
            "1e-6",
            "a.csv",
            "b.csv",
        ]);
    });
}

#[test]
fn comparison_file() {
    with_case("comparison_file", |case| {
        case.write("a.csv", "time,pressure,velocity\n0,1.0,2.0\n1,2.0,3.0\n");
        case.write("b.csv", "time,pressure,velocity\n0,1.0,2.0\n1,2.0,3.3\n");
        case.write(
            "t.cmp",
            "GLOBAL VARIABLES\nrelative 1e-4 floor 1e-9\n\n    pressure\n    velocity\n",
        );
        case.compare(&["-c", "t.cmp", "a.csv", "b.csv"]);
    });
}

#[test]
fn comparison_file_with_field_tolerances() {
    with_case("comparison_file_with_field_tolerances", |case| {
        case.write("a.csv", "time,pressure,velocity\n0,1.0,2.0\n1,2.0,3.0\n");
        case.write("b.csv", "time,pressure,velocity\n0,1.0,2.0\n1,2.0,3.05\n");
        case.write(
        "t.cmp",
        "GLOBAL VARIABLES\nrelative 1e-4 floor 1e-9\n\n    pressure\n    velocity\n        relative 1e-1\n",
    );
        case.compare(&["-c", "t.cmp", "a.csv", "b.csv"]);
    });
}

#[test]
fn summary_mode() {
    with_case("summary_mode", |case| {
        case.write("a.csv", "time,u,v\n0,1.0,2.22\n1,3.3,4.0\n");
        case.compare(&["-s", "a.csv"]);
    });
}

#[test]
fn summary_without_time_column() {
    with_case("summary_without_time_column", |case| {
        case.write("a.csv", "name1,name2,name3\n1,2.22,3.3\n");
        case.compare(&["-s", "a.csv"]);
    });
}

#[test]
fn non_numeric_cells() {
    with_case("non_numeric_cells", |case| {
        case.write("a.csv", "time,u\n0,abc\n1,2\n");
        case.write("b.csv", "time,u\n0,abc\n1,2\n");
        case.compare(&["a.csv", "b.csv"]);
    });
}

#[test]
fn row_column_count_mismatch() {
    with_case("row_column_count_mismatch", |case| {
        case.write("a.csv", "time,u\n0,1,2\n1,2\n");
        case.write("b.csv", "time,u\n0,1\n1,2\n");
        case.compare(&["a.csv", "b.csv"]);
    });
}

#[test]
fn blank_lines_are_ignored() {
    with_case("blank_lines_are_ignored", |case| {
        case.write("a.csv", "time,u\n\n0,1\n\n\n1,2\n");
        case.write("b.csv", "time,u\n0,1\n1,2\n");
        case.compare(&["a.csv", "b.csv"]);
    });
}

#[test]
fn verify_args_failures() {
    with_case("verify_args_failures", |case| {
        case.write("a.csv", "time,u\n0,1\n");
        case.write("b.csv", "time,u\n0,1\n");
        // No input files
        case.compare(&[]);
        // One file only
        case.compare(&["a.csv"]);
        // Diff and summary together
        case.compare(&["-s", "a.csv", "b.csv"]);
        // Partial custom triple
        case.compare(&["--custom-columns", "u", "a.csv", "b.csv"]);
        // Mismatched custom lengths
        case.compare(&[
            "--custom-columns",
            "u",
            "w",
            "--custom-abs-zero",
            "1e-10",
            "--custom-rel-err",
            "1e-6",
            "1e-6",
            "a.csv",
            "b.csv",
        ]);
        // Custom plus comparison file
        case.write("t.cmp", "GLOBAL VARIABLES\n\n    u\n");
        case.compare(&[
            "-c",
            "t.cmp",
            "--custom-columns",
            "u",
            "--custom-abs-zero",
            "1e-10",
            "--custom-rel-err",
            "1e-6",
            "a.csv",
            "b.csv",
        ]);
    });
}

#[test]
fn summary_with_comparison_file_ignored_notice() {
    with_case("summary_with_comparison_file_ignored_notice", |case| {
        case.write("a.csv", "time,u\n0,1\n");
        case.write("t.cmp", "GLOBAL VARIABLES\n\n    u\n");
        case.compare(&["-s", "a.csv", "-c", "t.cmp"]);
    });
}
