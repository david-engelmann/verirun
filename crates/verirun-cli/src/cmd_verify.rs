//! The `verify` command (docs/ARCHITECTURE.md sections 9.3
//! and 12.1).
//!
//! Opens the store and runs the verification algorithm of
//! section 9.3 over the named manifest: blob re-hash,
//! canonical re-encode, Merkle root recomputation, and one
//! re-hash of each distinct named blob.

use std::path::Path;

use verirun_core::digest::Digest;
use verirun_core::manifest::Manifest;
use verirun_store::cas_fs::Cas;

use crate::{CliError, verify_manifest};

/// Verifies the manifest named by `digest` against the store
/// at `cas_root`, returning the verified manifest on success.
///
/// # Errors
///
/// Returns [`CliError::Verify`] for the first verification
/// check that fails and [`CliError::Store`] when the store
/// cannot be opened.
///
/// # Panics
///
/// Never panics in the implementation.
pub fn verify(cas_root: &Path, digest: &Digest) -> Result<Manifest, CliError> {
    let cas = Cas::open(cas_root).map_err(|source| CliError::Store { source })?;
    verify_manifest(&cas, digest).map_err(|source| CliError::Verify { source })
}
