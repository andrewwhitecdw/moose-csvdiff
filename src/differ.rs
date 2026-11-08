//! Port of `CSVDiffer.diff`.

use std::collections::{HashMap, HashSet};

use crate::config::{self, CustomParams};
use crate::pyfmt::{py_e, py_g};
use crate::table::convert_to_table;

/// Everything the differ needs that came from the command line.
#[derive(Debug, Clone, Default)]
pub struct DiffOptions {
    /// Absolute-zero floor (default `1e-10` like the reference).
    pub abs_zero: f64,
    /// Relative tolerance (default `5.5e-6` like the reference).
    pub rel_tol: f64,
    /// `--custom-columns` triple.
    pub custom_columns: Option<Vec<String>>,
    /// `--custom-abs-zero` triple.
    pub custom_abs_zero: Option<Vec<String>>,
    /// `--custom-rel-err` triple.
    pub custom_rel_err: Option<Vec<String>>,
    /// `--ignore-fields`.
    pub ignore: Option<Vec<String>>,
    /// Contents and display name of the `--comparison-file`, if any.
    pub comparison: Option<(String, String)>,
}

/// Outcome of a diff: accumulated messages and the error count.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// Every message, in the order the reference produces them.
    pub messages: Vec<String>,
    /// Number of errors (messages added through `addError`).
    pub errors: usize,
}

/// Diff two CSV texts. `displays` are the basenames used in messages.
///
/// `files` order matches the reference: `[files[0], files[1]]`.
pub fn diff(displays: [&str; 2], texts: [&str; 2], opts: &DiffOptions) -> Result<Outcome, String> {
    let mut messages: Vec<String> = Vec::new();
    let mut errors = 0usize;

    let mut abs_zero = opts.abs_zero;
    let mut rel_tol = opts.rel_tol;
    let mut custom_columns: Vec<String> = Vec::new();
    let mut custom_abs_zero: Vec<f64> = Vec::new();
    let mut custom_rel_err: Vec<f64> = Vec::new();
    let mut only_compare_custom = false;

    if let Some((text, display)) = &opts.comparison {
        only_compare_custom = true;
        let mut config_messages = Vec::new();
        let custom: CustomParams =
            config::parse_comparison_file(display, text, &mut config_messages);
        errors += config_messages.len();
        messages.append(&mut config_messages);
        if errors > 0 {
            return Ok(Outcome { messages, errors });
        }
        abs_zero = custom.zero;
        rel_tol = custom.relative;
        for (name, tol) in custom.fields {
            custom_abs_zero.push(tol.zero.unwrap_or(abs_zero));
            custom_rel_err.push(tol.relative.unwrap_or(rel_tol));
            custom_columns.push(name);
        }
    } else if let (Some(cols), Some(zeros), Some(rels)) = (
        &opts.custom_columns,
        &opts.custom_abs_zero,
        &opts.custom_rel_err,
    ) {
        custom_columns = cols.clone();
        custom_abs_zero = zeros
            .iter()
            .map(|z| z.parse::<f64>().map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        custom_rel_err = rels
            .iter()
            .map(|r| r.parse::<f64>().map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
    }

    let ignore: HashSet<&str> = opts.ignore.iter().flatten().map(String::as_str).collect();

    let mut rel_err_map: HashMap<&str, f64> = HashMap::new();
    let mut abs_zero_map: HashMap<&str, f64> = HashMap::new();
    let mut found_column: HashMap<&str, bool> = HashMap::new();
    if !custom_columns.is_empty() {
        for (i, col) in custom_columns.iter().enumerate() {
            rel_err_map.insert(col, custom_rel_err[i]);
            abs_zero_map.insert(col, custom_abs_zero[i]);
            found_column.insert(col, false);
        }
    }

    let mut table_errors = Vec::new();
    let table1 = convert_to_table(displays[0], texts[0], &mut table_errors);
    let mut table_errors2 = Vec::new();
    let table2 = convert_to_table(displays[1], texts[1], &mut table_errors2);
    errors += table_errors.len() + table_errors2.len();
    messages.append(&mut table_errors);
    messages.append(&mut table_errors2);
    if errors > 0 {
        return Ok(Outcome { messages, errors });
    }

    // Header comparison. The branches and their quirks mirror the
    // reference: with a config file the custom list becomes the iteration
    // set; unequal counts report on `large`; equal counts report nothing
    // and mark every column found.
    let mut keys1: Vec<&str> = table1.keys();
    {
        if only_compare_custom && !custom_columns.is_empty() {
            keys1 = custom_columns.iter().map(String::as_str).collect();
            for key in &custom_columns {
                if table2.get(key).is_none() || table1.get(key).is_none() {
                    push_error(
                        &mut messages,
                        &mut errors,
                        displays[0],
                        format!("Header '{key}' is missing"),
                    );
                }
            }
        } else if table1.len() < table2.len() {
            for key in table2.keys() {
                if table1.get(key).is_none() && !ignore.contains(key) {
                    push_error(
                        &mut messages,
                        &mut errors,
                        displays[0],
                        format!("Header '{key}' is missing"),
                    );
                }
            }
        } else if table1.len() > table2.len() {
            for key in table1.keys() {
                if table2.get(key).is_none() && !ignore.contains(key) {
                    push_error(
                        &mut messages,
                        &mut errors,
                        displays[1],
                        format!("Header '{key}' is missing"),
                    );
                }
            }
        } else {
            for key in &keys1 {
                found_column.insert(key, true);
            }
        }
    }
    if errors > 0 {
        return Ok(Outcome { messages, errors });
    }

    // Column length check: first mismatch only, like the reference.
    for key in &keys1 {
        if ignore.contains(key) {
            continue;
        }
        let (a, b) = match (table1.get(key), table2.get(key)) {
            (Some(a), Some(b)) => (a, b),
            // The reference raises an uncaught KeyError here (exit 1).
            _ => return Err(format!("KeyError: '{key}'")),
        };
        if a.len() != b.len() {
            push_error(
                &mut messages,
                &mut errors,
                displays[0],
                format!("Columns with header '{key}' aren't the same length"),
            );
            break;
        }
    }
    if errors > 0 {
        return Ok(Outcome { messages, errors });
    }

    let time_col: Option<&[f64]> = table1.get("time").or_else(|| table1.get("Time"));

    for key in &keys1 {
        if ignore.contains(key) {
            continue;
        }
        let (col1, col2) = match (table1.get(key), table2.get(key)) {
            (Some(a), Some(b)) => (a, b),
            _ => return Err(format!("KeyError: '{key}'")),
        };
        for (row, (&v1, &v2)) in col1.iter().zip(col2.iter()).enumerate() {
            let mut abs_zero = abs_zero;
            if !custom_columns.is_empty() {
                if let Some(v) = abs_zero_map.get(key) {
                    abs_zero = *v;
                }
            }
            let val1 = if v1.abs() < abs_zero { 0.0 } else { v1 };
            let val2 = if v2.abs() < abs_zero { 0.0 } else { v2 };

            let before = errors;
            for (file, v) in [(displays[0], val1), (displays[1], val2)] {
                if !v.is_finite() {
                    push_error(
                        &mut messages,
                        &mut errors,
                        file,
                        format!("Column \"{}\" has NaN/Inf value(s)", key.trim()),
                    );
                }
            }
            if errors != before {
                break;
            }

            if val1 == 0.0 && val2 == 0.0 {
                continue;
            }

            let mut rel_diff = 0.0f64;
            let max_abs = val1.abs().max(val2.abs());
            if max_abs > 0.0 {
                rel_diff = ((val1 - val2) / max_abs).abs();
            }

            let mut rel_tol = rel_tol;
            if !custom_columns.is_empty() {
                if let Some(v) = rel_err_map.get(key) {
                    rel_tol = *v;
                }
            }

            if rel_diff > rel_tol {
                let time_info = match time_col {
                    Some(t) if row < t.len() => {
                        format!(" at row {row} (time = {})", py_g(t[row], 6))
                    }
                    _ => format!(" at row {row}"),
                };
                push_error(
                    &mut messages,
                    &mut errors,
                    displays[1],
                    format!(
                        "The values in column \"{}\" don't match{time_info}. \n\trelative diff:   {} ~ {} = {} ({})",
                        key.trim(),
                        py_e(val1, 3),
                        py_e(val2, 3),
                        py_e(rel_diff, 3),
                        py_e(rel_diff, 3)
                    ),
                );
                break;
            }
        }
    }

    if !custom_columns.is_empty() && !only_compare_custom {
        for col in &custom_columns {
            if found_column.get(col.as_str()) == Some(&false) {
                push_error(
                    &mut messages,
                    &mut errors,
                    displays[0],
                    format!("all CSV files Variable '{col}' in custom_columns is missing"),
                );
            }
        }
    }

    Ok(Outcome { messages, errors })
}

fn push_error(messages: &mut Vec<String>, errors: &mut usize, file: &str, msg: String) {
    messages.push(format!("In file {file}: {msg}"));
    *errors += 1;
}
