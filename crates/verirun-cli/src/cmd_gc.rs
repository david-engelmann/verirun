//! The `gc` command (docs/ARCHITECTURE.md sections 10 and
//! 12.1).
//!
//! Opens the store and runs one collection with the grace
//! threshold from `--grace-secs` (default 3600). The command
//! reads the wall clock once, through the store's clock
//! module, and hands the instant to the collector.

use std::path::Path;

use verirun_store::cas_fs::Cas;
use verirun_store::clock;
use verirun_store::gc::GcReport;

use crate::CliError;

/// Collects garbage in the store at `cas_root`.
///
/// # Errors
///
/// Returns [`CliError::Gc`] when collection refuses or fails
/// and [`CliError::Store`] when the store cannot be opened.
///
/// # Panics
///
/// Never panics in the implementation.
pub fn gc(cas_root: &Path, grace_secs: u64, dry_run: bool) -> Result<GcReport, CliError> {
    let cas = Cas::open(cas_root).map_err(|source| CliError::Store { source })?;
    verirun_store::gc::collect(&cas, grace_secs, dry_run, clock::now())
        .map_err(|source| CliError::Gc { source })
}
