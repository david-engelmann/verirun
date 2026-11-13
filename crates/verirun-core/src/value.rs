//! The value model canonicalization operates on
//! (docs/ARCHITECTURE.md section 3.1).
//!
//! Objects store pairs in insertion order in memory. Order in memory
//! carries no meaning: the canonical encoder sorts at encode time, so
//! two objects with the same pairs in different orders are the same
//! value for every verirun purpose.

use crate::canonical::check_number_domain;
use crate::error::{CanonicalError, CanonicalErrorKind};

/// A JSON value under the verirun number domain.
///
/// The `Number` payload is an `f64`. Domain validation (finite, and no
/// integral value above the safe integer bound) is enforced by
/// [`Value::number`], by the parser, and again by the canonical encoder,
/// which is the gate that protects digests: a `Number` built directly
/// through the enum variant with an out-of-domain payload is rejected
/// at encode time, never silently hashed.
///
/// Equality follows the model, not the memory layout: two objects
/// with the same pairs in different orders are equal, because pair
/// order in memory carries no meaning.
#[derive(Debug, Clone)]
pub enum Value {
    /// JSON `null`.
    Null,
    /// JSON `true` or `false`.
    Bool(bool),
    /// A JSON number, as an `f64` under the domain rules.
    Number(f64),
    /// A JSON string.
    String(String),
    /// A JSON array, order-significant.
    Array(Vec<Self>),
    /// A JSON object, as insertion-ordered pairs with unique keys.
    Object(Vec<(String, Self)>),
}

#[allow(
    clippy::float_cmp,
    reason = "number equality is payload identity by specification; NaN cannot be constructed through the validating constructors"
)]
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(left), Self::Bool(right)) => left == right,
            (Self::Number(left), Self::Number(right)) => left == right,
            (Self::String(left), Self::String(right)) => left == right,
            (Self::Array(left), Self::Array(right)) => left == right,
            (Self::Object(left), Self::Object(right)) => {
                if left.len() != right.len() {
                    return false;
                }
                let mut left_sorted: Vec<&(String, Self)> = left.iter().collect();
                let mut right_sorted: Vec<&(String, Self)> = right.iter().collect();
                left_sorted.sort_by(|a, b| a.0.cmp(&b.0));
                right_sorted.sort_by(|a, b| a.0.cmp(&b.0));
                left_sorted
                    .iter()
                    .zip(right_sorted.iter())
                    .all(|(a, b)| a.0 == b.0 && a.1 == b.1)
            }
            _ => false,
        }
    }
}

impl Value {
    /// Builds a `Number` value, validating the number domain.
    ///
    /// # Errors
    ///
    /// Returns [`CanonicalError`] with kind
    /// [`CanonicalErrorKind::NonFiniteNumber`] for NaN and the
    /// infinities, or [`CanonicalErrorKind::IntegerOutOfSafeRange`] for
    /// an integral value whose magnitude exceeds 9007199254740991.
    pub fn number(value: f64) -> Result<Self, CanonicalError> {
        check_number_domain(value)?;
        Ok(Self::Number(value))
    }

    /// Builds an `Object` value, rejecting duplicate keys.
    ///
    /// # Errors
    ///
    /// Returns [`CanonicalError`] with kind
    /// [`CanonicalErrorKind::DuplicateKey`], naming the duplicated key,
    /// when any key appears twice in `pairs`.
    pub fn object(pairs: Vec<(String, Self)>) -> Result<Self, CanonicalError> {
        let mut seen = std::collections::BTreeSet::new();
        for (key, _) in &pairs {
            if !seen.insert(key) {
                return Err(CanonicalError::new(
                    CanonicalErrorKind::DuplicateKey,
                    format!("duplicate object key {key:?}"),
                ));
            }
        }
        Ok(Self::Object(pairs))
    }
}
