//! Hermetic subprocess execution (docs/ARCHITECTURE.md section 8).
//!
//! The runner owns everything about one task's process: building
//! its environment from nothing, materializing its inputs with
//! verification during the copy, resolving its executable,
//! spawning it with two concurrent drain threads that stream its
//! output into the store as raw bytes, enforcing its timeout, and
//! collecting exactly its declared outputs under the component
//! walk of section 8.4.
//!
//! Outcomes that belong to the task (a nonzero exit, death by
//! signal, a spawn failure, a missing or linked or non-regular
//! output) are reported in [`TaskExecution`], never as errors.
//! [`RunError`] is reserved for what aborts a run: store failures,
//! including the corrupted input blob that materialization finds
//! before the task starts (INV-SBX-5).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use verirun_core::dag::TaskSpec;
use verirun_core::digest::Digest;
use verirun_core::manifest::ArtifactRecord;
use verirun_core::paths::NormalizedPath;

use crate::cas_fs::Cas;
use crate::error::RunError;

/// The fixed default environment pairs of section 8.1, sorted by
/// key.
///
/// A spec that declares any of these keys is rejected at
/// validation (INV-DAG-6); the runner sets them unconditionally,
/// because fixed means fixed (INV-RUN-2).
pub const FIXED_ENV: [(&str, &str); 3] =
    [("LC_ALL", "C"), ("SOURCE_DATE_EPOCH", "0"), ("TZ", "UTC")];

/// The raw outcome of one task's execution, before the scheduler
/// assigns a status.
///
/// `exit_code` is absent when the child died by signal, when it
/// was killed at its deadline, and when it never spawned.
/// `artifacts` is empty unless the child exited with code 0 and
/// collection succeeded; a task that fails keeps none of its
/// outputs. `failure_detail` explains a task-level failure that
/// the exit code alone cannot: an unresolvable executable, a
/// spawn failure, a missing or linked or non-regular output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskExecution {
    /// The child exit code, when the child exited with one.
    pub exit_code: Option<i32>,
    /// Whether the deadline fired and the child was killed.
    pub timed_out: bool,
    /// The blob digest of the captured stdout stream. The empty
    /// blob digest when the child never spawned.
    pub stdout_digest: Digest,
    /// The blob digest of the captured stderr stream.
    pub stderr_digest: Digest,
    /// The collected artifacts, sorted by path.
    pub artifacts: Vec<ArtifactRecord>,
    /// A human-readable account of a task-level failure.
    pub failure_detail: Option<String>,
}

/// The executor for one store. Holds the `PATH` snapshot taken at
/// construction, which bare executable names resolve against
/// (section 8.2). The snapshot is a host detail and enters no
/// digest.
pub struct Runner<'a> {
    cas: &'a Cas,
    path_dirs: Vec<PathBuf>,
}

impl<'a> Runner<'a> {
    /// Creates a runner over `cas`, snapshotting the current
    /// process `PATH` for bare name resolution.
    #[must_use]
    pub fn new(cas: &'a Cas) -> Self {
        let path_dirs = std::env::var_os("PATH")
            .map(|value| std::env::split_paths(&value).collect())
            .unwrap_or_default();
        Self { cas, path_dirs }
    }

    /// Builds the child's full environment, sorted by key: the
    /// declared pairs, the fixed defaults of [`FIXED_ENV`], and
    /// `TMPDIR` naming the task's private temporary directory
    /// inside its sandbox. Nothing else is present (INV-RUN-1).
    /// `TMPDIR` is host-specific, so it is excluded from the
    /// environment digest (section 5.3); this list is the spawn
    /// environment, not the digest input.
    ///
    /// # Panics
    ///
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    #[must_use]
    pub fn build_environment(
        declared: &BTreeMap<String, String>,
        tmpdir: &Path,
    ) -> Vec<(String, String)> {
        let _ = (declared, tmpdir);
        panic!("verirun Milestone 2 Stage A: build_environment is not yet implemented")
    }

    /// Resolves the executable string of section 8.2. A string
    /// containing a path separator is interpreted relative to
    /// `sandbox` and must pass the path rules of section 8.4. A
    /// bare name is resolved against the `PATH` snapshot.
    ///
    /// # Errors
    ///
    /// Returns a failure detail string when the string names no
    /// executable file: resolution failure is a task outcome, so
    /// it is reported as text for [`TaskExecution::failure_detail`],
    /// not as a [`RunError`].
    ///
    /// # Panics
    ///
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    pub fn resolve_executable(&self, executable: &str, sandbox: &Path) -> Result<PathBuf, String> {
        let _ = (executable, sandbox);
        let _ = &self.path_dirs;
        panic!("verirun Milestone 2 Stage A: resolve_executable is not yet implemented")
    }

    /// Executes one task in `sandbox`, creating the sandbox and
    /// its `inputs`, `outputs`, and `tmp` areas.
    ///
    /// `inputs` pairs each normalized declared input path with the
    /// blob digest to materialize. Materialization copies through
    /// the store's verified read, walking every destination
    /// component for symlinks before writing any bytes
    /// (section 8.4 rule 5).
    ///
    /// # Errors
    ///
    /// Returns [`RunError`] when the store fails, including the
    /// integrity failure a corrupted input blob raises; the task
    /// is then never spawned and the whole run aborts
    /// (INV-SBX-5). Task-level failures are returned inside
    /// [`TaskExecution`], not as errors.
    ///
    /// # Panics
    ///
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    pub fn execute(
        &self,
        task: &TaskSpec,
        inputs: &[(NormalizedPath, Digest)],
        sandbox: &Path,
    ) -> Result<TaskExecution, RunError> {
        let _ = (task, inputs, sandbox);
        let _ = self.cas;
        panic!("verirun Milestone 2 Stage A: Runner::execute is not yet implemented")
    }
}
