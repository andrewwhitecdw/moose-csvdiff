//! Formatting helpers that reproduce CPython's number-to-string behavior.
//!
//! The reference implementation prints floats with Python's `str()` and
//! old-style `%` formatting (`%.3e`, `%.6g`). To keep output
//! byte-compatible, the routines below re-render Rust's shortest-round-trip
//! digits the way CPython does: two-digit signed exponents, fixed notation
//! for exponents in `[-4, 16)` for `repr` (and `[-4, P)` for `%.*g`), and
//! `%g`-style trailing-zero stripping.

/// `repr()` / `str()` of a Python float.
pub fn py_float_repr(x: f64) -> String {
    if x.is_nan() {
        return "nan".to_string();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0" } else { "0.0" }.to_string();
    }

    let (neg, digits, exp) = shortest_digits(x);
    let sign = if neg { "-" } else { "" };
    if !(-4..16).contains(&exp) {
        format!(
            "{sign}{}e{}{:02}",
            sci_mantissa(&digits),
            exp_sign(exp),
            exp.unsigned_abs()
        )
    } else {
        format!("{sign}{}", fixed_repr(&digits, exp))
    }
}

/// `"%.*e" % x` with CPython's two-digit signed exponent.
pub fn py_e(x: f64, prec: usize) -> String {
    if x.is_nan() {
        return "nan".to_string();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    let (neg, digits, exp) = shortest_digits(x);
    let (digits, exp) = round_to(&digits, exp, prec + 1);
    format!(
        "{}{}e{}{:02}",
        if neg { "-" } else { "" },
        dotted(&digits),
        exp_sign(exp),
        exp.unsigned_abs()
    )
}

/// `"%.*g" % x` with CPython's trailing-zero stripping.
pub fn py_g(x: f64, prec: usize) -> String {
    if x.is_nan() {
        return "nan".to_string();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0" } else { "0" }.to_string();
    }
    let (neg, digits, exp) = shortest_digits(x);
    let (digits, exp) = round_to(&digits, exp, prec.max(1));
    // %g strips trailing zeros of the fractional part.
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let sign = if neg { "-" } else { "" };
    if exp < -4 || exp >= prec.max(1) as i32 {
        format!(
            "{sign}{}e{}{:02}",
            sci_mantissa(digits),
            exp_sign(exp),
            exp.unsigned_abs()
        )
    } else {
        format!("{sign}{}", fixed_g(digits, exp))
    }
}

/// Extract sign, digit string (no dot), and base-10 exponent from Rust's
/// `{:e}` shortest formatting of `x`.
fn shortest_digits(x: f64) -> (bool, String, i32) {
    let s = format!("{:e}", x); // e.g. "5.5e-6", "1e16", "-2.25e-1"
    let (mantissa, exp) = s.split_once('e').expect("LowerExp always contains 'e'");
    let (neg, mantissa) = match mantissa.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, mantissa),
    };
    let digits: String = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
    (
        neg,
        digits,
        exp.parse::<i32>().expect("exponent is an integer"),
    )
}

/// Round a digit string (with the given base-10 exponent) to `want`
/// significant digits, returning the rounded digits and adjusted exponent.
/// The result always has exactly `want` digits unless `digits` was shorter,
/// in which case it is zero-padded on the right.
fn round_to(digits: &str, exp: i32, want: usize) -> (String, i32) {
    let mut d: Vec<u8> = digits.bytes().collect();
    while d.len() < want {
        d.push(b'0');
    }
    if d.len() > want {
        let round_up = d[want] >= b'5';
        d.truncate(want);
        if round_up {
            let mut i = want;
            loop {
                if i == 0 {
                    d.insert(0, b'1');
                    d.pop();
                    return (String::from_utf8(d).expect("ascii"), exp + 1);
                }
                i -= 1;
                if d[i] == b'9' {
                    d[i] = b'0';
                } else {
                    d[i] += 1;
                    break;
                }
            }
        }
    }
    (String::from_utf8(d).expect("ascii"), exp)
}

/// "d[.ddd]" for scientific notation (single digit is bare).
fn sci_mantissa(digits: &str) -> String {
    dotted(digits)
}

/// "d.ddd" rendering used by `%.e` (always shows the decimal point when
/// there is more than one digit).
fn dotted(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() + 1);
    out.push_str(&digits[..1]);
    if digits.len() > 1 {
        out.push('.');
        out.push_str(&digits[1..]);
    }
    out
}

/// Fixed form for `repr()`: integral values keep a trailing ".0".
fn fixed_repr(digits: &str, exp: i32) -> String {
    if exp >= 0 {
        let int_len = exp as usize + 1;
        if digits.len() <= int_len {
            let mut out = String::from(digits);
            out.extend(std::iter::repeat_n('0', int_len - digits.len()));
            out.push_str(".0");
            out
        } else {
            format!("{}.{}", &digits[..int_len], &digits[int_len..])
        }
    } else {
        let mut out = String::from("0.");
        out.extend(std::iter::repeat_n('0', (-exp - 1) as usize));
        out.push_str(digits);
        out
    }
}

/// Fixed form for `%g`: trailing zeros are already stripped by the caller,
/// so only the decimal placement remains.
fn fixed_g(digits: &str, exp: i32) -> String {
    if exp >= 0 {
        let int_len = exp as usize + 1;
        if digits.len() <= int_len {
            let mut out = String::from(digits);
            out.extend(std::iter::repeat_n('0', int_len - digits.len()));
            out
        } else {
            format!("{}.{}", &digits[..int_len], &digits[int_len..])
        }
    } else {
        let mut out = String::from("0.");
        out.extend(std::iter::repeat_n('0', (-exp - 1) as usize));
        out.push_str(digits);
        out
    }
}

fn exp_sign(exp: i32) -> char {
    if exp < 0 { '-' } else { '+' }
}

/// Python `repr()` of a string, as used in the reference's row-mismatch
/// message: single quotes preferred, double quotes when the value contains
/// a single quote but no double quote.
pub fn py_repr_str(s: &str) -> String {
    let use_double = s.contains('\'') && !s.contains('"');
    let quote = if use_double { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || (c as u32) == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_repr_matches_python() {
        assert_eq!(py_float_repr(5.5e-6), "5.5e-06");
        assert_eq!(py_float_repr(1e-10), "1e-10");
        assert_eq!(py_float_repr(1e-16), "1e-16");
        assert_eq!(py_float_repr(1e16), "1e+16");
        assert_eq!(py_float_repr(1.5e16), "1.5e+16");
        assert_eq!(py_float_repr(0.001), "0.001");
        assert_eq!(py_float_repr(100.0), "100.0");
        assert_eq!(py_float_repr(5.0), "5.0");
        assert_eq!(py_float_repr(0.5), "0.5");
        assert_eq!(py_float_repr(123456.7), "123456.7");
        assert_eq!(py_float_repr(1e15), "1000000000000000.0");
        assert_eq!(py_float_repr(-0.0), "-0.0");
        assert_eq!(py_float_repr(f64::INFINITY), "inf");
        assert_eq!(py_float_repr(f64::NAN), "nan");
    }

    #[test]
    fn e_format_matches_python() {
        assert_eq!(py_e(1.0, 3), "1.000e+00");
        assert_eq!(py_e(2.22, 3), "2.220e+00");
        assert_eq!(py_e(0.00012345, 3), "1.235e-04");
        assert_eq!(py_e(1e-10, 3), "1.000e-10");
        assert_eq!(py_e(0.0, 3), "0.000e+00");
        assert_eq!(py_e(-9.996293e-1, 3), "-9.996e-01");
        assert_eq!(py_e(9.9999999, 3), "1.000e+01");
    }

    #[test]
    fn g_format_matches_python() {
        assert_eq!(py_g(0.0, 6), "0");
        assert_eq!(py_g(1.0, 6), "1");
        assert_eq!(py_g(0.5, 6), "0.5");
        assert_eq!(py_g(123456.789, 6), "123457");
        assert_eq!(py_g(0.0001, 6), "0.0001");
        assert_eq!(py_g(0.00001, 6), "1e-05");
        assert_eq!(py_g(1234567.0, 6), "1.23457e+06");
        assert_eq!(py_g(9.9999999, 6), "10");
    }
}
