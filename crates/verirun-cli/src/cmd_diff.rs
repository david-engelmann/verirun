//! The `diff` command (docs/ARCHITECTURE.md sections 9.4 and
//! 12.1).
//!
//! Verifies both manifests under section 9.3 before comparing
//! anything: a side that fails verification is an integrity
//! failure (exit 2), never a mere difference. Comparison then
//! follows the pure core rule of
//! [`verirun_core::manifest::diff_manifests`].

use std::path::Path;

use verirun_core::digest::Digest;
use verirun_core::manifest::TaskDiff;
use verirun_store::cas_fs::Cas;

use crate::{CliError, verify_manifest};

/// The outcome of a `diff`: both manifests verified, plus the
/// differences between them in task id order. An empty
/// difference list means the runs are equivalent in every
/// compared field.
#[derive(Debug)]
pub struct DiffOutcome {
    /// The first manifest's digest, as requested.
    pub first: Digest,
    /// The second manifest's digest, as requested.
    pub second: Digest,
    /// The differences, in task id order.
    pub differences: Vec<TaskDiff>,
}

/// Verifies and compares the manifests named by `first` and
/// `second` in the store at `cas_root`.
///
/// # Errors
///
/// Returns [`CliError::Verify`] when either side fails
/// verification and [`CliError::Store`] when the store cannot
/// be opened.
///
/// # Panics
///
/// Never panics in the implementation.
pub fn diff(cas_root: &Path, first: &Digest, second: &Digest) -> Result<DiffOutcome, CliError> {
    let cas = Cas::open(cas_root).map_err(|source| CliError::Store { source })?;
    let first_manifest =
        verify_manifest(&cas, first).map_err(|source| CliError::Verify { source })?;
    let second_manifest =
        verify_manifest(&cas, second).map_err(|source| CliError::Verify { source })?;
    let differences = verirun_core::manifest::diff_manifests(&first_manifest, &second_manifest);
    Ok(DiffOutcome {
        first: *first,
        second: *second,
        differences,
    })
}
