//! The RFC 8785 (JCS) canonical encoder, under the verirun profile of
//! docs/ARCHITECTURE.md section 4.
//!
//! The profile in brief: no whitespace; object members sorted by the
//! UTF-16 code unit sequences of their names; strings escaped minimally
//! per RFC 8785 section 3.2.2.2; numbers laid out per ECMA-262
//! `Number::toString` over a value-based domain that rejects NaN, the
//! infinities, and any integral value whose magnitude exceeds
//! [`MAX_SAFE_INTEGER`].

use crate::error::{CanonicalError, CanonicalErrorKind};
use crate::value::Value;

/// The largest IEEE 754 safe integer, 2^53 minus 1. Integral values with
/// a larger magnitude have no portable canonical form under the
/// value-based domain rule and are rejected.
pub const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// A destination for canonical bytes.
///
/// The encoder streams into a sink and never requires the whole output
/// in memory at once. The `Vec<u8>` implementation is infallible. Test
/// sinks can count calls and chunk sizes to prove the streaming shape.
pub trait ByteSink {
    /// Appends bytes to the sink.
    ///
    /// # Errors
    ///
    /// Implementations backed by fallible storage return
    /// [`CanonicalError`] when a write fails.
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), CanonicalError>;
}

impl ByteSink for Vec<u8> {
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), CanonicalError> {
        self.extend_from_slice(bytes);
        Ok(())
    }
}

/// Validates one `f64` against the number domain of section 4.3:
/// finite, and not integral with magnitude above [`MAX_SAFE_INTEGER`].
/// Negative zero is in domain and encodes as `0`.
///
/// # Errors
///
/// Returns [`CanonicalError`] with kind
/// [`CanonicalErrorKind::NonFiniteNumber`] or
/// [`CanonicalErrorKind::IntegerOutOfSafeRange`].
#[allow(
    clippy::float_cmp,
    reason = "exact comparisons are the domain rule itself: the zero check and the integrality check must be exact, never approximate"
)]
pub fn check_number_domain(value: f64) -> Result<(), CanonicalError> {
    if !value.is_finite() {
        return Err(CanonicalError::new(
            CanonicalErrorKind::NonFiniteNumber,
            format!("number {value} is not finite; RFC 8785 has no encoding for it"),
        ));
    }
    if value.fract() == 0.0 && value.abs() > MAX_SAFE_INTEGER {
        return Err(CanonicalError::new(
            CanonicalErrorKind::IntegerOutOfSafeRange,
            format!("integer {value} exceeds the safe bound {MAX_SAFE_INTEGER} (2^53 minus 1)"),
        ));
    }
    Ok(())
}

/// Formats one `f64` under the ECMA-262 `Number::toString` layout of
/// section 4.3, after validating the domain with
/// [`check_number_domain`].
///
/// The digit extraction starts from the standard library's plain
/// decimal rendering, which is the shortest string that rounds back to
/// the same `f64` and never uses scientific notation. The layout cases
/// then follow ECMA-262 exactly.
///
/// # Errors
///
/// Returns the [`CanonicalError`] of [`check_number_domain`] for an
/// out-of-domain value.
#[allow(
    clippy::float_cmp,
    reason = "the zero and sign checks here are exact by specification"
)]
pub fn format_number(value: f64) -> Result<String, CanonicalError> {
    check_number_domain(value)?;
    if value == 0.0 {
        // Covers negative zero: the canonical form is "0".
        return Ok("0".to_string());
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let rendered = format!("{}", value.abs());
    let (int_part, frac_part) = match rendered.split_once('.') {
        Some((int, frac)) => (int, frac),
        None => (rendered.as_str(), ""),
    };
    // The value is DIGITS * 10^exp10, where DIGITS is the integer
    // formed by concatenating the integer and fraction digits.
    // The digits are ASCII, so the trimming below works on the
    // string directly; an earlier shape detoured through a byte
    // vector and a collect, two allocations the layout never
    // needed.
    let mut digit_text = String::with_capacity(int_part.len() + frac_part.len());
    digit_text.push_str(int_part);
    digit_text.push_str(frac_part);
    let mut exp10 = -i32::try_from(frac_part.len()).unwrap_or(i32::MAX);
    // Leading zeros do not change the integer or the exponent.
    let leading_zeros = digit_text.bytes().take_while(|byte| *byte == b'0').count();
    digit_text.drain(..leading_zeros);
    // Each trailing zero removed raises the exponent by one.
    while digit_text.len() > 1 && digit_text.ends_with('0') {
        digit_text.pop();
        exp10 += 1;
    }
    let k = i32::try_from(digit_text.len()).unwrap_or(i32::MAX);
    // ECMA-262 works with n such that value = 0.digits * 10^n.
    let n = exp10 + k;
    let body = layout(&digit_text, k, n);
    if sign.is_empty() {
        Ok(body)
    } else {
        Ok(format!("{sign}{body}"))
    }
}

/// The four ECMA-262 layout cases over a digit string `s` of length
/// `k` and decimal exponent `n` (the value is `0.s * 10^n`).
fn layout(s: &str, k: i32, n: i32) -> String {
    if k <= n && n <= 21 {
        // Digits followed by n - k zeros, no decimal point.
        let zeros = usize::try_from(n - k).unwrap_or(0);
        let mut out = String::with_capacity(s.len() + zeros);
        out.push_str(s);
        for _ in 0..zeros {
            out.push('0');
        }
        out
    } else if 0 < n && n <= 21 {
        // The decimal point falls inside the digit string.
        let point = usize::try_from(n).unwrap_or(0);
        format!("{}.{}", &s[..point], &s[point..])
    } else if -6 < n && n <= 0 {
        // Zero, point, -n zeros, then the digits.
        let zeros = usize::try_from(-n).unwrap_or(0);
        let mut out = String::from("0.");
        for _ in 0..zeros {
            out.push('0');
        }
        out.push_str(s);
        out
    } else if k == 1 {
        // One digit in scientific notation.
        format!("{s}e{}{}", exponent_sign(n - 1), (n - 1).abs())
    } else {
        // Scientific notation with the point after the first digit.
        format!(
            "{}.{}e{}{}",
            &s[..1],
            &s[1..],
            exponent_sign(n - 1),
            (n - 1).abs()
        )
    }
}

const fn exponent_sign(exponent: i32) -> char {
    if exponent >= 0 { '+' } else { '-' }
}

/// Encodes a value as canonical bytes into `sink`.
///
/// Object members are sorted by UTF-16 code unit order at every depth.
/// Duplicate keys in an object are rejected. Every number is validated
/// against the domain as it is reached.
///
/// # Errors
///
/// Returns [`CanonicalError`] for an out-of-domain number or a
/// duplicate object key, and propagates any sink failure.
pub fn encode<S: ByteSink + ?Sized>(value: &Value, sink: &mut S) -> Result<(), CanonicalError> {
    match value {
        Value::Null => sink.write_bytes(b"null"),
        Value::Bool(true) => sink.write_bytes(b"true"),
        Value::Bool(false) => sink.write_bytes(b"false"),
        Value::Number(number) => {
            let text = format_number(*number)?;
            sink.write_bytes(text.as_bytes())
        }
        Value::String(text) => encode_string(text, sink),
        Value::Array(items) => {
            sink.write_bytes(b"[")?;
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    sink.write_bytes(b",")?;
                }
                encode(item, sink)?;
            }
            sink.write_bytes(b"]")
        }
        Value::Object(pairs) => encode_object(pairs, sink),
    }
}

fn encode_object<S: ByteSink + ?Sized>(
    pairs: &[(String, Value)],
    sink: &mut S,
) -> Result<(), CanonicalError> {
    // Sort by the UTF-16 code unit sequence of each name. Rust string
    // order is code point order, which disagrees with UTF-16 order
    // around the BMP boundary, so the comparison runs over the
    // encode_utf16 iterators directly. That keeps the pass to one
    // allocation, the reference vector; an earlier shape collected
    // a UTF-16 buffer per key before it could compare anything.
    let mut refs: Vec<(&String, &Value)> = pairs.iter().map(|pair| (&pair.0, &pair.1)).collect();
    refs.sort_by(|left, right| left.0.encode_utf16().cmp(right.0.encode_utf16()));
    // Equal names have equal UTF-16 sequences, so duplicates are
    // adjacent after the sort.
    for window in refs.windows(2) {
        if window[0].0 == window[1].0 {
            return Err(CanonicalError::new(
                CanonicalErrorKind::DuplicateKey,
                format!("duplicate object key {:?}", window[0].0),
            ));
        }
    }
    sink.write_bytes(b"{")?;
    for (index, (key, value)) in refs.iter().enumerate() {
        if index > 0 {
            sink.write_bytes(b",")?;
        }
        encode_string(key, sink)?;
        sink.write_bytes(b":")?;
        encode(value, sink)?;
    }
    sink.write_bytes(b"}")
}

/// Writes one string with the RFC 8785 section 3.2.2.2 escapes: only
/// the quote, the reverse solidus, and control characters below
/// U+0020 are escaped, the five named control characters by their
/// short forms and the rest as `\u00xx` with lowercase hex.
fn encode_string<S: ByteSink + ?Sized>(text: &str, sink: &mut S) -> Result<(), CanonicalError> {
    // Written to the sink in runs: plain spans cross as slices
    // of the source and only escaped characters produce their
    // own writes, so no intermediate string is built, however
    // long the text. An earlier shape buffered every string
    // whole before writing it once.
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = text.as_bytes();
    sink.write_bytes(b"\"")?;
    let mut run_start = 0;
    for (index, ch) in text.char_indices() {
        let escape: &[u8] = match ch {
            '"' => b"\\\"",
            '\\' => b"\\\\",
            '\u{08}' => b"\\b",
            '\u{09}' => b"\\t",
            '\u{0A}' => b"\\n",
            '\u{0C}' => b"\\f",
            '\u{0D}' => b"\\r",
            control if (control as u32) < 0x20 => {
                sink.write_bytes(&bytes[run_start..index])?;
                let code = control as u32;
                let escape = [
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    HEX[usize::try_from((code >> 4) & 0xF).unwrap_or(0)],
                    HEX[usize::try_from(code & 0xF).unwrap_or(0)],
                ];
                sink.write_bytes(&escape)?;
                // A character below U+0020 is one byte in UTF-8.
                run_start = index + 1;
                continue;
            }
            _ => continue,
        };
        sink.write_bytes(&bytes[run_start..index])?;
        sink.write_bytes(escape)?;
        run_start = index + ch.len_utf8();
    }
    sink.write_bytes(&bytes[run_start..])?;
    sink.write_bytes(b"\"")
}

/// Encodes a value to a fresh byte vector. Convenience wrapper over
/// [`encode`] for callers that hold values small enough to buffer.
///
/// # Errors
///
/// Returns the [`CanonicalError`] of [`encode`].
pub fn to_vec(value: &Value) -> Result<Vec<u8>, CanonicalError> {
    let mut out = Vec::new();
    encode(value, &mut out)?;
    Ok(out)
}
