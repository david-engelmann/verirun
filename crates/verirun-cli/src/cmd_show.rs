//! The `show` command (docs/ARCHITECTURE.md sections 9.3
//! and 12.1).
//!
//! Displays a manifest's records, but only after the
//! verification algorithm of section 9.3 has passed over it
//! in full (INV-CLI-5): the command is `verify` with a
//! rendering, never a raw read of the manifest blob, so an
//! unverified manifest is never presented as fact.

use std::path::Path;

use verirun_core::digest::Digest;
use verirun_core::manifest::Manifest;

use crate::CliError;

/// Verifies the manifest named by `digest` against the store
/// at `cas_root` and returns it for display.
///
/// # Errors
///
/// Returns [`CliError::Verify`] for the first verification
/// check that fails and [`CliError::Store`] when the store
/// cannot be opened. A manifest that fails any check is
/// never returned, so nothing downstream can render it.
///
/// # Panics
///
/// Never panics in the implementation.
pub fn show(cas_root: &Path, digest: &Digest) -> Result<Manifest, CliError> {
    crate::cmd_verify::verify(cas_root, digest)
}
