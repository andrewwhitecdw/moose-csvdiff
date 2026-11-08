//! Ordered-column table model and the `convertToTable` port.

use std::collections::HashMap;

use crate::pyfmt::py_repr_str;

/// A parsed CSV table with insertion-ordered columns, mirroring the
/// `dict` of lists the reference implementation builds.
#[derive(Debug, Clone, Default)]
pub struct Table {
    cols: Vec<(String, Vec<f64>)>,
    index: HashMap<String, usize>,
}

impl Table {
    /// Column names in header order.
    pub fn keys(&self) -> Vec<&str> {
        self.cols.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Values of a column.
    pub fn get(&self, key: &str) -> Option<&[f64]> {
        self.index.get(key).map(|&i| self.cols[i].1.as_slice())
    }

    /// Number of columns.
    pub fn len(&self) -> usize {
        self.cols.len()
    }

    /// Whether the table has no columns.
    pub fn is_empty(&self) -> bool {
        self.cols.is_empty()
    }

    fn reset_or_insert(&mut self, header: &str) {
        match self.index.get(header) {
            Some(&i) => self.cols[i].1.clear(),
            None => {
                self.index.insert(header.to_string(), self.cols.len());
                self.cols.push((header.to_string(), Vec::new()));
            }
        }
    }

    fn push(&mut self, header: &str, value: f64) {
        if let Some(&i) = self.index.get(header) {
            self.cols[i].1.push(value);
        }
    }
}

/// Port of `CSVTools.convertToTable`: parse one CSV text into a table.
///
/// `display_name` is the basename used in error messages. Parse and shape
/// problems are appended to `errors` in the reference's wording.
pub fn convert_to_table(display_name: &str, text: &str, errors: &mut Vec<String>) -> Table {
    // re.sub(r"\n\s*\n", "\n", text).strip(): runs of blank lines collapse
    // to nothing, then both ends are trimmed.
    let collapsed: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let text = collapsed.join("\n").trim().to_string();

    if text.is_empty() {
        // list.pop(0) on the header row raises IndexError inside the
        // reference's catch-all; the message embeds the exception args.
        errors.push(format!(
            "In file {display_name}: Exception parsing file: ('pop from empty list',)"
        ));
        return Table::default();
    }

    let mut lines = text.split('\n');
    let header_line = lines.next().expect("text is non-empty");
    let headers: Vec<String> = header_line
        .split(',')
        .map(|h| h.trim().to_string())
        .collect();

    let mut table = Table::default();
    for header in &headers {
        table.reset_or_insert(header);
    }

    for row in lines {
        let vals: Vec<&str> = row.split(',').collect();
        if headers.len() != vals.len() {
            errors.push(format!(
                "In file {display_name}: Number of columns ({}) not the same as number of column names ({}) in row {}",
                vals.len(),
                headers.len(),
                py_repr_str(row)
            ));
            break;
        }
        for (header, val) in headers.iter().zip(vals.iter()) {
            // float(val), with strings silently mapped to 0 as the
            // reference does.
            let value: f64 = val.trim().parse().unwrap_or(0.0);
            table.push(header, value);
        }
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_table() {
        let mut errors = Vec::new();
        let t = convert_to_table("a.csv", "time,u,v\n0,1.5,2.5\n1,3.5,4.5\n", &mut errors);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(t.keys(), ["time", "u", "v"]);
        assert_eq!(t.get("u").expect("u"), &[1.5, 3.5]);
    }

    #[test]
    fn blank_lines_are_collapsed() {
        let mut errors = Vec::new();
        let t = convert_to_table("a.csv", "time,u\n\n  \n0,1\n\n1,2\n", &mut errors);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(t.get("u").expect("u"), &[1.0, 2.0]);
    }

    #[test]
    fn non_numeric_cells_become_zero() {
        let mut errors = Vec::new();
        let t = convert_to_table("a.csv", "time,u\n0,abc\n1,2\n", &mut errors);
        assert_eq!(t.get("u").expect("u"), &[0.0, 2.0]);
    }

    #[test]
    fn column_count_mismatch_reports_and_stops() {
        let mut errors = Vec::new();
        let t = convert_to_table("a.csv", "time,u\n0,1,2\n1,2\n", &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains(
                "Number of columns (3) not the same as number of column names (2) in row '0,1,2'"
            ),
            "{errors:?}"
        );
        assert_eq!(t.get("u").expect("u"), &[]);
    }

    #[test]
    fn empty_file_reports_pop_error() {
        let mut errors = Vec::new();
        let t = convert_to_table("a.csv", "\n  \n", &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Exception parsing file: ('pop from empty list',)"));
        assert!(t.is_empty());
    }
}
