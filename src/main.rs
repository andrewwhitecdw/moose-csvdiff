//! Command-line entry point for `moose-csvdiff`, a byte-compatible Rust
//! port of MOOSE's `csvdiff.py`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use moose_csvdiff::cli::{self, Args};
use moose_csvdiff::differ::{self, DiffOptions};
use moose_csvdiff::summary::{self, SummaryOptions};

#[derive(Parser)]
#[command(
    name = "moose-csvdiff",
    about = "Tool for testing differences between two CSV files (Rust port of MOOSE's csvdiff)",
    version,
    disable_help_subcommand = true
)]
struct Cli {
    /// CSV files to compare
    csv_file: Vec<PathBuf>,

    /// Produce a summary in csvdiff input format
    #[arg(short, long, value_name = "csv_file")]
    summary: Option<PathBuf>,

    /// Use comparison configuration file (can be generated using --summary)
    #[arg(short, long, value_name = "comparison_file")]
    comparison_file: Option<PathBuf>,

    /// Ignore specified space-separated field IDs
    #[arg(short, long, value_name = "id", num_args = 1..)]
    ignore_fields: Option<Vec<String>>,

    /// Perform diff tests only on space-separated field IDs (accepted for
    /// compatibility; the reference parses but never uses this option)
    #[arg(short, long, value_name = "id", num_args = 1..)]
    diff_fields: Option<Vec<String>>,

    /// Value representing an absolute zero (default: 1e-10)
    #[arg(long, value_name = "absolute zero", default_value = "1e-10")]
    abs_zero: String,

    /// Value representing the acceptable tolerance between comparisons
    /// (default: 5.5e-6)
    #[arg(long, value_name = "tolerance", default_value = "5.5e-6")]
    relative_tolerance: String,

    /// Space separated list of custom field IDs to compare
    #[arg(long, value_name = "field", num_args = 1..)]
    custom_columns: Option<Vec<String>>,

    /// Space separated list of corresponding exponential absolute zero values
    #[arg(long, value_name = "exponential", num_args = 1..)]
    custom_abs_zero: Option<Vec<String>>,

    /// Space separated list of corresponding acceptable exponential tolerance values
    #[arg(long, value_name = "exponential", num_args = 1..)]
    custom_rel_err: Option<Vec<String>>,
}

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let args = Args {
        csv_files: cli.csv_file.clone(),
        summary: cli.summary.clone(),
        comparison_file: cli.comparison_file.clone(),
        ignore_fields: cli.ignore_fields.clone(),
        diff_fields: cli.diff_fields.clone(),
        abs_zero: cli.abs_zero.clone(),
        relative_tolerance: cli.relative_tolerance.clone(),
        custom_columns: cli.custom_columns.clone(),
        custom_abs_zero: cli.custom_abs_zero.clone(),
        custom_rel_err: cli.custom_rel_err.clone(),
    };

    let problems = cli::verify_args(&args);
    for problem in &problems {
        println!("{problem}");
    }
    if !problems.is_empty() {
        return Ok(ExitCode::FAILURE);
    }

    // The reference opens every file up front via argparse FileType.
    let mut texts: Vec<String> = Vec::new();
    for path in &args.csv_files {
        texts.push(read_file(path));
    }
    let summary_text = args.summary.as_ref().map(|p| read_file(p));
    let comparison = args
        .comparison_file
        .as_ref()
        .map(|p| (read_file(p), basename(p)));

    let (messages, errors) = if let Some(text) = summary_text {
        let opts = SummaryOptions {
            abs_zero: parse_float_or_exit(&args.abs_zero),
            rel_tol: parse_float_or_exit(&args.relative_tolerance),
        };
        let display = basename(args.summary.as_ref().expect("checked above"));
        summary::summary(&display, &text, &opts)
    } else {
        let texts: [&str; 2] = [texts[0].as_str(), texts[1].as_str()];
        let display0 = basename(&args.csv_files[0]);
        let display1 = basename(&args.csv_files[1]);
        let displays = [display0.as_str(), display1.as_str()];
        let opts = DiffOptions {
            abs_zero: parse_float_or_exit(&args.abs_zero),
            rel_tol: parse_float_or_exit(&args.relative_tolerance),
            custom_columns: args.custom_columns.clone(),
            custom_abs_zero: args.custom_abs_zero.clone(),
            custom_rel_err: args.custom_rel_err.clone(),
            ignore: args.ignore_fields.clone(),
            comparison,
        };
        let outcome = differ::diff(displays, texts, &opts).unwrap_or_else(|e| {
            // float() failures in the reference die with a traceback.
            eprintln!("{e}");
            std::process::exit(1);
        });
        (outcome.messages, outcome.errors)
    };

    for message in &messages {
        println!("{message}");
    }
    if errors == 0 && args.summary.is_none() {
        println!("Files are the same");
    }

    Ok(if errors > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn basename(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Read a file to string, exiting 2 like argparse's FileType on failure.
fn read_file(path: &Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("moose-csvdiff: can't open '{}': {err}", path.display());
            std::process::exit(2);
        }
    }
}

/// Parse a float like Python's float(), exiting 1 on failure (the reference
/// raises an uncaught ValueError).
fn parse_float_or_exit(s: &str) -> f64 {
    match s.trim().parse::<f64>() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("could not convert string to float: '{s}': {err}");
            std::process::exit(1);
        }
    }
}
