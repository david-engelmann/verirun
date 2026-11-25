//! The `pin` command (docs/ARCHITECTURE.md sections 10 and
//! 12.1).
//!
//! Verifies the named manifest in full under section 9.3 before
//! writing its pin file under `pins/`, so an absent, malformed,
//! or tampered manifest can never be pinned and block future
//! garbage collection sweeps. Pinning an already pinned manifest
//! is idempotent.

use std::path::Path;

use verirun_core::digest::Digest;
use verirun_core::manifest::Manifest;
use verirun_store::cas_fs::Cas;

use crate::{CliError, verify_manifest};

/// Verifies the manifest named by `digest` in the store at
/// `cas_root` and records a pin preserving its reachable object
/// graph across garbage collection sweeps.
///
/// # Errors
///
/// Returns [`CliError::Verify`] when the manifest fails any
/// check of section 9.3 and [`CliError::Store`] when the store
/// cannot be opened or the pin file cannot be written.
///
/// # Panics
///
/// Never panics in the implementation.
pub fn pin(cas_root: &Path, digest: &Digest) -> Result<Manifest, CliError> {
    let cas = Cas::open(cas_root).map_err(|source| CliError::Store { source })?;
    let manifest = verify_manifest(&cas, digest).map_err(|source| CliError::Verify { source })?;
    cas.pin_manifest(digest)
        .map_err(|source| CliError::Store { source })?;
    Ok(manifest)
}
