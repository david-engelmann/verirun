//! The strict JSON parser (docs/ARCHITECTURE.md section 4.5).
//!
//! Recursive descent over a byte slice, with a byte offset on every
//! failure. The parser never panics on malformed input: every index is
//! reached through checked access, and recursion is bounded by
//! [`MAX_DEPTH`].

use crate::canonical::MAX_SAFE_INTEGER;
use crate::error::{ParseError, ParseErrorKind};
use crate::value::Value;

/// The maximum nesting depth of arrays and objects. Deeper input fails
/// with [`crate::error::ParseErrorKind::DepthLimit`]. The limit bounds
/// stack use against adversarial input.
pub const MAX_DEPTH: usize = 128;

/// Parses one complete JSON document into a [`Value`].
///
/// Strictness, each with its byte offset: duplicate object keys are
/// rejected; numbers follow the JSON grammar and the value-based
/// domain of [`crate::canonical`]; `\u` escapes in the surrogate range
/// must form an adjacent valid pair; raw control characters inside
/// strings are rejected; trailing bytes after the top-level value are
/// rejected; nesting is capped at [`MAX_DEPTH`].
///
/// # Errors
///
/// Returns [`ParseError`] describing the first failure, with its kind
/// and byte offset.
pub fn parse(input: &[u8]) -> Result<Value, ParseError> {
    if input.is_empty() {
        return Err(ParseError::new(
            ParseErrorKind::EmptyInput,
            0,
            "input is empty".to_string(),
        ));
    }
    let text = match core::str::from_utf8(input) {
        Ok(text) => text,
        Err(err) => {
            let offset = err.valid_up_to();
            return Err(ParseError::new(
                ParseErrorKind::InvalidUtf8,
                offset,
                format!("invalid UTF-8 starting at byte {offset}"),
            ));
        }
    };
    let mut parser = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    parser.skip_whitespace();
    if parser.pos >= parser.bytes.len() {
        return Err(ParseError::new(
            ParseErrorKind::EmptyInput,
            parser.pos,
            "input holds only whitespace".to_string(),
        ));
    }
    let value = parser.parse_value(1)?;
    parser.skip_whitespace();
    if parser.pos != parser.bytes.len() {
        return Err(ParseError::new(
            ParseErrorKind::TrailingBytes,
            parser.pos,
            format!(
                "trailing byte 0x{:02X} after the top-level value",
                parser.bytes[parser.pos]
            ),
        ));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    const fn error(&self, kind: ParseErrorKind, detail: String) -> ParseError {
        ParseError::new(kind, self.pos, detail)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    /// Parses one value. `depth` is 1 for the top-level value and
    /// grows by one per enclosing container.
    fn parse_value(&mut self, depth: usize) -> Result<Value, ParseError> {
        match self.peek() {
            None => Err(self.error(
                ParseErrorKind::UnexpectedEnd,
                "input ended where a value was expected".to_string(),
            )),
            Some(b'n') => {
                self.parse_literal("null")?;
                Ok(Value::Null)
            }
            Some(b't') => {
                self.parse_literal("true")?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.parse_literal("false")?;
                Ok(Value::Bool(false))
            }
            Some(b'"') => Ok(Value::String(self.parse_string()?)),
            Some(b'[') => self.parse_array(depth),
            Some(b'{') => self.parse_object(depth),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(byte) => Err(self.error(
                ParseErrorKind::UnexpectedByte,
                format!("byte 0x{byte:02X} cannot start a value"),
            )),
        }
    }

    fn parse_literal(&mut self, literal: &str) -> Result<(), ParseError> {
        let expected = literal.as_bytes();
        let remaining = &self.bytes[self.pos..];
        if remaining.starts_with(expected) {
            self.pos += expected.len();
            return Ok(());
        }
        if remaining.len() < expected.len() && expected.starts_with(remaining) {
            return Err(self.error(
                ParseErrorKind::UnexpectedEnd,
                format!("input ended inside the literal {literal}"),
            ));
        }
        Err(self.error(
            ParseErrorKind::InvalidLiteral,
            format!("expected the literal {literal}"),
        ))
    }

    fn parse_number(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        // Integer part: a single zero, or a non-zero digit run.
        match self.peek() {
            Some(b'0') => {
                self.pos += 1;
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(self.error(
                        ParseErrorKind::InvalidNumber,
                        "leading zeros are not part of the number grammar".to_string(),
                    ));
                }
            }
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => {
                return Err(self.error(
                    ParseErrorKind::InvalidNumber,
                    "a number needs at least one integer digit".to_string(),
                ));
            }
        }
        // Fraction part.
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error(
                    ParseErrorKind::InvalidNumber,
                    "a fraction needs at least one digit".to_string(),
                ));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        // Exponent part.
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error(
                    ParseErrorKind::InvalidNumber,
                    "an exponent needs at least one digit".to_string(),
                ));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        let text = core::str::from_utf8(&self.bytes[start..self.pos])
            .expect("number bytes are ASCII by construction");
        let value: f64 = text.parse().map_err(|_| {
            self.error(
                ParseErrorKind::InvalidNumber,
                format!("number literal {text} does not convert to f64"),
            )
        })?;
        if value.is_infinite() {
            return Err(ParseError::new(
                ParseErrorKind::NumberOutOfRange,
                start,
                format!("number literal {text} overflows f64"),
            ));
        }
        #[allow(
            clippy::float_cmp,
            reason = "the integrality check of the value-based domain is exact by specification"
        )]
        if value.fract() == 0.0 && value.abs() > MAX_SAFE_INTEGER {
            return Err(ParseError::new(
                ParseErrorKind::IntegerOutOfSafeRange,
                start,
                format!("integer {text} exceeds the safe bound {MAX_SAFE_INTEGER} (2^53 minus 1)"),
            ));
        }
        Ok(Value::Number(value))
    }

    /// Parses a string starting at the opening quote, consuming
    /// through the closing quote.
    fn parse_string(&mut self) -> Result<String, ParseError> {
        debug_assert_eq!(self.peek(), Some(b'"'));
        self.pos += 1;
        let mut out = String::new();
        let mut segment_start = self.pos;
        loop {
            match self.peek() {
                None => {
                    return Err(self.error(
                        ParseErrorKind::UnexpectedEnd,
                        "input ended inside a string".to_string(),
                    ));
                }
                Some(b'"') => {
                    out.push_str(
                        core::str::from_utf8(&self.bytes[segment_start..self.pos])
                            .expect("segments end at ASCII boundaries"),
                    );
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    out.push_str(
                        core::str::from_utf8(&self.bytes[segment_start..self.pos])
                            .expect("segments end at ASCII boundaries"),
                    );
                    self.pos += 1;
                    self.parse_escape(&mut out)?;
                    segment_start = self.pos;
                }
                Some(byte) if byte < 0x20 => {
                    return Err(self.error(
                        ParseErrorKind::ControlCharacterInString,
                        format!("raw control byte 0x{byte:02X} inside a string"),
                    ));
                }
                Some(_) => {
                    self.pos += 1;
                }
            }
        }
    }

    /// Parses one escape sequence. `self.pos` is just past the
    /// backslash on entry.
    fn parse_escape(&mut self, out: &mut String) -> Result<(), ParseError> {
        let escape_offset = self.pos - 1;
        match self.peek() {
            None => Err(self.error(
                ParseErrorKind::UnexpectedEnd,
                "input ended after a string backslash".to_string(),
            )),
            Some(b'"') => {
                self.pos += 1;
                out.push('"');
                Ok(())
            }
            Some(b'\\') => {
                self.pos += 1;
                out.push('\\');
                Ok(())
            }
            Some(b'/') => {
                self.pos += 1;
                out.push('/');
                Ok(())
            }
            Some(b'b') => {
                self.pos += 1;
                out.push('\u{08}');
                Ok(())
            }
            Some(b'f') => {
                self.pos += 1;
                out.push('\u{0C}');
                Ok(())
            }
            Some(b'n') => {
                self.pos += 1;
                out.push('\n');
                Ok(())
            }
            Some(b'r') => {
                self.pos += 1;
                out.push('\r');
                Ok(())
            }
            Some(b't') => {
                self.pos += 1;
                out.push('\t');
                Ok(())
            }
            Some(b'u') => {
                self.pos += 1;
                let code = self.parse_hex4()?;
                if (0xD800..0xDC00).contains(&code) {
                    // A high surrogate must be followed immediately by
                    // a low surrogate escape.
                    if self.peek() == Some(b'\\') && self.bytes.get(self.pos + 1) == Some(&b'u') {
                        self.pos += 2;
                        let low = self.parse_hex4()?;
                        if (0xDC00..0xE000).contains(&low) {
                            let combined = 0x1_0000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                            let ch = char::from_u32(combined)
                                .expect("a valid surrogate pair forms a valid scalar value");
                            out.push(ch);
                            return Ok(());
                        }
                    }
                    Err(ParseError::new(
                        ParseErrorKind::LoneSurrogate,
                        escape_offset,
                        format!("high surrogate \\u{code:04X} has no low surrogate partner"),
                    ))
                } else if (0xDC00..0xE000).contains(&code) {
                    Err(ParseError::new(
                        ParseErrorKind::LoneSurrogate,
                        escape_offset,
                        format!("low surrogate \\u{code:04X} has no high surrogate partner"),
                    ))
                } else {
                    let ch = char::from_u32(code)
                        .expect("a non-surrogate BMP code unit is a valid scalar value");
                    out.push(ch);
                    Ok(())
                }
            }
            Some(byte) => Err(self.error(
                ParseErrorKind::InvalidEscape,
                format!("escape byte 0x{byte:02X} is not a JSON escape"),
            )),
        }
    }

    /// Reads exactly four hexadecimal digits after a `\u`.
    fn parse_hex4(&mut self) -> Result<u32, ParseError> {
        let mut code: u32 = 0;
        for _ in 0..4 {
            match self.peek() {
                None => {
                    return Err(self.error(
                        ParseErrorKind::UnexpectedEnd,
                        "input ended inside a unicode escape".to_string(),
                    ));
                }
                Some(byte) => {
                    let digit = match byte {
                        b'0'..=b'9' => u32::from(byte - b'0'),
                        b'a'..=b'f' => u32::from(byte - b'a') + 10,
                        b'A'..=b'F' => u32::from(byte - b'A') + 10,
                        _ => {
                            return Err(self.error(
                                ParseErrorKind::InvalidUnicodeEscape,
                                format!("byte 0x{byte:02X} is not a hexadecimal digit"),
                            ));
                        }
                    };
                    code = code * 16 + digit;
                    self.pos += 1;
                }
            }
        }
        Ok(code)
    }

    fn parse_array(&mut self, depth: usize) -> Result<Value, ParseError> {
        if depth > MAX_DEPTH {
            return Err(self.error(
                ParseErrorKind::DepthLimit,
                format!("nesting exceeds the depth limit of {MAX_DEPTH}"),
            ));
        }
        self.pos += 1; // consume '['
        self.skip_whitespace();
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_whitespace();
            let item = self.parse_value(depth + 1)?;
            items.push(item);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                Some(byte) => {
                    return Err(self.error(
                        ParseErrorKind::UnexpectedByte,
                        format!("byte 0x{byte:02X} where ',' or ']' was expected"),
                    ));
                }
                None => {
                    return Err(self.error(
                        ParseErrorKind::UnexpectedEnd,
                        "input ended inside an array".to_string(),
                    ));
                }
            }
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<Value, ParseError> {
        if depth > MAX_DEPTH {
            return Err(self.error(
                ParseErrorKind::DepthLimit,
                format!("nesting exceeds the depth limit of {MAX_DEPTH}"),
            ));
        }
        self.pos += 1; // consume '{'
        self.skip_whitespace();
        let mut pairs: Vec<(String, Value)> = Vec::new();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(pairs));
        }
        loop {
            self.skip_whitespace();
            let key_offset = self.pos;
            if self.peek() != Some(b'"') {
                return Err(self.peek().map_or_else(
                    || {
                        self.error(
                            ParseErrorKind::UnexpectedEnd,
                            "input ended inside an object".to_string(),
                        )
                    },
                    |byte| {
                        self.error(
                            ParseErrorKind::UnexpectedByte,
                            format!("byte 0x{byte:02X} where an object key was expected"),
                        )
                    },
                ));
            }
            let key = self.parse_string()?;
            if pairs.iter().any(|(existing, _)| existing == &key) {
                return Err(ParseError::new(
                    ParseErrorKind::DuplicateKey,
                    key_offset,
                    format!("duplicate object key {key:?}"),
                ));
            }
            self.skip_whitespace();
            match self.peek() {
                Some(b':') => {
                    self.pos += 1;
                }
                Some(byte) => {
                    return Err(self.error(
                        ParseErrorKind::UnexpectedByte,
                        format!("byte 0x{byte:02X} where ':' was expected"),
                    ));
                }
                None => {
                    return Err(self.error(
                        ParseErrorKind::UnexpectedEnd,
                        "input ended after an object key".to_string(),
                    ));
                }
            }
            self.skip_whitespace();
            let value = self.parse_value(depth + 1)?;
            pairs.push((key, value));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(pairs));
                }
                Some(byte) => {
                    return Err(self.error(
                        ParseErrorKind::UnexpectedByte,
                        format!("byte 0x{byte:02X} where ',' or '}}' was expected"),
                    ));
                }
                None => {
                    return Err(self.error(
                        ParseErrorKind::UnexpectedEnd,
                        "input ended inside an object".to_string(),
                    ));
                }
            }
        }
    }
}
