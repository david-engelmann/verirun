//! The worker pool scheduler (docs/ARCHITECTURE.md section 7.3).
//!
//! The scheduler executes a [`ValidatedDag`] over a fixed pool of
//! workers. A task becomes ready when all of its dependencies have
//! status Succeeded or Cached; the ready set is ordered by task id
//! and workers always take the lexicographically smallest ready
//! task, so start order is deterministic even though finish order
//! is not. Outcomes land in a map keyed by task id and the
//! manifest is built from that map in key order, so finish order
//! has no path into the manifest (INV-RUN-5).
//!
//! Input binding: a task's declared input resolves to the artifact
//! a dependency produced under the same normalized path, or to the
//! caller's binding in `inputs` when no task in the run produces
//! the path. An input that resolves nowhere aborts the run with
//! [`RunError::MissingInput`] before any task executes.
//!
//! Failure propagation: when a task ends `Failed` or `TimedOut`,
//! every transitive dependent is recorded `Skipped` without
//! starting. A skipped record still carries its execution digest:
//! inputs that resolved (from a succeeded producer or a caller
//! binding) frame normally, and an input whose producer failed or
//! was skipped has no content, so it frames with the blob digest
//! of the empty byte string, the same digest section 3.3 assigns
//! to a skipped task's own streams. Recording a skipped task puts
//! the empty blob into the store through the normal put protocol
//! first, so every digest a skipped record names exists in the
//! store the run produced (INV-RUN-6).
//!
//! Cache: before executing a ready task the scheduler looks its
//! execution digest up in the store's `by-exec/` index. The entry
//! is reused only when the recorded task record and every blob it
//! names still exist and re-hash correctly; a missing or corrupt
//! target is ignored and the task executes (INV-RUN-7). After a
//! task succeeds, its execution digest to record digest mapping
//! is written with the same atomic discipline as the put
//! protocol.

use std::collections::BTreeMap;
use std::path::Path;

use verirun_core::dag::ValidatedDag;
use verirun_core::digest::{Digest, HasherFactory};
use verirun_core::manifest::Manifest;

use crate::cas_fs::Cas;
use crate::error::RunError;

/// The result of one completed run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    /// The run's manifest: one record per spec task, sorted by
    /// task id.
    pub manifest: Manifest,
    /// The manifest digest: the blob digest of the manifest's
    /// canonical bytes, under which the manifest itself was
    /// stored.
    pub manifest_digest: Digest,
    /// The run's exit code (section 12.2): 0 when every task
    /// succeeded or was cached, 1 when any task failed, timed
    /// out, or was skipped through propagation.
    pub exit_code: i32,
    /// Per-task failure details for tasks whose records show a
    /// failure the exit code cannot explain (spawn failures,
    /// collection failures). Diagnostics only; never digested.
    pub failures: BTreeMap<String, String>,
}

/// Executes one validated run to completion.
///
/// `inputs` binds normalized input paths that no task in the run
/// produces to the blob digests to materialize. `jobs` sizes the
/// worker pool; 0 selects the available parallelism. Worker count
/// is an execution detail only: it appears in no digest and no
/// record, and the manifest is byte-identical for any count
/// (INV-RUN-5).
///
/// The manifest blob is stored in `cas` before this returns, and
/// every blob any record names is present in `cas`.
///
/// # Errors
///
/// Returns [`RunError::MissingInput`] before any task executes
/// when a declared input resolves to no producer and no caller
/// binding. Returns [`RunError::Store`] when the store fails,
/// including the integrity failure a corrupted input blob raises
/// during materialization, which aborts the run before the
/// consuming task starts.
///
/// # Panics
///
/// Panics in Milestone 2 Stage A: the interface is staged ahead
/// of its implementation, which lands in Stage B.
pub fn run(
    cas: &Cas,
    factory: &dyn HasherFactory,
    work_root: &Path,
    dag: &ValidatedDag,
    inputs: &BTreeMap<String, Digest>,
    jobs: usize,
) -> Result<RunOutcome, RunError> {
    let _ = (cas, factory, work_root, dag, inputs, jobs);
    panic!("verirun Milestone 2 Stage A: scheduler::run is not yet implemented")
}
