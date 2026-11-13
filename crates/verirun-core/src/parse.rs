//! The strict JSON parser (docs/ARCHITECTURE.md section 4.5).
//!
//! Recursive descent over a byte slice, with a byte offset on every
//! failure. The parser never panics on malformed input.

use crate::error::ParseError;
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
    unimplemented!("Stage A contract; implemented in Stage B")
}
