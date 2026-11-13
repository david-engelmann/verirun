//! The verirun command library: the run pipeline and manifest
//! verification as callable functions, with the binary in
//! `main.rs` as a thin shell over them.
//!
//! Boundary rules (docs/ARCHITECTURE.md section 2.3): this crate
//! never canonicalizes, hashes, or validates on its own. It hands
//! run file bytes to core parsing and validation, hands validated
//! graphs to the store scheduler, and renders outcomes. Run files
//! are read as bytes and parsed by core first, so a lenient
//! transport parser can never weaken validation.
//!
//! The pipeline order is itself an invariant (INV-LAT-4): parse
//! and validation complete before the store is opened, before any
//! sandbox is created, and before any process is spawned, so an
//! invalid run leaves the store and the work directory untouched.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::BTreeMap;
use std::path::Path;

use verirun_core::digest::Digest;
use verirun_core::error::{DagError, ParseError, VerifyError};
use verirun_core::manifest::Manifest;
use verirun_store::cas_fs::Cas;
use verirun_store::error::RunError;
use verirun_store::scheduler::RunOutcome;

/// An error at the command boundary. The exit code mapping of
/// section 12.2 is total over this type: every variant maps to
/// exactly one code through [`CliError::exit_code`].
#[derive(Debug)]
pub enum CliError {
    /// The run file bytes fail strict parsing (exit 3).
    Parse {
        /// The parse failure, with its byte offset.
        source: ParseError,
    },
    /// The parsed run file fails DAG validation (exit 3).
    Dag {
        /// The validation failure.
        source: DagError,
    },
    /// The run itself aborted. Store integrity and missing blob
    /// failures exit 2; caller binding failures and other store
    /// failures exit 3 (see [`CliError::exit_code`]).
    Run {
        /// The run failure.
        source: RunError,
    },
}

impl CliError {
    /// The exit code of section 12.2 for this error.
    ///
    /// Parse and validation failures are invalid input: 3. Inside
    /// a run, a store integrity failure or a missing blob is an
    /// integrity failure: 2, the same class as the verification
    /// failures of section 9.3. A caller binding failure (an input
    /// no task produced and no binding resolved) is an input
    /// error: 3. Any remaining store failure is a store
    /// configuration class failure: 3.
    ///
    /// # Panics
    ///
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        panic!("verirun Milestone 2 Stage A: CliError::exit_code is not yet implemented")
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse { source } => write!(f, "{source}"),
            Self::Dag { source } => write!(f, "{source}"),
            Self::Run { source } => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse { source } => Some(source),
            Self::Dag { source } => Some(source),
            Self::Run { source } => Some(source),
        }
    }
}

impl From<ParseError> for CliError {
    fn from(source: ParseError) -> Self {
        Self::Parse { source }
    }
}

impl From<DagError> for CliError {
    fn from(source: DagError) -> Self {
        Self::Dag { source }
    }
}

impl From<RunError> for CliError {
    fn from(source: RunError) -> Self {
        Self::Run { source }
    }
}

/// Runs one run file, given as raw bytes, end to end: strict
/// parse, DAG validation, then scheduled execution against the
/// store at `cas_root`, with sandboxes under `work_root`.
///
/// `inputs` binds normalized input paths that no task in the run
/// produces to the blob digests to materialize. `jobs` sizes the
/// worker pool; 0 selects the available parallelism.
///
/// Parse and validation complete before the store is opened, so
/// an invalid run leaves the store and the work directory
/// untouched (INV-LAT-4). On success the manifest blob is stored
/// in the store and the outcome, including the run's exit code,
/// is returned.
///
/// # Errors
///
/// Returns [`CliError::Parse`] for unparsable bytes,
/// [`CliError::Dag`] for a spec that fails validation, and
/// [`CliError::Run`] when the run aborts.
///
/// # Panics
///
/// Panics in Milestone 2 Stage A: the interface is staged ahead
/// of its implementation, which lands in Stage B.
pub fn run_spec_bytes(
    spec_bytes: &[u8],
    cas_root: &Path,
    work_root: &Path,
    inputs: &BTreeMap<String, Digest>,
    jobs: usize,
) -> Result<RunOutcome, CliError> {
    let _ = (spec_bytes, cas_root, work_root, inputs, jobs);
    panic!("verirun Milestone 2 Stage A: run_spec_bytes is not yet implemented")
}

/// Verifies the manifest named by `expected` against `cas`.
///
/// The checks follow section 9.3 in order: the manifest blob
/// re-hash, the canonical re-encode check, the Merkle root
/// recomputation, and one re-hash of each distinct blob the
/// manifest names.
///
/// # Errors
///
/// Returns [`VerifyError`] describing the first failed check,
/// with the expected and actual digests and one referencing task
/// and path wherever a blob is involved.
///
/// # Panics
///
/// Panics in Milestone 2 Stage A: the interface is staged ahead
/// of its implementation, which lands in Stage B.
pub fn verify_manifest(cas: &Cas, expected: &Digest) -> Result<Manifest, VerifyError> {
    let _ = (cas, expected);
    panic!("verirun Milestone 2 Stage A: verify_manifest is not yet implemented")
}
