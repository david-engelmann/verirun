//! Garbage collection execution (docs/ARCHITECTURE.md
//! section 10).
//!
//! Collection runs in three phases. Mark: every pinned
//! manifest is read and verified, and the reachable set is
//! computed by the pure planner in
//! [`verirun_core::cas::reachable_objects`]. Plan: objects and
//! temporary files are aged against one wall clock read and
//! the grace threshold, again through core planning. Sweep:
//! under `gc.lock`, the planned files are deleted. A dry run
//! stops after the plan and deletes nothing (INV-GC-4).
//!
//! Collection is a maintenance operation outside the
//! determinism boundary: it changes what is stored, never what
//! a digest means.

use std::time::SystemTime;

use crate::cas_fs::Cas;
use crate::error::GcError;

/// The outcome of one collection: what was deleted, or on a
/// dry run what would be deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcReport {
    /// Whether this run stopped after planning and deleted
    /// nothing.
    pub dry_run: bool,
    /// How many objects were (or would be) deleted.
    pub objects: usize,
    /// The total byte size of those objects.
    pub object_bytes: u64,
    /// How many temporary files were (or would be) deleted.
    pub tmp_files: usize,
    /// The total byte size of those temporary files.
    pub tmp_bytes: u64,
    /// How many objects are reachable from pins and were
    /// therefore kept regardless of age.
    pub reachable: usize,
}

impl GcReport {
    /// An empty report for a run of the given kind.
    #[must_use]
    pub const fn new(dry_run: bool) -> Self {
        Self {
            dry_run,
            objects: 0,
            object_bytes: 0,
            tmp_files: 0,
            tmp_bytes: 0,
            reachable: 0,
        }
    }

    /// The total files deleted (or planned) across objects and
    /// temporary files.
    #[must_use]
    pub const fn files(&self) -> usize {
        self.objects + self.tmp_files
    }

    /// The total bytes reclaimed (or that would be reclaimed).
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.object_bytes + self.tmp_bytes
    }
}

/// Collects garbage in `cas` (section 10).
///
/// Every age in the plan derives from file modification times
/// measured against `now`, which the caller reads once through
/// [`crate::clock::now`]; passing an explicit `now` is what
/// lets tests age files deterministically. Objects and
/// temporary files older than `grace_secs` and unreachable
/// from any pin are deleted, unless `dry_run` is set, in which
/// case the plan is reported and nothing is deleted.
///
/// # Errors
///
/// Returns [`GcError::MissingPinnedManifest`] or
/// [`GcError::InvalidPinnedManifest`] when a pin's manifest
/// cannot be produced and verified, [`GcError::LockHeld`] when
/// another sweep holds the lock on a real run, and
/// [`GcError::Store`] for store failures. On every error the
/// sweep deletes nothing.
///
/// # Panics
///
/// Never panics in the implementation.
#[allow(
    clippy::missing_const_for_fn,
    reason = "the implementation performs filesystem I/O; it cannot be const"
)]
pub fn collect(
    cas: &Cas,
    grace_secs: u64,
    dry_run: bool,
    now: SystemTime,
) -> Result<GcReport, GcError> {
    // Stage A stub: the three phases land in Stage B.
    let _ = cas;
    let _ = grace_secs;
    let _ = now;
    Ok(GcReport::new(dry_run))
}
