//! Port of `CSVTools.parseComparisonFile`: the exodiff-style comparison
//! configuration accepted via `-c/--comparison-file`.

/// Per-field tolerance overrides.
#[derive(Debug, Clone, Default)]
pub struct FieldTol {
    /// Relative tolerance (`relative` / `rel_tol`).
    pub relative: Option<f64>,
    /// Absolute-zero floor (`floor` / `abs_zero` / `absolute`).
    pub zero: Option<f64>,
}

/// Parsed comparison configuration.
#[derive(Debug, Clone)]
pub struct CustomParams {
    /// Global relative tolerance.
    pub relative: f64,
    /// Global absolute-zero floor.
    pub zero: f64,
    /// Per-field overrides in file order.
    pub fields: Vec<(String, FieldTol)>,
}

const ZERO_PARAMS: [&str; 3] = ["floor", "abs_zero", "absolute"];
const TOLERANCE_PARAMS: [&str; 2] = ["relative", "rel_tol"];

/// Parse the comparison file text, appending errors in the reference's
/// wording (prefixed with `In file <display>:`).
pub fn parse_comparison_file(display: &str, text: &str, errors: &mut Vec<String>) -> CustomParams {
    let mut params = CustomParams {
        // The reference seeds both globals with 0.0, which means supplying
        // a config file resets tolerances unless the file sets them.
        relative: 0.0,
        zero: 0.0,
        fields: Vec::new(),
    };

    for a_line in text.lines() {
        let s_line = a_line.trim();
        let words: Vec<&str> = a_line.split_whitespace().collect();
        if !s_line.is_empty()
            && (s_line.starts_with('#')
                || s_line.to_lowercase().starts_with("time steps")
                || s_line.starts_with('!'))
        {
            continue;
        }

        // r"^\s+(\S+)": an indented line names the field it configures.
        let field_key = a_line
            .starts_with([' ', '\t'])
            .then(|| words.first().copied())
            .flatten();

        let global = !a_line.starts_with([' ', '\t']);
        let first_intersection = |set: &[&str]| words.iter().copied().find(|w| set.contains(w));

        let value_of = |word: &str| -> Result<f64, ()> {
            match get_param_values(word, a_line).first() {
                Some(v) => v.parse::<f64>().map_err(|_| ()),
                None => Err(()),
            }
        };

        let mut failed = false;
        if global {
            if let Some(word) = first_intersection(&ZERO_PARAMS) {
                match value_of(word) {
                    Ok(v) => params.zero = v,
                    Err(()) => failed = true,
                }
            }
            if !failed {
                if let Some(word) = first_intersection(&TOLERANCE_PARAMS) {
                    match value_of(word) {
                        Ok(v) => params.relative = v,
                        Err(()) => failed = true,
                    }
                }
            }
        } else if let Some(field) = field_key {
            let entry = match params.fields.iter().position(|(f, _)| f == field) {
                Some(i) => i,
                None => {
                    params.fields.push((field.to_string(), FieldTol::default()));
                    params.fields.len() - 1
                }
            };
            if let Some(word) = first_intersection(&ZERO_PARAMS) {
                match value_of(word) {
                    Ok(v) => params.fields[entry].1.zero = Some(v),
                    Err(()) => failed = true,
                }
            }
            if !failed {
                if let Some(word) = first_intersection(&TOLERANCE_PARAMS) {
                    match value_of(word) {
                        Ok(v) => params.fields[entry].1.relative = Some(v),
                        Err(()) => failed = true,
                    }
                }
            }
        }

        if failed {
            // The reference catches only IndexError from `[0]` on an empty
            // findall result; an unparseable float crashes it instead.
            errors.push(format!(
                "In file {display}: Error parsing comparison file on line: \n{a_line}"
            ));
        }
    }
    params
}

/// Port of `getParamValues`: `re.findall(param + r"\s+([0-9e.\-\+]+)", line)`.
fn get_param_values(param: &str, line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while let Some(rel) = line[i..].find(param) {
        let abs = i + rel + param.len();
        let mut j = abs;
        while j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\r' | b'\n') {
            j += 1;
        }
        if j == abs {
            i = abs;
            continue;
        }
        let start = j;
        while j < bytes.len() && matches!(bytes[j], b'0'..=b'9' | b'e' | b'E' | b'.' | b'-' | b'+')
        {
            j += 1;
        }
        if j > start {
            out.push(line[start..j].to_string());
            i = j;
        } else {
            i = abs;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_global_and_field_tolerances() {
        // Field names appear on their own (indented or bare) lines, with
        // tolerance lines indented beneath them — the layout the reference
        // summary emits. Verified against csvdiff.py.
        let text = "GLOBAL VARIABLES
relative 1e-4 floor 1e-9

    pressure    # min: 0 @ t0
    velocity
";
        let mut errors = Vec::new();
        let p = parse_comparison_file("cmp", text, &mut errors);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(p.relative, 1e-4);
        assert_eq!(p.zero, 1e-9);
        assert_eq!(p.fields.len(), 2);
        let (name, tol) = &p.fields[0];
        assert_eq!(name, "pressure");
        assert_eq!(tol.zero, None);
        assert_eq!(tol.relative, None);
        assert_eq!(p.fields[1].0, "velocity");
    }

    #[test]
    fn comments_and_time_steps_header_are_skipped() {
        let text = "# comment\nTIME STEPS relative 1 floor 0\n! also skipped\nrelative 1e-2\n";
        let mut errors = Vec::new();
        let p = parse_comparison_file("cmp", text, &mut errors);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(p.relative, 1e-2);
        assert_eq!(p.zero, 0.0);
        assert!(p.fields.is_empty());
    }

    #[test]
    fn missing_value_reports_line_error() {
        let text = "relative\n";
        let mut errors = Vec::new();
        parse_comparison_file("cmp", text, &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("Error parsing comparison file on line: \nrelative"),
            "{errors:?}"
        );
    }

    #[test]
    fn zero_value_gets_parsed_even_for_fixed_zero_params() {
        // 'absolute' is both a global param name and a field param name;
        // ensure the first word wins on a global line.
        let text = "absolute 1e-5\n";
        let mut errors = Vec::new();
        let p = parse_comparison_file("cmp", text, &mut errors);
        assert_eq!(p.zero, 1e-5);
    }
}
