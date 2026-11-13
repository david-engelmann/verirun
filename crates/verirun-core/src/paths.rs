//! Sandbox relative path validation (docs/ARCHITECTURE.md section 8.4).
//!
//! Pure checks over declared path strings. The same validation gates
//! task inputs, task outputs, and sandbox relative executable strings,
//! so a path means the same thing at every consumer. The rules admit
//! no judgement calls:
//!
//! 1. The path is not empty.
//! 2. The path is relative. A leading forward slash and a Windows
//!    drive prefix (one ASCII letter and a colon) are both absolute
//!    spellings and are rejected, so a spec cannot change meaning
//!    between hosts.
//! 3. The path contains no backslash. The backslash is a separator
//!    spelling on other platforms; a verirun path has exactly one
//!    separator spelling, the forward slash, and any other spelling
//!    is rejected rather than reinterpreted.
//! 4. No component is `..`. Any parent component is rejected, not
//!    only escaping ones, so the rule never has to decide how far a
//!    path travels.
//! 5. Dot components and empty components (from repeated or leading
//!    separators that survive rule 2) are normalized away. If
//!    nothing remains, the path names no file and is rejected as
//!    empty.
//!
//! The normalized form joins the surviving components with single
//! forward slashes. Two spellings of one path have the same
//! normalized form, which is what collision checks, input binding,
//! and artifact records all use.

use std::fmt;

use crate::error::{PathError, PathErrorKind};

/// A declared path that passed validation, in normalized form.
///
/// The wrapped string is the components of the declared path joined
/// by single forward slashes, with dot and empty components removed.
/// It contains at least one component, no `..` component, no
/// backslash, and no leading separator.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NormalizedPath(String);

impl NormalizedPath {
    /// The normalized path text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the path and returns the normalized text.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for NormalizedPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for NormalizedPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Validates one declared path and returns its normalized form.
///
/// The checks run in the order stated in the module documentation,
/// so the reported rule is deterministic for a given path string.
///
/// # Errors
///
/// Returns [`PathError`] naming the declared path and the rule it
/// broke: [`crate::error::PathErrorKind::Empty`],
/// [`crate::error::PathErrorKind::Absolute`],
/// [`crate::error::PathErrorKind::BackslashSeparator`], or
/// [`crate::error::PathErrorKind::ParentTraversal`].
///
/// # Panics
///
/// Never panics in the core implementation: every failure is a
/// [`PathError`].
pub fn validate_path(declared: &str) -> Result<NormalizedPath, PathError> {
    if declared.is_empty() {
        return Err(PathError::new(
            PathErrorKind::Empty,
            declared.to_string(),
            "the path is empty".to_string(),
        ));
    }
    let bytes = declared.as_bytes();
    if declared.starts_with('/')
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
    {
        return Err(PathError::new(
            PathErrorKind::Absolute,
            declared.to_string(),
            "the path is absolute; sandbox paths are relative".to_string(),
        ));
    }
    if declared.contains('\\') {
        return Err(PathError::new(
            PathErrorKind::BackslashSeparator,
            declared.to_string(),
            "the path contains a backslash; the forward slash is the only separator spelling"
                .to_string(),
        ));
    }
    let mut components: Vec<&str> = Vec::new();
    for component in declared.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                return Err(PathError::new(
                    PathErrorKind::ParentTraversal,
                    declared.to_string(),
                    "the path contains a parent component".to_string(),
                ));
            }
            _ => components.push(component),
        }
    }
    if components.is_empty() {
        return Err(PathError::new(
            PathErrorKind::Empty,
            declared.to_string(),
            "the path names no file after normalization".to_string(),
        ));
    }
    Ok(NormalizedPath(components.join("/")))
}
