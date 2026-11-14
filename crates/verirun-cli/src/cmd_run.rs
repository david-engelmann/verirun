//! The `run` command (docs/ARCHITECTURE.md section 12.1).
//!
//! Loads and validates the run file, opens the store, and
//! schedules the graph. The CLI surface binds no inputs: every
//! declared input must be produced by a dependency inside the
//! run, and an input nothing produces fails the run before any
//! task executes (section 7.3).

use std::path::Path;

use verirun_store::scheduler::RunOutcome;

use crate::CliError;

/// Runs the run file at `spec_path` against the store at
/// `cas_root`, with task sandboxes under `work_root`. `jobs`
/// sizes the worker pool; 0 selects the available parallelism.
///
/// # Errors
///
/// Returns [`CliError`] for an unreadable, unparsable, or
/// invalid run file and for a run that aborts. A run whose
/// tasks merely fail is not an error here: its outcome carries
/// the exit code 1 and the per-task failure details.
///
/// # Panics
///
/// Never panics in the implementation.
pub fn run(
    spec_path: &Path,
    cas_root: &Path,
    work_root: &Path,
    jobs: usize,
) -> Result<RunOutcome, CliError> {
    // Stage A stub: the pipeline composition lands in Stage B.
    let _ = cas_root;
    let _ = work_root;
    let _ = jobs;
    Err(CliError::SpecIo {
        path: spec_path.to_path_buf(),
        source: std::io::Error::other("the run command lands in Milestone 3 Stage B"),
    })
}
