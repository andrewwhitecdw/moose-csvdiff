//! Port of the `argparse`/`verifyArgs` command line.

use std::path::PathBuf;

/// Parsed command-line arguments, mirroring the reference's `args`.
#[derive(Debug, Clone, Default)]
pub struct Args {
    /// Positional CSV files.
    pub csv_files: Vec<PathBuf>,
    /// `-s/--summary` file.
    pub summary: Option<PathBuf>,
    /// `-c/--comparison-file`.
    pub comparison_file: Option<PathBuf>,
    /// `-i/--ignore-fields`.
    pub ignore_fields: Option<Vec<String>>,
    /// `-f/--diff-fields`. Parsed by the reference but never read; kept for
    /// flag compatibility (see PORTING.md).
    pub diff_fields: Option<Vec<String>>,
    /// `--abs-zero` (string, parsed as float like the reference does).
    pub abs_zero: String,
    /// `--relative-tolerance` (string, parsed as float).
    pub relative_tolerance: String,
    /// `--custom-columns`.
    pub custom_columns: Option<Vec<String>>,
    /// `--custom-abs-zero`.
    pub custom_abs_zero: Option<Vec<String>>,
    /// `--custom-rel-err`.
    pub custom_rel_err: Option<Vec<String>>,
}

/// Problems reported by `verifyArgs`, in the reference's wording. The
/// caller prints each to stdout and exits 1 when the list is non-empty.
pub fn verify_args(args: &Args) -> Vec<String> {
    let mut problems = Vec::new();
    if args.csv_files.is_empty() && args.summary.is_none() {
        problems.push("No input files given".to_string());
    } else if args.csv_files.len() != 2 && args.summary.is_none() {
        problems.push("Specify two files to compare".to_string());
    } else if !args.csv_files.is_empty() && args.summary.is_some() {
        problems.push(
            "Incorrect positional arguments, or you are trying to perform a diff and show a summary (can only do one or the other)"
                .to_string(),
        );
    } else if args.summary.is_some() && args.comparison_file.is_some() {
        println!("Ignoring request to use config file while being asked to display a summary\n");
    }

    let unify = [
        &args.custom_columns,
        &args.custom_abs_zero,
        &args.custom_rel_err,
    ]
    .iter()
    .filter(|o| o.is_some())
    .count();
    if unify > 0 && unify != 3 {
        problems.push("When using any --custom-* option, you must use all three".to_string());
    } else if unify == 3 {
        let lens: Vec<usize> = [
            &args.custom_columns,
            &args.custom_abs_zero,
            &args.custom_rel_err,
        ]
        .iter()
        .map(|o| o.as_ref().map_or(0, |v| v.len()))
        .collect();
        if lens.iter().collect::<std::collections::HashSet<_>>().len() > 1 {
            problems.push(
                "All --custom-* options need to contain the same number of space separated items"
                    .to_string(),
            );
        }
    }

    if unify == 3 && args.comparison_file.is_some() {
        problems.push(
            "When supplying a config file (--comparison-file|-c), you can not use any --custom-* args"
                .to_string(),
        );
    }

    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Args {
        Args {
            abs_zero: "1e-10".to_string(),
            relative_tolerance: "5.5e-6".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn no_files_is_a_problem() {
        assert_eq!(verify_args(&base()), ["No input files given"]);
    }

    #[test]
    fn wrong_file_count_is_a_problem() {
        let mut a = base();
        a.csv_files = vec!["only.csv".into()];
        assert_eq!(verify_args(&a), ["Specify two files to compare"]);
    }

    #[test]
    fn diff_and_summary_together_is_a_problem() {
        let mut a = base();
        a.csv_files = vec!["a.csv".into(), "b.csv".into()];
        a.summary = Some("s.csv".into());
        assert_eq!(
            verify_args(&a),
            [
                "Incorrect positional arguments, or you are trying to perform a diff and show a summary (can only do one or the other)"
            ]
        );
    }

    #[test]
    fn partial_custom_triple_is_a_problem() {
        let mut a = base();
        a.csv_files = vec!["a.csv".into(), "b.csv".into()];
        a.custom_columns = Some(vec!["u".into()]);
        assert_eq!(
            verify_args(&a),
            ["When using any --custom-* option, you must use all three"]
        );
    }

    #[test]
    fn mismatched_custom_lengths_are_a_problem() {
        let mut a = base();
        a.csv_files = vec!["a.csv".into(), "b.csv".into()];
        a.custom_columns = Some(vec!["u".into(), "v".into()]);
        a.custom_abs_zero = Some(vec!["1e-10".into()]);
        a.custom_rel_err = Some(vec!["1e-5".into(), "1e-5".into()]);
        assert_eq!(
            verify_args(&a),
            ["All --custom-* options need to contain the same number of space separated items"]
        );
    }

    #[test]
    fn custom_and_comparison_file_conflict() {
        let mut a = base();
        a.csv_files = vec!["a.csv".into(), "b.csv".into()];
        a.custom_columns = Some(vec!["u".into()]);
        a.custom_abs_zero = Some(vec!["1e-10".into()]);
        a.custom_rel_err = Some(vec!["1e-5".into()]);
        a.comparison_file = Some("cmp".into());
        assert_eq!(
            verify_args(&a),
            [
                "When supplying a config file (--comparison-file|-c), you can not use any --custom-* args"
            ]
        );
    }

    #[test]
    fn two_files_are_fine() {
        let mut a = base();
        a.csv_files = vec!["a.csv".into(), "b.csv".into()];
        assert!(verify_args(&a).is_empty());
    }
}
