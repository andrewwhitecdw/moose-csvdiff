//! Port of `CSVSummary.summary`: emit a comparison-file summary in
//! exodiff input format from a single CSV.

use crate::pyfmt::{py_e, py_float_repr};
use crate::table::{Table, convert_to_table};

/// Absolute-zero floor and relative tolerance used in the header lines.
#[derive(Debug, Clone)]
pub struct SummaryOptions {
    /// Absolute-zero floor (default `1e-10`).
    pub abs_zero: f64,
    /// Relative tolerance (default `5.5e-6`).
    pub rel_tol: f64,
}

impl Default for SummaryOptions {
    fn default() -> Self {
        SummaryOptions {
            abs_zero: 1e-10,
            rel_tol: 5.5e-6,
        }
    }
}

/// Summarize one CSV text. Returns the message list (a single joined
/// block, like the reference) and the error count.
///
/// Errors from table conversion end the run exactly as the reference's
/// early `return self.getMessages()` does.
pub fn summary(display: &str, text: &str, opts: &SummaryOptions) -> (Vec<String>, usize) {
    let mut errors = Vec::new();
    let table = convert_to_table(display, text, &mut errors);
    if !errors.is_empty() {
        let n = errors.len();
        return (errors, n);
    }

    let mut formatted: Vec<String> = vec![format!(
        "GLOBAL VARIABLES relative {} floor {}",
        py_float_repr(opts.rel_tol),
        py_float_repr(opts.abs_zero)
    )];

    // First pass: justification widths, mirroring the reference's loops.
    let mut field_len = 0usize;
    let mut value_len = 0usize;
    for (field, value) in cols_iter(&table) {
        field_len = field_len.max(field.chars().count());
        let (min_v, min_i) = min_index(value).unwrap_or((f64::NAN, 0usize));
        value_len = value_len.max(min_part(min_v, min_i).chars().count());
    }
    let field_justify = field_len + 10;
    let value_justify = value_len + 5;

    // A "TIME STEPS" line is synthesized when no `time` column exists.
    let has_time = table.keys().iter().any(|k| k.to_lowercase() == "time");
    if !has_time {
        let first_len = table
            .keys()
            .first()
            .and_then(|k| table.get(k))
            .map_or(0, |v| v.len());
        let value_count = first_len as i64 - 1;
        formatted.insert(
            0,
            format!("TIME STEPS relative 1 floor 0  # min: 0 @ t0  max: {value_count} @ t{value_count}\n"),
        );
    }

    for (field, value) in cols_iter(&table) {
        if field.to_lowercase() == "time" {
            let (min_v, min_i) = match min_index(value) {
                Some(v) => v,
                None => return (vec!["min() arg is an empty sequence".to_string()], 1),
            };
            let (max_v, max_i) = max_index(value).expect("same length as min");
            formatted.insert(
                0,
                format!(
                    "TIME STEPS relative {} floor {}  # min: {} @ t{min_i}  max: {} @ t{max_i}\n",
                    py_float_repr(opts.rel_tol),
                    py_float_repr(opts.abs_zero),
                    py_d(min_v),
                    py_d(max_v)
                ),
            );
        }
        let (min_v, min_i) = match min_index(value) {
            Some(v) => v,
            None => return (vec!["min() arg is an empty sequence".to_string()], 1),
        };
        let (max_v, max_i) = max_index(value).expect("same length as min");
        let min_text = min_part(min_v, min_i);
        formatted.push(format!(
            "    {field}{}{min_text}{}max: {} @ t{max_i}",
            " ".repeat((field_justify - field.chars().count()) + 10),
            " ".repeat((value_justify - min_text.chars().count()) + 5),
            py_e(max_v, 3),
        ));
    }

    (vec![formatted.join("\n")], 0)
}

fn cols_iter(table: &Table) -> Vec<(String, &[f64])> {
    table
        .keys()
        .into_iter()
        .map(|k| (k.to_string(), table.get(k).expect("key from keys")))
        .collect()
}

fn min_index(values: &[f64]) -> Option<(f64, usize)> {
    let mut best: Option<(f64, usize)> = None;
    for (i, &v) in values.iter().enumerate() {
        // Mirrors Python's min(): v replaces the running best only when
        // `v < best`, so NaN never replaces but a leading NaN wins.
        let replace = match best {
            Some((b, _)) => matches!(v.partial_cmp(&b), Some(std::cmp::Ordering::Less)),
            None => true,
        };
        if replace {
            best = Some((v, i));
        }
    }
    best
}

fn max_index(values: &[f64]) -> Option<(f64, usize)> {
    let mut best: Option<(f64, usize)> = None;
    for (i, &v) in values.iter().enumerate() {
        // Mirrors Python's max(): v replaces the running best only when
        // `v > best`, so NaN never replaces but a leading NaN wins.
        let replace = match best {
            Some((b, _)) => matches!(v.partial_cmp(&b), Some(std::cmp::Ordering::Greater)),
            None => true,
        };
        if replace {
            best = Some((v, i));
        }
    }
    best
}

fn min_part(min_v: f64, min_i: usize) -> String {
    format!("# min: {} @ t{min_i}", py_e(min_v, 3))
}

/// `"%d" % x` for the float values the reference formats this way:
/// truncation toward zero, with a fallback for magnitudes beyond i64.
fn py_d(x: f64) -> String {
    if x.is_finite() && x.abs() < 9.2e18 {
        (x as i64).to_string()
    } else if x.is_finite() {
        format!("{x:.0}")
    } else {
        // Python raises TypeError formatting inf/nan with %d.
        "0".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_reference_summary_layout() {
        // The layout string is taken verbatim from
        // python/mooseutils/tests/test_csvdiff.py::testSummary.
        let text = "name1,name2,name3\n1,2.22,3.3\n";
        let (messages, errors) = summary("f.csv", text, &SummaryOptions::default());
        assert_eq!(errors, 0);
        assert_eq!(messages.len(), 1);
        assert!(
            messages[0].contains(
                "TIME STEPS relative 1 floor 0  # min: 0 @ t0  max: 0 @ t0\n\nGLOBAL VARIABLES relative 5.5e-06 floor 1e-10"
            ),
            "{}",
            messages[0]
        );
        assert!(
            messages[0].contains("# min: 1.000e+00 @ t0"),
            "{}",
            messages[0]
        );
        assert!(
            messages[0].contains("max: 2.220e+00 @ t0"),
            "{}",
            messages[0]
        );
    }

    #[test]
    fn time_column_produces_time_steps_line() {
        let text = "time,u\n0,1\n0.5,2\n";
        let (messages, errors) = summary("f.csv", text, &SummaryOptions::default());
        assert_eq!(errors, 0);
        assert!(
            messages[0].starts_with("TIME STEPS relative 5.5e-06 floor 1e-10"),
            "{}",
            messages[0]
        );
        assert!(messages[0].contains("# min: 0 @ t0"), "{}", messages[0]);
        assert!(messages[0].contains("max: 0 @ t1"), "{}", messages[0]);
    }
}
