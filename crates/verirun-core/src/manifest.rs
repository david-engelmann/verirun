//! Manifests, task records, and the Merkle construction
//! (docs/ARCHITECTURE.md sections 3.3 and 9).
//!
//! A manifest is the commitment a run leaves behind: one record per
//! task in the spec, sorted by task id, plus the Merkle root over
//! the task record digests. It is stored as a blob whose bytes are
//! the canonical encoding (section 4) of the value form defined in
//! section 9.1, so the manifest digest is the blob digest of those
//! bytes and a manifest carries no timestamps, host paths, user
//! names, or tool versions.
//!
//! The Merkle construction pairs adjacent digests level by level
//! under the `verirun/v1/node` tag. An odd element at any level is
//! promoted unchanged to the next level, never duplicated. A single
//! leaf is its own root, and zero tasks give the `verirun/v1/empty`
//! digest. This module builds manifests, renders and parses their
//! value form, and recomputes roots from record fields alone, which
//! is the core of the verification algorithm (section 9.3).

use crate::canonical;
use crate::digest::{self, Digest, HasherFactory, TaskStatus};
use crate::error::{CanonicalError, ManifestError};
use crate::value::Value;

/// The value of the manifest `format` field.
///
/// The format tag is the version marker: a future format change
/// mints a new tag and a new domain tag set, and version 1
/// manifests stay verifiable under version 1 rules.
pub const MANIFEST_FORMAT: &str = "verirun-manifest/1";

/// One produced file in a task record.
///
/// The record holds the file's sandbox relative path, the blob
/// digest of its content, its byte length, and whether any
/// execute bit was set on it at collection time. No timestamps,
/// owners, or host paths ever appear (INV-SBX-4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRecord {
    /// The normalized sandbox relative path.
    pub path: String,
    /// The blob digest of the file content.
    pub digest: Digest,
    /// The byte length of the content.
    pub bytes: u64,
    /// Whether any execute bit was set when the file was collected.
    pub executable: bool,
}

impl ArtifactRecord {
    /// The `verirun/v1/artifact` framing digest of this record.
    #[must_use]
    pub fn framing_digest(&self, factory: &dyn HasherFactory) -> Digest {
        digest::artifact_digest(
            factory,
            &self.path,
            &self.digest,
            self.bytes,
            self.executable,
        )
    }

    /// The value form of section 9.1: an object with `path`,
    /// `sha256`, `bytes`, and `executable` fields.
    ///
    /// # Panics
    ///
    /// Never panics in the core implementation.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "byte lengths are inside the safe integer domain by physical necessity (section 9.1), and the canonical encoder rejects any value outside it at encode time"
    )]
    pub fn to_value(&self) -> Value {
        Value::Object(vec![
            ("path".to_string(), Value::String(self.path.clone())),
            ("sha256".to_string(), Value::String(self.digest.to_hex())),
            ("bytes".to_string(), Value::Number(self.bytes as f64)),
            ("executable".to_string(), Value::Bool(self.executable)),
        ])
    }

    /// Parses the value form back into a record.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] when a field is missing, mistyped,
    /// or holds a digest that is not valid lowercase hex.
    ///
    /// # Panics
    ///
    /// Never panics in the core implementation.
    pub fn from_value(value: &Value) -> Result<Self, ManifestError> {
        let pairs = as_object(value, "artifact")?;
        Ok(Self {
            path: as_string(get(pairs, "path")?, "path")?.to_string(),
            digest: as_digest(get(pairs, "sha256")?, "sha256")?,
            bytes: as_bytes(get(pairs, "bytes")?, "bytes")?,
            executable: as_bool(get(pairs, "executable")?, "executable")?,
        })
    }
}

/// One task's outcome in a manifest (section 3.3).
///
/// `artifacts` is sorted by path; construction through
/// [`TaskRecord::new`] enforces the sort. A record with status
/// [`TaskStatus::Skipped`] has an absent exit code, the empty blob
/// digest for both streams, and no artifacts, by construction of
/// the scheduler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRecord {
    /// The task identifier.
    pub task_id: String,
    /// The execution digest of the task's spec (section 5.3).
    pub exec_digest: Digest,
    /// The terminal status of the task.
    pub status: TaskStatus,
    /// The child exit code, when the child exited with one. Absent
    /// for skipped tasks, timed out tasks, and death by signal.
    pub exit_code: Option<i32>,
    /// The blob digest of the captured stdout stream.
    pub stdout_digest: Digest,
    /// The blob digest of the captured stderr stream.
    pub stderr_digest: Digest,
    /// The collected artifacts, sorted by path.
    pub artifacts: Vec<ArtifactRecord>,
}

impl TaskRecord {
    /// Builds a record, sorting `artifacts` by path so the
    /// framing order of section 5.2 holds by construction.
    #[must_use]
    pub fn new(
        task_id: String,
        exec_digest: Digest,
        status: TaskStatus,
        exit_code: Option<i32>,
        stdout_digest: Digest,
        stderr_digest: Digest,
        artifacts: Vec<ArtifactRecord>,
    ) -> Self {
        let mut artifacts = artifacts;
        artifacts.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
        Self {
            task_id,
            exec_digest,
            status,
            exit_code,
            stdout_digest,
            stderr_digest,
            artifacts,
        }
    }

    /// The `verirun/v1/task` framing digest of this record, over
    /// the artifact framing digests in path order.
    ///
    /// # Panics
    ///
    /// Never panics in the core implementation.
    #[must_use]
    pub fn record_digest(&self, factory: &dyn HasherFactory) -> Digest {
        let framing: Vec<Digest> = self
            .artifacts
            .iter()
            .map(|artifact| artifact.framing_digest(factory))
            .collect();
        digest::task_digest(
            factory,
            &self.task_id,
            &self.exec_digest,
            self.status,
            self.exit_code,
            &self.stdout_digest,
            &self.stderr_digest,
            &framing,
        )
    }

    /// Every blob digest this record names, deduplicated by
    /// digest: the stdout and stderr digests and every artifact
    /// content digest. Verification re-hashes each distinct digest
    /// exactly once (INV-LAT-3).
    ///
    /// # Panics
    ///
    /// Never panics in the core implementation.
    #[must_use]
    pub fn named_digests(&self) -> Vec<Digest> {
        let mut out: Vec<Digest> = Vec::new();
        for digest in std::iter::once(&self.stdout_digest)
            .chain(std::iter::once(&self.stderr_digest))
            .chain(self.artifacts.iter().map(|artifact| &artifact.digest))
        {
            if !out.contains(digest) {
                out.push(*digest);
            }
        }
        out
    }

    /// The value form of section 9.1.
    ///
    /// # Panics
    ///
    /// Never panics in the core implementation.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let exit_code = self
            .exit_code
            .map_or(Value::Null, |code| Value::Number(f64::from(code)));
        Value::Object(vec![
            ("task_id".to_string(), Value::String(self.task_id.clone())),
            (
                "exec_digest".to_string(),
                Value::String(self.exec_digest.to_hex()),
            ),
            (
                "status".to_string(),
                Value::String(self.status.as_str().to_string()),
            ),
            ("exit_code".to_string(), exit_code),
            (
                "stdout_sha256".to_string(),
                Value::String(self.stdout_digest.to_hex()),
            ),
            (
                "stderr_sha256".to_string(),
                Value::String(self.stderr_digest.to_hex()),
            ),
            (
                "artifacts".to_string(),
                Value::Array(
                    self.artifacts
                        .iter()
                        .map(ArtifactRecord::to_value)
                        .collect(),
                ),
            ),
        ])
    }

    /// Parses the value form back into a record.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] when a field is missing, mistyped,
    /// holds an invalid digest, or names an unknown status.
    ///
    /// # Panics
    ///
    /// Never panics in the core implementation.
    pub fn from_value(value: &Value) -> Result<Self, ManifestError> {
        let pairs = as_object(value, "task")?;
        let artifacts = match get(pairs, "artifacts")? {
            Value::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(ArtifactRecord::from_value(item)?);
                }
                out
            }
            _ => {
                return Err(ManifestError::WrongType {
                    field: "artifacts",
                    expected: "an array",
                });
            }
        };
        Ok(Self::new(
            as_string(get(pairs, "task_id")?, "task_id")?.to_string(),
            as_digest(get(pairs, "exec_digest")?, "exec_digest")?,
            as_status(get(pairs, "status")?)?,
            as_exit_code(get(pairs, "exit_code")?)?,
            as_digest(get(pairs, "stdout_sha256")?, "stdout_sha256")?,
            as_digest(get(pairs, "stderr_sha256")?, "stderr_sha256")?,
            artifacts,
        ))
    }
}

/// A whole run's commitment: the task records, sorted by task id,
/// and the Merkle root over their record digests (section 9.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// The task records, sorted by task id.
    pub tasks: Vec<TaskRecord>,
    /// The Merkle root stored in the manifest value form.
    /// Verification recomputes it from the record fields alone and
    /// compares (section 9.3).
    pub merkle_root: Digest,
}

impl Manifest {
    /// Builds a manifest from records in any order: the records
    /// are sorted by task id and the Merkle root is computed over
    /// their record digests in that order.
    ///
    /// # Panics
    ///
    /// Never panics in the core implementation.
    #[must_use]
    pub fn build(factory: &dyn HasherFactory, mut records: Vec<TaskRecord>) -> Self {
        records.sort_by(|left, right| left.task_id.as_bytes().cmp(right.task_id.as_bytes()));
        let leaves: Vec<Digest> = records
            .iter()
            .map(|record| record.record_digest(factory))
            .collect();
        let merkle_root = digest::merkle_root(factory, &leaves);
        Self {
            tasks: records,
            merkle_root,
        }
    }

    /// The Merkle root recomputed from the record fields alone,
    /// ignoring the stored root. Equal to `merkle_root` in any
    /// manifest this build produced.
    ///
    /// # Panics
    ///
    /// Never panics in the core implementation.
    #[must_use]
    pub fn recompute_root(&self, factory: &dyn HasherFactory) -> Digest {
        let leaves: Vec<Digest> = self
            .tasks
            .iter()
            .map(|record| record.record_digest(factory))
            .collect();
        digest::merkle_root(factory, &leaves)
    }

    /// The value form of section 9.1: an object with `format`,
    /// `merkle_root`, and `tasks` fields, and exactly those fields.
    ///
    /// # Panics
    ///
    /// Never panics in the core implementation.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::Object(vec![
            (
                "format".to_string(),
                Value::String(MANIFEST_FORMAT.to_string()),
            ),
            (
                "merkle_root".to_string(),
                Value::String(self.merkle_root.to_hex()),
            ),
            (
                "tasks".to_string(),
                Value::Array(self.tasks.iter().map(TaskRecord::to_value).collect()),
            ),
        ])
    }

    /// Parses the value form back into a manifest. The stored
    /// Merkle root is carried as read; comparing it against
    /// [`Manifest::recompute_root`] is the verifier's job.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] when the format tag differs from
    /// [`MANIFEST_FORMAT`], a field is missing or mistyped, or a
    /// record fails to parse.
    ///
    /// # Panics
    ///
    /// Never panics in the core implementation.
    pub fn from_value(value: &Value) -> Result<Self, ManifestError> {
        let pairs = as_object(value, "manifest")?;
        let format = as_string(get(pairs, "format")?, "format")?;
        if format != MANIFEST_FORMAT {
            return Err(ManifestError::WrongFormat {
                found: format.to_string(),
            });
        }
        let merkle_root = as_digest(get(pairs, "merkle_root")?, "merkle_root")?;
        let mut tasks = match get(pairs, "tasks")? {
            Value::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(TaskRecord::from_value(item)?);
                }
                out
            }
            _ => {
                return Err(ManifestError::WrongType {
                    field: "tasks",
                    expected: "an array",
                });
            }
        };
        // Records are stored sorted by task id, as in a built
        // manifest. Sorting here means a stored order that differs
        // re-encodes to different bytes, which verification
        // reports as a non-canonical manifest (section 9.3).
        tasks.sort_by(|left, right| left.task_id.as_bytes().cmp(right.task_id.as_bytes()));
        Ok(Self { tasks, merkle_root })
    }

    /// The canonical bytes of the manifest: the bytes that are
    /// stored as its blob.
    ///
    /// # Errors
    ///
    /// Returns [`CanonicalError`] if a record field falls outside
    /// the canonical number domain, which construction prevents.
    ///
    /// # Panics
    ///
    /// Never panics in the core implementation.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CanonicalError> {
        let mut out = Vec::new();
        canonical::encode(&self.to_value(), &mut out)?;
        Ok(out)
    }

    /// The manifest digest: the blob digest of the canonical
    /// bytes. This is the digest `verirun verify` takes.
    ///
    /// # Panics
    ///
    /// Panics if a record field falls outside the canonical
    /// number domain, which construction and parsing both
    /// prevent: the failure is a programming error, not data.
    #[must_use]
    pub fn digest(&self, factory: &dyn HasherFactory) -> Digest {
        let bytes = self
            .canonical_bytes()
            .expect("manifest fields are in the canonical domain by construction");
        digest::hash_bytes(factory, digest::DomainTag::Blob, &bytes)
    }
}

const fn as_object<'a>(
    value: &'a Value,
    field: &'static str,
) -> Result<&'a Vec<(String, Value)>, ManifestError> {
    match value {
        Value::Object(pairs) => Ok(pairs),
        _ => Err(ManifestError::WrongType {
            field,
            expected: "an object",
        }),
    }
}

fn get<'a>(pairs: &'a [(String, Value)], field: &'static str) -> Result<&'a Value, ManifestError> {
    pairs
        .iter()
        .find(|(key, _)| key == field)
        .map(|(_, value)| value)
        .ok_or(ManifestError::MissingField { field })
}

fn as_string<'a>(value: &'a Value, field: &'static str) -> Result<&'a str, ManifestError> {
    match value {
        Value::String(text) => Ok(text),
        _ => Err(ManifestError::WrongType {
            field,
            expected: "a string",
        }),
    }
}

fn as_digest(value: &Value, field: &'static str) -> Result<Digest, ManifestError> {
    let text = as_string(value, field)?;
    Digest::from_hex(text).map_err(|source| ManifestError::InvalidDigest { field, source })
}

const fn as_bool(value: &Value, field: &'static str) -> Result<bool, ManifestError> {
    match value {
        Value::Bool(flag) => Ok(*flag),
        _ => Err(ManifestError::WrongType {
            field,
            expected: "a boolean",
        }),
    }
}

/// The largest magnitude the canonical number domain admits,
/// 2^53 - 1 (section 4.3).
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

#[allow(
    clippy::float_cmp,
    reason = "the domain checks are exactness tests on parsed values: a field is valid only when the number is exactly integral"
)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is range-checked against 0..=9007199254740991 and exact integrality immediately before the conversion, so the conversion is exact"
)]
fn as_bytes(value: &Value, field: &'static str) -> Result<u64, ManifestError> {
    match value {
        Value::Number(number) => {
            if *number < 0.0 || *number > MAX_SAFE_INTEGER || number.fract() != 0.0 {
                return Err(ManifestError::InvalidNumber {
                    field,
                    detail: format!(
                        "byte length must be an integer between 0 and 9007199254740991, found {number}"
                    ),
                });
            }
            Ok(*number as u64)
        }
        _ => Err(ManifestError::WrongType {
            field,
            expected: "a number",
        }),
    }
}

#[allow(
    clippy::float_cmp,
    reason = "the domain checks are exactness tests on parsed values: a field is valid only when the number is exactly integral"
)]
fn as_exit_code(value: &Value) -> Result<Option<i32>, ManifestError> {
    match value {
        Value::Null => Ok(None),
        Value::Number(number) => {
            if *number < f64::from(i32::MIN)
                || *number > f64::from(i32::MAX)
                || number.fract() != 0.0
            {
                return Err(ManifestError::InvalidNumber {
                    field: "exit_code",
                    detail: format!(
                        "exit code must be an integer in the i32 range, found {number}"
                    ),
                });
            }
            #[allow(
                clippy::cast_possible_truncation,
                reason = "the value was range-checked against the i32 bounds immediately above"
            )]
            Ok(Some(*number as i32))
        }
        _ => Err(ManifestError::WrongType {
            field: "exit_code",
            expected: "an integer or null",
        }),
    }
}

fn as_status(value: &Value) -> Result<TaskStatus, ManifestError> {
    let text = as_string(value, "status")?;
    match text {
        "succeeded" => Ok(TaskStatus::Succeeded),
        "failed" => Ok(TaskStatus::Failed),
        "timed_out" => Ok(TaskStatus::TimedOut),
        "cached" => Ok(TaskStatus::Cached),
        "skipped" => Ok(TaskStatus::Skipped),
        _ => Err(ManifestError::UnknownStatus {
            found: text.to_string(),
        }),
    }
}
