//! verirun-platform: the platform seam of verirun.
//!
//! One safe API over the platform mechanisms the runner
//! needs: a Unix process group and a Windows job object for
//! process control, and handle relative path traversal for
//! output collection. The rest of the workspace forbids
//! unsafe code, and the Windows interfaces are foreign
//! function interfaces, so this crate exists to hold the
//! workspace's only unsafe blocks, each one small, behind a
//! safe API, and carrying its safety argument. On Unix the
//! process side needs no unsafe code at all: the group kill
//! goes through the shell's builtin `kill`, the mechanism
//! the runner has used since its first build, moved here
//! unchanged so both platforms present the same shape to
//! the runner. The traversal side uses descriptor relative
//! opens on Unix and handle relative creates on Windows.
//!
//! The contract of [`ProcessGroup`] is the runner's contract.
//! A group is created empty, a freshly spawned child is attached
//! to it, `terminate` ends every process in the group, and
//! dropping the group releases it. On Windows the job is created
//! with kill on job close, so dropping the group also ends any
//! member still running. A process that escapes its group, by
//! creating its own session on Unix or by breaking away from the
//! job on Windows, is beyond the group's reach on both
//! platforms; the runner's drain grace bounds what that costs.
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

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{DirHandle, ProcessGroup};
#[cfg(windows)]
pub use windows::{DirHandle, ProcessGroup};

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
