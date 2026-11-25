//! The verirun command library: the run pipeline and manifest
//! verification as callable functions, with the binary in
//! `main.rs` as a thin shell over them.
//!
//! Boundary rules (docs/ARCHITECTURE.md section 2.4): this crate
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

pub mod cmd_diff;
pub mod cmd_gc;
pub mod cmd_pin;
pub mod cmd_run;
pub mod cmd_show;
pub mod cmd_unpin;
pub mod cmd_verify;
pub mod spec_io;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use verirun_core::dag::validate_run_spec;
use verirun_core::digest::Digest;
use verirun_core::error::{DagError, ParseError, VerifyError};
use verirun_core::manifest::Manifest;
use verirun_core::parse::parse;
use verirun_store::cas_fs::{Cas, NoopReadObserver, ReadObserver};
use verirun_store::error::{GcError, RunError, StoreError};
use verirun_store::scheduler::{self, RunOutcome};
use verirun_store::sha256::Sha256Factory;

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
    /// The run file could not be read (exit 3).
    SpecIo {
        /// The run file path.
        path: PathBuf,
        /// The read failure.
        source: std::io::Error,
    },
    /// A store failure outside a run: opening the store for
    /// `verify`, `diff`, or `gc`. Integrity and missing object
    /// failures exit 2; layout and configuration failures exit
    /// 3.
    Store {
        /// The store failure.
        source: StoreError,
    },
    /// Manifest verification failed (exit 2).
    Verify {
        /// The verification failure.
        source: VerifyError,
    },
    /// Garbage collection failed. A pin whose manifest is
    /// missing or invalid is an integrity failure: exit 2. A
    /// held lock and other failures exit 3.
    Gc {
        /// The collection failure.
        source: GcError,
    },
    /// A bound host input file could not be opened or is not a
    /// regular file (exit 3).
    InputIo {
        /// The host file path.
        path: PathBuf,
        /// The underlying I/O failure.
        source: std::io::Error,
    },
    /// A command-line argument or configuration value is
    /// malformed (exit 3).
    Argument {
        /// The human-readable diagnostic detail.
        detail: String,
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
    /// Never panics in the implementation.
    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::Parse { .. }
            | Self::Dag { .. }
            | Self::SpecIo { .. }
            | Self::InputIo { .. }
            | Self::Argument { .. } => 3,
            Self::Run { source } => match source {
                RunError::Store {
                    source: StoreError::Integrity { .. } | StoreError::Missing { .. },
                } => 2,
                RunError::Store { .. }
                | RunError::MissingInput { .. }
                | RunError::WorkLocked { .. } => 3,
            },
            Self::Store { source } => match source {
                StoreError::Integrity { .. } | StoreError::Missing { .. } => 2,
                _ => 3,
            },
            Self::Verify { .. } => 2,
            Self::Gc { source } => match source {
                GcError::MissingPinnedManifest { .. }
                | GcError::InvalidPinnedManifest { .. }
                | GcError::Store {
                    source: StoreError::Integrity { .. } | StoreError::Missing { .. },
                } => 2,
                GcError::LockHeld { .. } | GcError::Store { .. } => 3,
            },
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse { source } => write!(f, "{source}"),
            Self::Dag { source } => write!(f, "{source}"),
            Self::Run { source } => write!(f, "{source}"),
            Self::SpecIo { path, source } => {
                write!(f, "cannot read the run file {}: {source}", path.display())
            }
            Self::Store { source } => write!(f, "{source}"),
            Self::Verify { source } => write!(f, "{source}"),
            Self::Gc { source } => write!(f, "{source}"),
            Self::InputIo { path, source } => {
                write!(
                    f,
                    "cannot read the bound input file {}: {source}",
                    path.display()
                )
            }
            Self::Argument { detail } => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse { source } => Some(source),
            Self::Dag { source } => Some(source),
            Self::Run { source } => Some(source),
            Self::SpecIo { source, .. } | Self::InputIo { source, .. } => Some(source),
            Self::Store { source } => Some(source),
            Self::Verify { source } => Some(source),
            Self::Gc { source } => Some(source),
            Self::Argument { .. } => None,
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

impl From<StoreError> for CliError {
    fn from(source: StoreError) -> Self {
        Self::Store { source }
    }
}

impl From<VerifyError> for CliError {
    fn from(source: VerifyError) -> Self {
        Self::Verify { source }
    }
}

impl From<GcError> for CliError {
    fn from(source: GcError) -> Self {
        Self::Gc { source }
    }
}

/// Renders an error as the `--json` failure object (INV-CLI-2).
///
/// The object always carries the same human readable `error`
/// text the text form prints, the `exit_code` the process will
/// use, and, for verification failures, the same digests and
/// referencing task and path the text form names, as structured
/// fields. The JSON form is therefore never the only place a
/// failure is explained, and never explains less than the text.
#[must_use]
pub fn error_json(error: &CliError) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    object.insert(
        "error".to_string(),
        serde_json::Value::String(error.to_string()),
    );
    object.insert(
        "exit_code".to_string(),
        serde_json::Value::Number(error.exit_code().into()),
    );
    if let CliError::Verify { source } = error {
        match source {
            VerifyError::ManifestMissing { digest } => {
                object.insert("kind".to_string(), "manifest_missing".into());
                object.insert("digest".to_string(), digest.to_hex().into());
            }
            VerifyError::ManifestUnreadable { digest, detail } => {
                object.insert("kind".to_string(), "manifest_unreadable".into());
                object.insert("digest".to_string(), digest.to_hex().into());
                object.insert("detail".to_string(), detail.clone().into());
            }
            VerifyError::ManifestDigestMismatch { expected, actual } => {
                object.insert("kind".to_string(), "manifest_digest_mismatch".into());
                object.insert("expected".to_string(), expected.to_hex().into());
                object.insert("actual".to_string(), actual.to_hex().into());
            }
            VerifyError::NonCanonicalManifest { detail } => {
                object.insert("kind".to_string(), "non_canonical_manifest".into());
                object.insert("detail".to_string(), detail.clone().into());
            }
            VerifyError::MalformedManifest { source } => {
                object.insert("kind".to_string(), "malformed_manifest".into());
                object.insert("detail".to_string(), source.to_string().into());
            }
            VerifyError::RootMismatch { stored, recomputed } => {
                object.insert("kind".to_string(), "root_mismatch".into());
                object.insert("stored".to_string(), stored.to_hex().into());
                object.insert("recomputed".to_string(), recomputed.to_hex().into());
            }
            VerifyError::BlobMissing {
                digest,
                task_id,
                path,
            } => {
                object.insert("kind".to_string(), "blob_missing".into());
                object.insert("digest".to_string(), digest.to_hex().into());
                object.insert("task_id".to_string(), task_id.clone().into());
                object.insert("path".to_string(), path.clone().into());
            }
            VerifyError::BlobUnreadable {
                digest,
                task_id,
                path,
                detail,
            } => {
                object.insert("kind".to_string(), "blob_unreadable".into());
                object.insert("digest".to_string(), digest.to_hex().into());
                object.insert("task_id".to_string(), task_id.clone().into());
                object.insert("path".to_string(), path.clone().into());
                object.insert("detail".to_string(), detail.clone().into());
            }
            VerifyError::BlobMismatch {
                expected,
                actual,
                task_id,
                path,
            } => {
                object.insert("kind".to_string(), "blob_mismatch".into());
                object.insert("expected".to_string(), expected.to_hex().into());
                object.insert("actual".to_string(), actual.to_hex().into());
                object.insert("task_id".to_string(), task_id.clone().into());
                object.insert("path".to_string(), path.clone().into());
            }
            VerifyError::ArtifactSizeMismatch {
                digest,
                expected,
                actual,
                task_id,
                path,
            } => {
                object.insert("kind".to_string(), "artifact_size_mismatch".into());
                object.insert("digest".to_string(), digest.to_hex().into());
                object.insert("expected".to_string(), (*expected).into());
                object.insert("actual".to_string(), (*actual).into());
                object.insert("task_id".to_string(), task_id.clone().into());
                object.insert("path".to_string(), path.clone().into());
            }
        }
    }
    serde_json::Value::Object(object)
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
/// Never panics in the implementation.
pub fn run_spec_bytes(
    spec_bytes: &[u8],
    cas_root: &Path,
    work_root: &Path,
    inputs: &BTreeMap<String, Digest>,
    jobs: usize,
) -> Result<RunOutcome, CliError> {
    let value = parse(spec_bytes)?;
    let dag = validate_run_spec(&value)?;
    let cas = Cas::open(cas_root).map_err(RunError::from)?;
    let outcome = scheduler::run(&cas, &Sha256Factory, work_root, &dag, inputs, jobs)?;
    Ok(outcome)
}

/// Verifies the manifest named by `expected` against `cas`.
///
/// The checks follow section 9.3 in order: the manifest blob
/// re-hash, the canonical re-encode check, the Merkle root
/// recomputation, and one re-hash of each distinct blob the
/// manifest names. Every object read is reported to `observer`
/// (see [`ReadObserver`]); [`verify_manifest`] is this function
/// with a no-op observer.
///
/// # Errors
///
/// Returns [`VerifyError`] describing the first failed check,
/// with the expected and actual digests and one referencing task
/// and path wherever a blob is involved.
///
/// # Panics
///
/// Never panics in the implementation.
pub fn verify_manifest_observed(
    cas: &Cas,
    expected: &Digest,
    observer: &mut dyn ReadObserver,
) -> Result<Manifest, VerifyError> {
    // Check 1: the manifest blob re-hash. The store's verified
    // read does the hashing; its outcomes map onto the first
    // variants: corruption is a digest mismatch, absence is
    // missing, and any other store failure is an unreadable
    // manifest, reported with the failure itself so an access
    // problem is never mistaken for a missing object.
    let bytes = match cas.read_to_vec_observed(expected, observer) {
        Ok(bytes) => bytes,
        Err(StoreError::Integrity { actual, .. }) => {
            return Err(VerifyError::ManifestDigestMismatch {
                expected: *expected,
                actual,
            });
        }
        Err(StoreError::Missing { .. }) => {
            return Err(VerifyError::ManifestMissing { digest: *expected });
        }
        Err(source) => {
            return Err(VerifyError::ManifestUnreadable {
                digest: *expected,
                detail: source.to_string(),
            });
        }
    };

    // Check 2: the canonical re-encode. Bytes that do not parse
    // have no canonical form; a value that parses but does not
    // read as a manifest is malformed; a manifest whose stored
    // bytes differ from the canonical encoding of its content is
    // non-canonical.
    let value = parse(&bytes).map_err(|source| VerifyError::NonCanonicalManifest {
        detail: format!("the manifest bytes do not parse: {source}"),
    })?;
    let manifest =
        Manifest::from_value(&value).map_err(|source| VerifyError::MalformedManifest { source })?;
    let reencoded =
        manifest
            .canonical_bytes()
            .map_err(|source| VerifyError::NonCanonicalManifest {
                detail: format!("re-encoding the manifest failed: {source}"),
            })?;
    if reencoded != bytes {
        return Err(VerifyError::NonCanonicalManifest {
            detail: "the stored bytes differ from the canonical encoding of their content"
                .to_string(),
        });
    }

    // Check 3: the Merkle root recomputation.
    let recomputed = manifest.recompute_root(cas.factory());
    if recomputed != manifest.merkle_root {
        return Err(VerifyError::RootMismatch {
            stored: manifest.merkle_root,
            recomputed,
        });
    }

    // Check 4: one re-hash of each distinct named blob, in
    // first-reference order: for each record in stored order,
    // its artifacts in path order, then its stdout, then its
    // stderr (INV-LAT-3). The reads run in parallel (see
    // read_references); the observer is notified once per
    // reference in that same order, and the first failure in
    // that order is the one reported, so the verdict and its
    // wording are exactly what a sequential pass produces.
    let references = collect_references(&manifest);
    for (digest, _, _) in &references {
        observer.on_read(digest);
    }
    let outcomes = read_references(cas, &references);
    let mut verified_sizes: BTreeMap<Digest, u64> = BTreeMap::new();
    for ((digest, task_id, path), outcome) in references.iter().zip(outcomes) {
        match outcome {
            Ok(byte_len) => {
                verified_sizes.insert(*digest, byte_len);
            }
            Err(StoreError::Integrity { actual, .. }) => {
                return Err(VerifyError::BlobMismatch {
                    expected: *digest,
                    actual,
                    task_id: task_id.clone(),
                    path: path.clone(),
                });
            }
            Err(StoreError::Missing { .. }) => {
                return Err(VerifyError::BlobMissing {
                    digest: *digest,
                    task_id: task_id.clone(),
                    path: path.clone(),
                });
            }
            Err(source) => {
                return Err(VerifyError::BlobUnreadable {
                    digest: *digest,
                    task_id: task_id.clone(),
                    path: path.clone(),
                    detail: source.to_string(),
                });
            }
        }
    }
    check_artifact_sizes(&manifest, &verified_sizes)?;
    Ok(manifest)
}

/// Collects distinct blob references across `manifest` in
/// first-reference order (artifacts in path order, then stdout,
/// then stderr for each task record).
fn collect_references(manifest: &Manifest) -> Vec<(Digest, String, String)> {
    let mut references: Vec<(Digest, String, String)> = Vec::new();
    for record in &manifest.tasks {
        for artifact in &record.artifacts {
            push_reference(
                &mut references,
                artifact.digest,
                &record.task_id,
                &artifact.path,
            );
        }
        push_reference(
            &mut references,
            record.stdout_digest,
            &record.task_id,
            "stdout",
        );
        push_reference(
            &mut references,
            record.stderr_digest,
            &record.task_id,
            "stderr",
        );
    }
    references
}

/// Verifies that every `ArtifactRecord.bytes` in `manifest`
/// equals the verified byte count of its referenced blob.
fn check_artifact_sizes(
    manifest: &Manifest,
    verified_sizes: &BTreeMap<Digest, u64>,
) -> Result<(), VerifyError> {
    for record in &manifest.tasks {
        for artifact in &record.artifacts {
            let actual = verified_sizes[&artifact.digest];
            if actual != artifact.bytes {
                return Err(VerifyError::ArtifactSizeMismatch {
                    digest: artifact.digest,
                    expected: artifact.bytes,
                    actual,
                    task_id: record.task_id.clone(),
                    path: artifact.path.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Verifies one referenced blob by streaming it to a sink and
/// returning its verified byte length, so no whole blob is ever
/// held in memory on this path.
fn verify_reference(cas: &Cas, digest: &Digest) -> Result<u64, StoreError> {
    cas.copy_to(digest, std::io::sink())
}

/// Reads every referenced blob and returns one outcome per
/// reference, in reference order. With at least eight
/// references the reads spread over scoped worker threads;
/// below that the pass is sequential, exactly as it always
/// was. A worker that panics has its chunk re-read on the
/// calling thread, so this function never panics either.
fn read_references(
    cas: &Cas,
    references: &[(Digest, String, String)],
) -> Vec<Result<u64, StoreError>> {
    const PARALLEL_THRESHOLD: usize = 8;
    if references.len() < PARALLEL_THRESHOLD {
        return references
            .iter()
            .map(|(digest, _, _)| verify_reference(cas, digest))
            .collect();
    }
    let workers = std::thread::available_parallelism()
        .map_or(2, std::num::NonZero::get)
        .min(8);
    let chunk_len = references.len().div_ceil(workers);
    let mut outcomes: Vec<Result<u64, StoreError>> = Vec::with_capacity(references.len());
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for part in references.chunks(chunk_len) {
            handles.push(scope.spawn(move || {
                part.iter()
                    .map(|(digest, _, _)| verify_reference(cas, digest))
                    .collect::<Vec<_>>()
            }));
        }
        for (index, handle) in handles.into_iter().enumerate() {
            if let Ok(part) = handle.join() {
                outcomes.extend(part);
            } else {
                let start = index * chunk_len;
                let end = (start + chunk_len).min(references.len());
                outcomes.extend(
                    references[start..end]
                        .iter()
                        .map(|(digest, _, _)| verify_reference(cas, digest)),
                );
            }
        }
    });
    outcomes
}

/// Verifies the manifest named by `expected` against `cas`.
///
/// This is [`verify_manifest_observed`] with a no-op observer.
///
/// # Errors
///
/// Returns the [`VerifyError`] of [`verify_manifest_observed`].
pub fn verify_manifest(cas: &Cas, expected: &Digest) -> Result<Manifest, VerifyError> {
    verify_manifest_observed(cas, expected, &mut NoopReadObserver)
}

/// Appends one blob reference unless its digest was already
/// referenced, keeping first-reference order.
fn push_reference(
    references: &mut Vec<(Digest, String, String)>,
    digest: Digest,
    task_id: &str,
    path: &str,
) {
    if references.iter().any(|(seen, _, _)| *seen == digest) {
        return;
    }
    references.push((digest, task_id.to_string(), path.to_string()));
}
