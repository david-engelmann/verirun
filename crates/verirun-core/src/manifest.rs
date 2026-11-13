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
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    #[must_use]
    pub fn to_value(&self) -> Value {
        panic!("verirun Milestone 2 Stage A: ArtifactRecord::to_value is not yet implemented")
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
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    pub fn from_value(value: &Value) -> Result<Self, ManifestError> {
        let _ = value;
        panic!("verirun Milestone 2 Stage A: ArtifactRecord::from_value is not yet implemented")
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
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    #[must_use]
    pub fn record_digest(&self, factory: &dyn HasherFactory) -> Digest {
        let _ = factory;
        panic!("verirun Milestone 2 Stage A: TaskRecord::record_digest is not yet implemented")
    }

    /// Every blob digest this record names, deduplicated by
    /// digest: the stdout and stderr digests and every artifact
    /// content digest. Verification re-hashes each distinct digest
    /// exactly once (INV-LAT-3).
    ///
    /// # Panics
    ///
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    #[must_use]
    pub fn named_digests(&self) -> Vec<Digest> {
        panic!("verirun Milestone 2 Stage A: TaskRecord::named_digests is not yet implemented")
    }

    /// The value form of section 9.1.
    ///
    /// # Panics
    ///
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    #[must_use]
    pub fn to_value(&self) -> Value {
        panic!("verirun Milestone 2 Stage A: TaskRecord::to_value is not yet implemented")
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
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    pub fn from_value(value: &Value) -> Result<Self, ManifestError> {
        let _ = value;
        panic!("verirun Milestone 2 Stage A: TaskRecord::from_value is not yet implemented")
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
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    #[must_use]
    pub fn build(factory: &dyn HasherFactory, records: Vec<TaskRecord>) -> Self {
        let _ = (factory, records);
        panic!("verirun Milestone 2 Stage A: Manifest::build is not yet implemented")
    }

    /// The Merkle root recomputed from the record fields alone,
    /// ignoring the stored root. Equal to `merkle_root` in any
    /// manifest this build produced.
    ///
    /// # Panics
    ///
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    #[must_use]
    pub fn recompute_root(&self, factory: &dyn HasherFactory) -> Digest {
        let _ = factory;
        panic!("verirun Milestone 2 Stage A: Manifest::recompute_root is not yet implemented")
    }

    /// The value form of section 9.1: an object with `format`,
    /// `merkle_root`, and `tasks` fields, and exactly those fields.
    ///
    /// # Panics
    ///
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    #[must_use]
    pub fn to_value(&self) -> Value {
        panic!("verirun Milestone 2 Stage A: Manifest::to_value is not yet implemented")
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
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    pub fn from_value(value: &Value) -> Result<Self, ManifestError> {
        let _ = value;
        panic!("verirun Milestone 2 Stage A: Manifest::from_value is not yet implemented")
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
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CanonicalError> {
        panic!("verirun Milestone 2 Stage A: Manifest::canonical_bytes is not yet implemented")
    }

    /// The manifest digest: the blob digest of the canonical
    /// bytes. This is the digest `verirun verify` takes.
    ///
    /// # Panics
    ///
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    #[must_use]
    pub fn digest(&self, factory: &dyn HasherFactory) -> Digest {
        let _ = factory;
        panic!("verirun Milestone 2 Stage A: Manifest::digest is not yet implemented")
    }
}
