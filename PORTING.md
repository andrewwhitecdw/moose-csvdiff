# Porting notes

Rust port of `python/mooseutils/csvdiff.py` from the MOOSE framework
(idaholab/moose, LGPL-2.1). The goal is byte-identical stdout and identical
exit codes on the inputs the tool actually receives.

## API mapping

| Reference (`csvdiff.py`)                 | This crate                          |
|------------------------------------------|-------------------------------------|
| `CSVTools.convertToTable`                | `table::convert_to_table`           |
| `CSVTools.parseComparisonFile`           | `config::parse_comparison_file`     |
| `CSVTools.addError` / `addMessage`       | `differ::push_error` / message Vec  |
| `CSVDiffer.diff`                         | `differ::diff`                      |
| `CSVSummary.summary`                     | `summary::summary`                  |
| `parseArgs` / `verifyArgs`               | `cli::Args` / `cli::verify_args`    |
| `__main__` block                         | `src/main.rs`                       |
| Python `str(float)`, `%.3e`, `%.6g`, `%d`, `repr()` | `pyfmt` helpers        |

## Test mapping

`python/mooseutils/tests/test_csvdiff.py::testSummary` is covered by
`summary::tests::matches_reference_summary_layout`, using the expected
summary text quoted verbatim from the Python test.

Everything else is covered by differential tests
(`tests/differential.rs`): 21 scenarios run both implementations on the
same fixtures and require byte-identical stdout and equal exit codes.
They skip automatically when the Python reference is unavailable (CI).

## Preserved quirks

These behaviors come from the reference and are intentionally kept:

- A comparison file **resets** the global tolerances to the file's values
  (or 0.0 when absent), because `parseComparisonFile` seeds both with 0.0.
- Header checks: equal column counts report **no** missing headers; the
  iteration key set is replaced by the custom column list when a config
  file is in use.
- `-f/--diff-fields` is parsed and never used.
- Non-numeric cells become `0` without complaint; row shape mismatches
  report once and stop the table build.
- Missing headers are only reported when the column *counts* differ.
- When a config file names no fields at all, the reference compares with
  string tolerances and crashes with `TypeError`; this port parses the
  numbers and proceeds (see below).

## Deliberate differences

- **Exit codes on internal crashes.** The reference dies with an uncaught
  exception (exit 1, traceback on stderr): `KeyError` when the value loop
  meets a column absent from one table (equal-count quirk above),
  `TypeError` for a fieldless config file, `ValueError` for unparseable
  `--custom-*` floats, and `ValueError` from `min()` on empty columns in
  summary mode. The port exits 1 with a short stderr message instead.
  stdout stays empty in both.
- **Config tolerances are parsed eagerly** as floats. The reference keeps
  them as strings and only survives because the per-column maps call
  `float()` later; a config file with no field sections crashes it.
  The port accepts the same files and works.
- **Usage/error text for argument parsing** comes from `clap`, not
  argparse; exit code 2 matches.
- `os.path.basename` on trailing-slash paths is not replicated.
- Python `repr()` of exotic control characters in row text (beyond
  `\n \r \t`, quotes, and `\xNN` for other control bytes) may differ.

## Verification

- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`,
  `cargo doc --no-deps` — all clean (42 tests).
- Differential suite run against the Python reference on this machine:
  21/21 byte-identical stdout and exit codes.
