//! The store error type (docs/ARCHITECTURE.md section 11).
//!
//! Every variant carries the CAS path involved and, for I/O failures,
//! the protocol step that was in progress. Integrity failures name the
//! expected and actual digests.

use core::fmt;
use std::io;
use std::path::PathBuf;

use verirun_core::digest::Digest;

/// The step of the put or read protocol during which an I/O failure
/// occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StoreStep {
    /// Creating the temporary file.
    CreateTemp,
    /// Streaming bytes into the temporary file.
    Write,
    /// Syncing the temporary file.
    SyncFile,
    /// Setting the temporary file read-only.
    SetReadOnly,
    /// Creating the shard directory.
    CreateShard,
    /// Renaming the temporary file to its object path.
    Rename,
    /// Syncing the destination shard directory.
    SyncShard,
    /// Syncing the `objects/` directory after creating a shard.
    SyncObjects,
    /// Creating a store layout directory while opening the store.
    CreateLayout,
    /// Opening an object for reading.
    OpenObject,
    /// Opening a source file for a `put_file`.
    OpenSource,
    /// Streaming bytes out of an object.
    ReadObject,
    /// Writing bytes to a read destination.
    WriteDestination,
    /// Removing a discarded temporary file.
    RemoveTemp,
}

impl fmt::Display for StoreStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::CreateTemp => "create temp file",
            Self::Write => "write temp file",
            Self::SyncFile => "sync temp file",
            Self::SetReadOnly => "set temp file read-only",
            Self::CreateShard => "create shard directory",
            Self::Rename => "rename into place",
            Self::SyncShard => "sync shard directory",
            Self::SyncObjects => "sync objects directory",
            Self::CreateLayout => "create store layout",
            Self::OpenObject => "open object",
            Self::OpenSource => "open source file",
            Self::ReadObject => "read object",
            Self::WriteDestination => "write destination",
            Self::RemoveTemp => "remove temp file",
        };
        f.write_str(name)
    }
}

/// A store failure.
#[derive(Debug)]
pub enum StoreError {
    /// An I/O failure at a stated protocol step and path.
    Io {
        /// The path being operated on.
        path: PathBuf,
        /// The protocol step in progress.
        step: StoreStep,
        /// The underlying I/O error.
        source: io::Error,
    },
    /// The `tmp/` and `objects/` directories resolve to different
    /// filesystems, so renames between them could fail with EXDEV.
    CrossDevice {
        /// The store root whose layout was refused.
        root: PathBuf,
        /// The device id of `tmp/`.
        tmp_device: u64,
        /// The device id of `objects/`.
        objects_device: u64,
    },
    /// A blob read back under a digest does not hash to that digest.
    Integrity {
        /// The digest the content was addressed by.
        expected: Digest,
        /// The digest the content actually hashes to.
        actual: Digest,
        /// The object path read.
        path: PathBuf,
    },
    /// No object exists under the requested digest.
    Missing {
        /// The digest that was looked up.
        digest: Digest,
        /// The object path that was expected to hold it.
        path: PathBuf,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, step, source } => {
                write!(
                    f,
                    "store I/O failure at {} on {}: {source}",
                    step,
                    path.display()
                )
            }
            Self::CrossDevice {
                root,
                tmp_device,
                objects_device,
            } => write!(
                f,
                "store layout refused at {}: tmp/ is on device {tmp_device} but objects/ is on device {objects_device}; renames would cross filesystems",
                root.display()
            ),
            Self::Integrity {
                expected,
                actual,
                path,
            } => write!(
                f,
                "integrity failure at {}: expected digest {expected}, content hashes to {actual}",
                path.display()
            ),
            Self::Missing { digest, path } => {
                write!(f, "missing object {digest} at {}", path.display())
            }
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// A run failure: what aborts a run rather than failing one task
/// (docs/ARCHITECTURE.md sections 7, 8, and 11).
///
/// Task-level failures (a nonzero exit, a spawn failure, a missing
/// or linked output at collection) are outcomes recorded in the
/// manifest, never errors on this type. This type carries the
/// failures that stop the run itself: store failures, including
/// the integrity failure a corrupted input blob raises before its
/// task starts (section 6.4), and inputs that no producer and no
/// caller binding resolves.
#[derive(Debug)]
pub enum RunError {
    /// A store failure during the run.
    Store {
        /// The store failure.
        source: StoreError,
    },
    /// A task declares an input path that no dependency produced
    /// and the caller did not bind. Raised before any task in the
    /// run executes, so a run never starts on unresolvable inputs.
    MissingInput {
        /// The task declaring the input.
        task_id: String,
        /// The normalized input path that resolves nowhere.
        path: String,
    },
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store { source } => write!(f, "run aborted by a store failure: {source}"),
            Self::MissingInput { task_id, path } => write!(
                f,
                "task {task_id:?} declares input {path:?}, which no dependency produced and the caller did not bind"
            ),
        }
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store { source } => Some(source),
            Self::MissingInput { .. } => None,
        }
    }
}

impl From<StoreError> for RunError {
    fn from(source: StoreError) -> Self {
        Self::Store { source }
    }
}
