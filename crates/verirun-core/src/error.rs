//! Core error types.
//!
//! Every error carries the concrete diagnostic context a person needs to
//! act: byte offsets, the offending text, and expected versus actual
//! values where a comparison failed. A bare variant name is never the
//! whole message (docs/ARCHITECTURE.md section 11).

use core::fmt;

/// The class of a [`CanonicalError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CanonicalErrorKind {
    /// The number is NaN or an infinity. RFC 8785 has no encoding for it.
    NonFiniteNumber,
    /// The number is integral in value and its magnitude exceeds the
    /// IEEE 754 safe integer bound of 9007199254740991.
    IntegerOutOfSafeRange,
    /// An object was constructed or encoded with a duplicate key.
    DuplicateKey,
}

/// A canonicalization failure: a domain rejection from the number rules
/// or from object construction or encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalError {
    kind: CanonicalErrorKind,
    offset: Option<usize>,
    detail: String,
}

impl CanonicalError {
    /// Creates an error with no source offset.
    #[must_use]
    pub fn new(kind: CanonicalErrorKind, detail: String) -> Self {
        Self {
            kind,
            offset: None,
            detail,
        }
    }

    /// Creates an error tied to a byte offset in a parsed source.
    #[must_use]
    pub fn at_offset(kind: CanonicalErrorKind, offset: usize, detail: String) -> Self {
        Self {
            kind,
            offset: Some(offset),
            detail,
        }
    }

    /// The error class.
    #[must_use]
    pub const fn kind(&self) -> CanonicalErrorKind {
        self.kind
    }

    /// The byte offset in the source, when the error came from parsing.
    #[must_use]
    pub const fn offset(&self) -> Option<usize> {
        self.offset
    }

    /// The human-readable diagnostic detail.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for CanonicalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.offset {
            Some(offset) => write!(f, "canonicalization error at byte {offset}: {}", self.detail),
            None => write!(f, "canonicalization error: {}", self.detail),
        }
    }
}

impl std::error::Error for CanonicalError {}

/// The class of a [`ParseError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParseErrorKind {
    /// The input is empty.
    EmptyInput,
    /// A byte that cannot start or continue the expected token.
    UnexpectedByte,
    /// The input ended in the middle of a value.
    UnexpectedEnd,
    /// A literal other than `true`, `false`, or `null`.
    InvalidLiteral,
    /// A number that violates the JSON number grammar.
    InvalidNumber,
    /// A number literal that overflows f64 to an infinity.
    NumberOutOfRange,
    /// A number whose integral value exceeds the safe integer bound.
    IntegerOutOfSafeRange,
    /// An object key that already appeared in the same object.
    DuplicateKey,
    /// Nesting deeper than the parser depth limit.
    DepthLimit,
    /// A `\u` escape holding a surrogate with no valid partner.
    LoneSurrogate,
    /// An unknown escape character after a backslash.
    InvalidEscape,
    /// A `\u` escape that is not four hexadecimal digits.
    InvalidUnicodeEscape,
    /// A raw control character (below U+0020) inside a string.
    ControlCharacterInString,
    /// The input is not valid UTF-8.
    InvalidUtf8,
    /// Non-whitespace bytes after the top-level value.
    TrailingBytes,
}

/// A parse failure. Always carries the byte offset of the failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    kind: ParseErrorKind,
    offset: usize,
    detail: String,
}

impl ParseError {
    /// Creates a parse error at a byte offset.
    #[must_use]
    pub fn new(kind: ParseErrorKind, offset: usize, detail: String) -> Self {
        Self {
            kind,
            offset,
            detail,
        }
    }

    /// The error class.
    #[must_use]
    pub const fn kind(&self) -> ParseErrorKind {
        self.kind
    }

    /// The byte offset of the failure in the input.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// The human-readable diagnostic detail.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at byte {}: {}", self.offset, self.detail)
    }
}

impl std::error::Error for ParseError {}

/// The class of a [`DigestError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DigestErrorKind {
    /// The hex text has the wrong length for a 32-byte digest.
    InvalidLength,
    /// The hex text holds a character that is not a lowercase hex digit.
    InvalidCharacter,
}

/// A failure to parse a [`crate::digest::Digest`] from hex text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestError {
    kind: DigestErrorKind,
    detail: String,
}

impl DigestError {
    /// Creates a digest error.
    #[must_use]
    pub fn new(kind: DigestErrorKind, detail: String) -> Self {
        Self { kind, detail }
    }

    /// The error class.
    #[must_use]
    pub const fn kind(&self) -> DigestErrorKind {
        self.kind
    }

    /// The human-readable diagnostic detail.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for DigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "digest error: {}", self.detail)
    }
}

impl std::error::Error for DigestError {}
