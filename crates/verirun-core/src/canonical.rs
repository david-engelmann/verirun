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
    unimplemented!("Stage A contract; implemented in Stage B")
}

/// Formats one `f64` under the ECMA-262 `Number::toString` layout of
/// section 4.3, after validating the domain with
/// [`check_number_domain`].
///
/// # Errors
///
/// Returns the [`CanonicalError`] of [`check_number_domain`] for an
/// out-of-domain value.
pub fn format_number(value: f64) -> Result<String, CanonicalError> {
    unimplemented!("Stage A contract; implemented in Stage B")
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
    unimplemented!("Stage A contract; implemented in Stage B")
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
