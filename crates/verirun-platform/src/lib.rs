//! verirun-platform: the platform seam of verirun.
//!
//! One safe API over the platform mechanisms the runner and the
//! store need: a Unix process group and a Windows job object for
//! process control, handle relative path traversal for output
//! collection, and [`rename_replace`] for atomic file replacement
//! in the store's put and cache protocols. The rest of the
//! workspace forbids unsafe code, and the operating system
//! interfaces here are foreign function interfaces, so this crate
//! exists to hold the workspace's only unsafe blocks, each one
//! small, behind a safe API, and carrying its safety argument.
//!
//! The contract of [`ProcessGroup`] is the runner's contract.
//! A group is created empty, a freshly spawned child is attached
//! to it, `terminate` ends every process in the group, and
//! dropping the group terminates any member still running,
//! unless the group was disarmed first. A caller that has
//! reaped the leader and sent any termination it intends
//! disarms the group before any wait that could outlast the
//! group itself, because the Unix group is named by a number
//! the system can hand to a new group once this one is empty,
//! while the Windows job is named by its handle and cannot be
//! confused with another job. An abandoned group that was
//! never disarmed keeps the drop termination. On
//! Unix `terminate` signals the negative process-group id directly
//! through `libc::kill` with `SIGKILL`. On Windows the job is
//! created with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, so explicit
//! termination and handle close both end every member. A process
//! that escapes its group, by creating its own session on Unix, is
//! beyond the group's reach; the runner's drain grace bounds what
//! that costs.
//!
//! The contract of [`DirHandle`] is the collection contract.
//! A traversal opens one path component at a time, each
//! relative to the held handle of its parent directory, so
//! the object that is checked is the object that is opened
//! and no component is ever followed through a symbolic link
//! or a reparse point. A path assembled from checked
//! components and opened afterwards has a window between the
//! check and the open where a component can be swapped; a
//! traversal through held handles has no such window, because
//! renaming a component after it is opened does not change
//! the object its handle names.
//!
//! The contract of [`rename_replace`] is the store's atomic
//! replacement step. On Windows it calls `MoveFileExW` with
//! `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH` so
//! metadata is flushed before returning while refusing replacement
//! of a read-only destination. On Unix it performs an atomic
//! `rename`, with directory-entry durability completed by the
//! caller's parent-directory `sync_all`.

#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{DirHandle, ProcessGroup, rename_replace};
#[cfg(windows)]
pub use windows::{DirHandle, ProcessGroup, rename_replace};

use std::fmt;
use std::io;

/// How a handle relative traversal failed, in the terms the
/// caller acts on. The variants name the outcomes a caller
/// distinguishes; everything else travels as the operating
/// system error that caused it.
#[derive(Debug)]
pub enum TraversalError {
    /// A component of the path does not exist.
    NotFound,
    /// A component is a symbolic link or a reparse point.
    /// Traversal never follows one, at any position.
    Link,
    /// The final component exists but is not a regular file.
    NotRegularFile,
    /// Any other operating system failure.
    Io(io::Error),
}

impl fmt::Display for TraversalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "a component of the path does not exist"),
            Self::Link => write!(f, "a component of the path is a symbolic link"),
            Self::NotRegularFile => write!(f, "the final component is not a regular file"),
            Self::Io(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for TraversalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for TraversalError {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}
