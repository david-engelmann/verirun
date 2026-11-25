//! The `unpin` command (docs/ARCHITECTURE.md sections 10 and
//! 12.1).
//!
//! Removes the pin file for the named manifest digest if one
//! exists. Unpinning does not verify the manifest first, so an
//! operator can always clear a stale or damaged pin that is
//! blocking garbage collection. Removing a pin that does not
//! exist is idempotent.

use std::path::Path;

use verirun_core::digest::Digest;
use verirun_store::cas_fs::Cas;

use crate::CliError;

/// Removes the pin for `digest` in the store at `cas_root`.
///
/// # Errors
///
/// Returns [`CliError::Store`] when the store cannot be opened
/// or an existing pin file cannot be removed.
///
/// # Panics
///
/// Never panics in the implementation.
pub fn unpin(cas_root: &Path, digest: &Digest) -> Result<(), CliError> {
    let cas = Cas::open(cas_root).map_err(|source| CliError::Store { source })?;
    cas.unpin_manifest(digest)
        .map_err(|source| CliError::Store { source })
}
