# moose-csvdiff

Byte-compatible Rust port of MOOSE's `csvdiff` CSV comparison tool
(`python/mooseutils/csvdiff.py`). Same flags, same messages, same exit
codes — as a single native binary with no Python startup cost.

## Why

MOOSE's TestHarness runs a CSV comparison on every `CSVDiff` test. The
Python implementation works, but each invocation pays the interpreter
startup tax, and its message formats are load-bearing: gold-file
workflows, comparison files, and log archaeology all match on the exact
text. This port keeps that text identical.

## Install

```sh
cargo install --git https://github.com/andrewwhitecdw/moose-csvdiff
```

## Usage

Identical to the Python tool:

```sh
# Compare two files (exit 1 on difference)
moose-csvdiff gold/out.csv out.csv

# Tolerances
moose-csvdiff --abs-zero 1e-9 --relative-tolerance 1e-4 a.csv b.csv

# Compare only selected columns with custom tolerances
moose-csvdiff --custom-columns u v --custom-abs-zero 1e-10 1e-9 \
    --custom-rel-err 1e-6 1e-5 a.csv b.csv

# exodiff-style comparison file
moose-csvdiff -c tolerances.cmp a.csv b.csv

# Emit a comparison file from a gold CSV
moose-csvdiff -s gold/out.csv > tolerances.cmp

# Ignore fields
moose-csvdiff -i time aux_a a.csv b.csv
```

## Verification

21 differential tests run this port and the Python reference on the same
fixtures and require byte-identical stdout and exit codes (see
[PORTING.md](PORTING.md) for quirks and deliberate differences).

## Relationship to MOOSE

MOOSE is developed by Idaho National Laboratory and licensed LGPL-2.1.
This is an independent MIT-licensed reimplementation of one of MOOSE's
utility scripts; it contains no MOOSE code.

## License

MIT
