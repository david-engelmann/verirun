//! The Unix process group: the child leads its own group (the
//! runner arranges that at spawn through `process_group(0)`),
//! and termination signals the whole group through the shell's
//! builtin `kill`.
//!
//! The same file holds the Unix traversal backend: descriptor
//! relative opens through `openat`, the crate's one libc use.

use std::ffi::{CString, c_int};
use std::fs::File;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::TraversalError;

/// A Unix process group, identified by its leader's process id.
///
/// The group itself is created by the spawn configuration, so
/// attaching only records the leader. Termination sends KILL to
/// the group through the shell builtin: the standalone `kill`
/// utility's negative-pid form was observed to be a silent no-op
/// in the development sandbox, while the builtin reaches the
/// syscall directly. A failed shell spawn is silent by design;
/// the caller follows termination with a direct kill of the
/// leader as a fallback.
pub struct ProcessGroup {
    /// The group leader's process id, once a child is attached.
    leader: Option<u32>,
}

impl ProcessGroup {
    /// Creates an empty group with no leader attached.
    #[must_use]
    pub const fn new() -> Self {
        Self { leader: None }
    }

    /// Records `child` as the group's leader. The child was
    /// spawned as a group leader by the runner, so the group
    /// already exists; this cannot fail on Unix.
    ///
    /// # Errors
    ///
    /// Never returns an error on Unix. The result is part of the
    /// cross-platform API shape, where attaching can fail.
    pub fn attach(&mut self, child: &Child) -> io::Result<()> {
        self.leader = Some(child.id());
        Ok(())
    }

    /// Kills every process in the group with KILL. Does nothing
    /// when no child was attached.
    pub fn terminate(&self) {
        if let Some(leader) = self.leader {
            let script = format!("kill -KILL -{leader}");
            let _ = Command::new("/bin/sh")
                .args(["-c", &script])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

impl Default for ProcessGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// A directory held open for handle relative traversal.
///
/// Each child is opened with `openat` relative to this
/// directory's descriptor, with no-follow semantics, so the
/// object checked is the object opened and a component
/// swapped after the open cannot redirect the traversal: the
/// descriptor names the object itself, not its path.
pub struct DirHandle {
    /// The held directory, as the file its descriptor backs.
    dir: File,
}

impl DirHandle {
    /// Opens the traversal root, a directory the caller
    /// names by path. The root is opened with no-follow
    /// semantics like every component below it.
    ///
    /// # Errors
    ///
    /// Returns [`TraversalError::NotFound`] when the root does
    /// not exist, [`TraversalError::Link`] when it is a
    /// symbolic link, and [`TraversalError::Io`] for any other
    /// operating system failure.
    pub fn open_root(path: &Path) -> Result<Self, TraversalError> {
        let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
            TraversalError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the traversal root path contains a nul byte",
            ))
        })?;
        // SAFETY: `c_path` is a valid nul terminated string
        // that outlives the call, and the descriptor returned
        // is checked before use.
        let fd = unsafe {
            libc::open(
                c_path.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(map_open_error(io::Error::last_os_error()));
        }
        // SAFETY: `fd` is a fresh descriptor this call created
        // and nothing else owns it.
        let dir = unsafe { File::from_raw_fd(fd) };
        Ok(Self { dir })
    }

    /// Opens one component below this directory as the next
    /// directory of the traversal.
    ///
    /// # Errors
    ///
    /// Returns [`TraversalError::NotFound`] when the component
    /// does not exist, [`TraversalError::Link`] when it is a
    /// symbolic link, and [`TraversalError::Io`] for any other
    /// failure, including the component existing but not
    /// being a directory.
    pub fn open_dir(&self, name: &str) -> Result<Self, TraversalError> {
        let dir = self.open_at(
            name,
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )?;
        Ok(Self { dir })
    }

    /// Opens one component below this directory as the file
    /// the traversal ends at.
    ///
    /// # Errors
    ///
    /// Returns [`TraversalError::NotFound`] when the component
    /// does not exist, [`TraversalError::Link`] when it is a
    /// symbolic link, [`TraversalError::NotRegularFile`] when
    /// it exists but is not a regular file, and
    /// [`TraversalError::Io`] for any other failure.
    pub fn open_file(&self, name: &str) -> Result<File, TraversalError> {
        // NONBLOCK keeps a special file (a fifo above all)
        // from blocking the open until a writer appears; the
        // metadata check below rejects it immediately instead.
        // Regular files ignore the flag.
        let file = self.open_at(
            name,
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(TraversalError::NotRegularFile);
        }
        Ok(file)
    }

    /// Opens `name` relative to this directory's descriptor
    /// with the given flags.
    fn open_at(&self, name: &str, flags: c_int) -> Result<File, TraversalError> {
        let c_name = component(name)?;
        // SAFETY: `c_name` is a valid nul terminated string
        // that outlives the call, the descriptor is this
        // handle's live directory, and the descriptor returned
        // is checked before use.
        let fd = unsafe { libc::openat(self.dir.as_raw_fd(), c_name.as_ptr(), flags) };
        if fd < 0 {
            return Err(map_open_error(io::Error::last_os_error()));
        }
        // SAFETY: `fd` is a fresh descriptor this call created
        // and nothing else owns it.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

/// Validates one traversal component and converts it to the
/// C string the descriptor relative open takes. A component
/// is a single name: never empty, never a dot segment, and
/// never carrying a separator of either platform.
fn component(name: &str) -> Result<CString, TraversalError> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(TraversalError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a traversal component must be a single name",
        )));
    }
    CString::new(name).map_err(|_| {
        TraversalError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a traversal component contains a nul byte",
        ))
    })
}

/// Maps an open failure to the traversal error for it.
fn map_open_error(source: io::Error) -> TraversalError {
    match source.raw_os_error() {
        Some(code) if code == libc::ENOENT => TraversalError::NotFound,
        Some(code) if code == libc::ELOOP => TraversalError::Link,
        _ => TraversalError::Io(source),
    }
}
