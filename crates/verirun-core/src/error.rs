//! Core error types.
//!
//! Every error carries the concrete diagnostic context a person needs to
//! act: byte offsets, the offending text, and expected versus actual
//! values where a comparison failed. A bare variant name is never the
//! whole message (docs/ARCHITECTURE.md section 11).

use core::fmt;

use crate::digest::Digest;

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
    pub const fn new(kind: CanonicalErrorKind, detail: String) -> Self {
        Self {
            kind,
            offset: None,
            detail,
        }
    }

    /// Creates an error tied to a byte offset in a parsed source.
    #[must_use]
    pub const fn at_offset(kind: CanonicalErrorKind, offset: usize, detail: String) -> Self {
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
            Some(offset) => write!(
                f,
                "canonicalization error at byte {offset}: {}",
                self.detail
            ),
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
    pub const fn new(kind: ParseErrorKind, offset: usize, detail: String) -> Self {
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
    pub const fn new(kind: DigestErrorKind, detail: String) -> Self {
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

/// The class of a [`PathError`]: the sandbox path rule a declared
/// path broke (docs/ARCHITECTURE.md section 8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathErrorKind {
    /// The path is empty, or nothing remains after dot and empty
    /// components are normalized away.
    Empty,
    /// The path is absolute: a leading separator, or a Windows
    /// drive prefix of one ASCII letter and a colon.
    Absolute,
    /// The path contains a backslash, a separator spelling this
    /// system does not use.
    BackslashSeparator,
    /// The path contains a `..` component.
    ParentTraversal,
}

/// A sandbox path rule failure. Carries the declared path exactly
/// as written and the rule it broke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathError {
    kind: PathErrorKind,
    declared: String,
    detail: String,
}

impl PathError {
    /// Creates a path error for a declared path.
    #[must_use]
    pub const fn new(kind: PathErrorKind, declared: String, detail: String) -> Self {
        Self {
            kind,
            declared,
            detail,
        }
    }

    /// The rule the path broke.
    #[must_use]
    pub const fn kind(&self) -> PathErrorKind {
        self.kind
    }

    /// The declared path, exactly as written in the spec.
    #[must_use]
    pub fn declared(&self) -> &str {
        &self.declared
    }

    /// The human-readable diagnostic detail.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid sandbox path {:?}: {}",
            self.declared, self.detail
        )
    }
}

impl std::error::Error for PathError {}

/// A DAG validation failure (docs/ARCHITECTURE.md section 7.1).
///
/// Validation checks run in a fixed order, so for a given spec
/// exactly one of these is ever the reported error, and it is the
/// same on every host and every run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DagError {
    /// A task object, or the run file itself, violates the schema
    /// of section 3.2. `task_id` names the task when the violation
    /// is inside one and the id could be read.
    Schema {
        /// The task holding the violation, when identifiable.
        task_id: Option<String>,
        /// What the schema requires and what was found.
        detail: String,
    },
    /// Two tasks share a `task_id`. The error names the id.
    DuplicateTaskId {
        /// The duplicated task id.
        task_id: String,
    },
    /// A `depends_on` entry names no task in the run.
    UnknownDependency {
        /// The task declaring the dependency.
        task_id: String,
        /// The dependency id that names no task.
        missing: String,
    },
    /// A normalized output path is claimed by two tasks, or twice
    /// by one task.
    CollidingOutput {
        /// The normalized colliding path.
        path: String,
        /// The first task that claimed the path.
        first_task: String,
        /// The second task that claimed the path.
        second_task: String,
    },
    /// A declared input or output path breaks a sandbox path rule.
    InvalidPath {
        /// The task declaring the path.
        task_id: String,
        /// The path failure, carrying the declared path.
        source: PathError,
    },
    /// The graph does not fully order under Kahn's algorithm.
    Cycle {
        /// The tasks left with nonzero indegree, sorted.
        remnants: Vec<String>,
    },
}

impl fmt::Display for DagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema { task_id, detail } => match task_id {
                Some(task_id) => write!(f, "schema violation in task {task_id:?}: {detail}"),
                None => write!(f, "schema violation: {detail}"),
            },
            Self::DuplicateTaskId { task_id } => {
                write!(f, "duplicate task id {task_id:?}")
            }
            Self::UnknownDependency { task_id, missing } => {
                write!(f, "task {task_id:?} depends on unknown task {missing:?}")
            }
            Self::CollidingOutput {
                path,
                first_task,
                second_task,
            } => write!(
                f,
                "output path {path:?} is claimed by both {first_task:?} and {second_task:?}"
            ),
            Self::InvalidPath { task_id, source } => {
                write!(f, "task {task_id:?} declares an invalid path: {source}")
            }
            Self::Cycle { remnants } => write!(
                f,
                "dependency cycle: tasks left unordered are {}",
                remnants.join(", ")
            ),
        }
    }
}

impl std::error::Error for DagError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPath { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// A failure to interpret a value as a manifest or a task record
/// (docs/ARCHITECTURE.md section 9.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// The `format` field names a different manifest format.
    WrongFormat {
        /// The format string found.
        found: String,
    },
    /// A required field is absent.
    MissingField {
        /// The absent field's name.
        field: &'static str,
    },
    /// A field holds a value of the wrong JSON type.
    WrongType {
        /// The field's name.
        field: &'static str,
        /// The type the field must hold.
        expected: &'static str,
    },
    /// A digest field holds text that is not a valid digest.
    InvalidDigest {
        /// The field's name.
        field: &'static str,
        /// The digest parse failure.
        source: DigestError,
    },
    /// A status field names no known task status.
    UnknownStatus {
        /// The status string found.
        found: String,
    },
    /// A numeric field holds a value outside its domain: a
    /// negative or fractional byte length, or an exit code outside
    /// the `i32` range.
    InvalidNumber {
        /// The field's name.
        field: &'static str,
        /// What the number must satisfy and what was found.
        detail: String,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFormat { found } => {
                write!(
                    f,
                    "manifest format is {found:?}, expected \"verirun-manifest/1\""
                )
            }
            Self::MissingField { field } => write!(f, "manifest field {field:?} is missing"),
            Self::WrongType { field, expected } => {
                write!(f, "manifest field {field:?} must be {expected}")
            }
            Self::InvalidDigest { field, source } => {
                write!(
                    f,
                    "manifest field {field:?} is not a valid digest: {source}"
                )
            }
            Self::UnknownStatus { found } => write!(f, "unknown task status {found:?}"),
            Self::InvalidNumber { field, detail } => {
                write!(
                    f,
                    "manifest field {field:?} holds an invalid number: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for ManifestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidDigest { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// A manifest verification failure (docs/ARCHITECTURE.md
/// section 9.3).
///
/// Every variant carries the expected and actual digests, and the
/// referencing task and path where a blob is involved. The CLI
/// maps every variant to exit code 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// The requested manifest digest names no object in the store.
    ManifestMissing {
        /// The requested manifest digest.
        digest: Digest,
    },
    /// The manifest blob does not hash to the requested digest.
    ManifestDigestMismatch {
        /// The requested digest.
        expected: Digest,
        /// The digest the blob actually hashes to.
        actual: Digest,
    },
    /// The manifest bytes are not the canonical encoding of their
    /// own content.
    NonCanonicalManifest {
        /// What re-encoding changed.
        detail: String,
    },
    /// The manifest bytes do not parse into a manifest.
    MalformedManifest {
        /// The manifest parse failure.
        source: ManifestError,
    },
    /// The stored Merkle root differs from the root recomputed
    /// from the record fields.
    RootMismatch {
        /// The root stored in the manifest.
        stored: Digest,
        /// The root recomputed from the records.
        recomputed: Digest,
    },
    /// A blob the manifest names is absent from the store.
    BlobMissing {
        /// The missing blob digest.
        digest: Digest,
        /// One task that references the blob.
        task_id: String,
        /// The artifact path that references it, or `stdout` or
        /// `stderr` for a log blob.
        path: String,
    },
    /// A blob the manifest names does not hash to its digest.
    BlobMismatch {
        /// The digest the manifest names.
        expected: Digest,
        /// The digest the content actually hashes to.
        actual: Digest,
        /// One task that references the blob.
        task_id: String,
        /// The artifact path that references it, or `stdout` or
        /// `stderr` for a log blob.
        path: String,
    },
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestMissing { digest } => {
                write!(f, "no manifest object {digest} in the store")
            }
            Self::ManifestDigestMismatch { expected, actual } => write!(
                f,
                "manifest digest mismatch: requested {expected}, content hashes to {actual}"
            ),
            Self::NonCanonicalManifest { detail } => {
                write!(f, "manifest is not in canonical form: {detail}")
            }
            Self::MalformedManifest { source } => write!(f, "malformed manifest: {source}"),
            Self::RootMismatch { stored, recomputed } => write!(
                f,
                "merkle root mismatch: stored {stored}, recomputed {recomputed}"
            ),
            Self::BlobMissing {
                digest,
                task_id,
                path,
            } => write!(
                f,
                "blob {digest} referenced by task {task_id:?} at {path:?} is missing"
            ),
            Self::BlobMismatch {
                expected,
                actual,
                task_id,
                path,
            } => write!(
                f,
                "blob mismatch for task {task_id:?} at {path:?}: expected {expected}, content hashes to {actual}"
            ),
        }
    }
}

impl std::error::Error for VerifyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MalformedManifest { source } => Some(source),
            _ => None,
        }
    }
}
