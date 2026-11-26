//! The Windows job object: created with kill on job close,
//! assigned the spawned child, and terminated as a whole.
//!
//! The same file holds the Windows traversal backend: handle
//! relative creates through the native `NtCreateFile`, with
//! each component opened relative to its parent's handle and
//! reparse points rejected from the opened handle's own
//! attributes. It also holds [`rename_replace`], the atomic
//! write-through rename the store's put and cache protocols use.

use std::ffi::c_void;
use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;
use std::process::Child;

use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN,
    FILE_OPEN_REPARSE_POINT as NT_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, GENERIC_READ, HANDLE, INVALID_HANDLE_VALUE, STATUS_FILE_IS_A_DIRECTORY,
    STATUS_OBJECT_NAME_NOT_FOUND, STATUS_OBJECT_PATH_NOT_FOUND, UNICODE_STRING,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle,
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Kernel::OBJ_CASE_INSENSITIVE;

use crate::TraversalError;

/// A Windows job object holding one task's process tree.
///
/// The job is created at the first attach, configured with kill
/// on job close before any process joins it, and closed on drop,
/// which ends any member still running unless the group was
/// disarmed first. Assignment happens
/// immediately after the spawn; a process the child spawns in
/// the interval before assignment inherits no job membership and
/// could escape, the same class of window the Unix group has
/// always had with session creation, and bounded the same way,
/// by the runner's drain grace.
pub struct ProcessGroup {
    /// The job object handle, once created.
    job: Option<HANDLE>,
}

// SAFETY: a job object handle is a kernel handle. It names the
// same job from any thread, and every operation this type
// performs through it is a job object API call, which the
// operating system serializes. The handle is never shared
// except through this type's own methods.
unsafe impl Send for ProcessGroup {}

impl ProcessGroup {
    /// Creates an empty group. The job object itself is created
    /// at the first attach.
    #[must_use]
    pub const fn new() -> Self {
        Self { job: None }
    }

    /// Assigns `child` to the job, creating and configuring the
    /// job first when this is the first attach.
    ///
    /// # Errors
    ///
    /// Returns the operating system error when the job cannot be
    /// created, configured, or assigned. The caller treats an
    /// attach failure as a spawn failure, because a child outside
    /// the job is a child the deadline cannot kill as a tree.
    pub fn attach(&mut self, child: &Child) -> io::Result<()> {
        let job = if let Some(job) = self.job {
            job
        } else {
            let created = create_job()?;
            self.job = Some(created);
            created
        };
        // SAFETY: `job` is a live job object handle owned by
        // this group, and the child's handle is valid while the
        // child is alive, which the caller guarantees by keeping
        // the child across this call.
        let assigned = unsafe { AssignProcessToJobObject(job, child.as_raw_handle()) };
        if assigned == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Disarms the group: the job's kill on job close limit is
    /// cleared, so closing the handle on drop no longer ends
    /// members still running. A job handle cannot be confused
    /// with another job the way a recycled group id can, so
    /// this side needs no disarm for its own safety. It exists
    /// because the contract is one contract. The clearing is
    /// best effort, like termination: a failure leaves the
    /// armed behavior in place and is not reported.
    pub fn disarm(&mut self) {
        if let Some(job) = self.job {
            // SAFETY: every field of the information structure
            // is plain data with no invalid bit patterns, so the
            // zeroed value is valid, and its zeroed limit flags
            // are exactly the cleared state this call installs.
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { core::mem::zeroed() };
            info.BasicLimitInformation.LimitFlags = 0;
            if let Ok(size) =
                u32::try_from(core::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
            {
                // SAFETY: `job` is a live job object handle owned
                // by this group, `info` points to a valid
                // structure of the stated size, and the API reads
                // it synchronously during the call.
                let _ = unsafe {
                    SetInformationJobObject(
                        job,
                        JobObjectExtendedLimitInformation,
                        core::ptr::addr_of!(info).cast::<c_void>(),
                        size,
                    )
                };
            }
        }
    }

    /// Terminates every process in the job. Does nothing when no
    /// child was ever attached.
    pub fn terminate(&self) {
        if let Some(job) = self.job {
            // SAFETY: `job` is a live job object handle owned by
            // this group.
            unsafe {
                TerminateJobObject(job, 1);
            }
        }
    }
}

impl Default for ProcessGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            // SAFETY: `job` is a live job object handle owned by
            // this group, and this is its last use. Closing the
            // last handle of a job configured with kill on job
            // close terminates any member still running, unless
            // the group was disarmed first.
            unsafe {
                CloseHandle(job);
            }
        }
    }
}

/// Creates a job object and configures it with kill on job
/// close, so the job can never outlive this handle with members
/// still running.
fn create_job() -> io::Result<HANDLE> {
    // SAFETY: both pointers are null, which the API accepts as
    // default security attributes and an unnamed job.
    let job = unsafe { CreateJobObjectW(core::ptr::null(), core::ptr::null()) };
    if job.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: every field of the information structure is plain
    // data with no invalid bit patterns, so the zeroed value is
    // a valid starting point whose only set field is the limit
    // flag below.
    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { core::mem::zeroed() };
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let size = u32::try_from(core::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
        .map_err(|_| io::Error::other("the job information structure exceeds a u32 size"))?;
    // SAFETY: `job` is a live job object handle, `info` points
    // to a valid structure of the stated size, and the API reads
    // it synchronously during the call.
    let configured = unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            core::ptr::addr_of!(info).cast::<c_void>(),
            size,
        )
    };
    if configured == 0 {
        let source = io::Error::last_os_error();
        // SAFETY: `job` is a live job object handle owned by
        // this function, and this is its last use.
        unsafe {
            CloseHandle(job);
        }
        return Err(source);
    }
    Ok(job)
}

/// A directory held open for handle relative traversal.
///
/// Each child is created with `NtCreateFile` relative to this
/// directory's handle, with reparse point inspection, so the
/// object checked is the object opened and a component
/// swapped after the open cannot redirect the traversal: the
/// handle names the object itself, not its path.
pub struct DirHandle {
    /// The held directory handle.
    handle: HANDLE,
}

// SAFETY: a directory handle is a kernel handle. It names the
// same object from any thread, and every operation this type
// performs through it is a file API call, which the operating
// system serializes. The handle is never shared except
// through this type's own methods.
unsafe impl Send for DirHandle {}

impl DirHandle {
    /// Opens the traversal root, a directory the caller
    /// names by path. The root is opened with reparse point
    /// inspection like every component below it.
    ///
    /// # Errors
    ///
    /// Returns [`TraversalError::NotFound`] when the root does
    /// not exist, [`TraversalError::Link`] when it is a
    /// reparse point, and [`TraversalError::Io`] for any other
    /// operating system failure.
    pub fn open_root(path: &Path) -> Result<Self, TraversalError> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: `wide` is a valid nul terminated UTF-16
        // path that outlives the call, the security attributes
        // and template are null, which the API accepts, and
        // the returned handle is checked before use.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                core::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                core::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(map_create_error(io::Error::last_os_error()));
        }
        let dir = Self { handle };
        dir.require_directory()?;
        Ok(dir)
    }

    /// Opens one component below this directory as the next
    /// directory of the traversal.
    ///
    /// # Errors
    ///
    /// Returns [`TraversalError::NotFound`] when the component
    /// does not exist, [`TraversalError::Link`] when it is a
    /// reparse point, and [`TraversalError::Io`] for any other
    /// failure, including the component existing but not
    /// being a directory.
    pub fn open_dir(&self, name: &str) -> Result<Self, TraversalError> {
        let dir = Self {
            handle: self.open_relative(name, true)?,
        };
        dir.require_directory()?;
        Ok(dir)
    }

    /// Opens one component below this directory as the file
    /// the traversal ends at.
    ///
    /// # Errors
    ///
    /// Returns [`TraversalError::NotFound`] when the component
    /// does not exist, [`TraversalError::Link`] when it is a
    /// reparse point, [`TraversalError::NotRegularFile`] when
    /// it exists but is not a regular file, and
    /// [`TraversalError::Io`] for any other failure.
    pub fn open_file(&self, name: &str) -> Result<File, TraversalError> {
        let handle = self.open_relative(name, false)?;
        // SAFETY: `handle` is a fresh handle this call created
        // and nothing else owns it.
        let file = unsafe { File::from_raw_handle(handle) };
        let metadata = file.metadata()?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(TraversalError::Link);
        }
        if !metadata.is_file() {
            return Err(TraversalError::NotRegularFile);
        }
        Ok(file)
    }

    /// Creates one component relative to this directory's
    /// handle through the native create call. The create asks
    /// for the reparse point itself rather than its target,
    /// so a link is opened as a link and the caller's
    /// attribute check on the resulting handle rejects it.
    /// The create asks for synchronous I/O, because the
    /// caller reads the opened file through ordinary
    /// synchronous reads, and carries the synchronize right
    /// in its raw form in the access mask: the native create
    /// checks that right in the raw mask when the
    /// synchronous option is set, and the generic read mask
    /// does not satisfy the check (the first hosted run of
    /// this backend proved the point with invalid parameter
    /// failures on every relative open, and a variant probe
    /// isolated the mask as the cause).
    fn open_relative(&self, name: &str, directory: bool) -> Result<HANDLE, TraversalError> {
        let mut name16: Vec<u16> = component(name)?;
        let byte_len = u16::try_from(name16.len() * 2).map_err(|_| {
            TraversalError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a traversal component is too long",
            ))
        })?;
        let unicode = UNICODE_STRING {
            Length: byte_len,
            MaximumLength: byte_len,
            Buffer: name16.as_mut_ptr(),
        };
        // SAFETY: every field of the attributes structure is
        // plain data with no invalid bit patterns, so the
        // zeroed value is a valid starting point for the
        // fields set below.
        let mut attributes: OBJECT_ATTRIBUTES = unsafe { core::mem::zeroed() };
        attributes.Length =
            u32::try_from(core::mem::size_of::<OBJECT_ATTRIBUTES>()).map_err(|_| {
                TraversalError::Io(io::Error::other(
                    "the object attributes structure exceeds a u32 size",
                ))
            })?;
        attributes.RootDirectory = self.handle;
        attributes.ObjectName = core::ptr::addr_of!(unicode);
        attributes.Attributes = OBJ_CASE_INSENSITIVE.cast_unsigned();
        // SAFETY: the status block is plain data, zeroed here
        // and written by the call before it is read.
        let mut status_block: IO_STATUS_BLOCK = unsafe { core::mem::zeroed() };
        let mut handle: HANDLE = core::ptr::null_mut();
        let options = NT_OPEN_REPARSE_POINT
            | FILE_SYNCHRONOUS_IO_NONALERT
            | if directory {
                FILE_DIRECTORY_FILE
            } else {
                FILE_NON_DIRECTORY_FILE
            };
        // The access masks, built from the individual
        // rights. Both carry synchronize (0x0010_0000) and
        // read attributes (0x0000_0080). A directory adds
        // list (0x0000_0001) and traverse (0x0000_0020); a
        // file adds read data (0x0000_0001).
        let access = if directory { 0x0010_00A1 } else { 0x0010_0081 };
        // SAFETY: `attributes` names `unicode`, which outlives
        // the call because `name16` is alive in this frame;
        // the parent handle is this handle's live directory;
        // the out parameters are valid writable locations; and
        // the returned handle is checked through the status
        // before any use.
        let status = unsafe {
            NtCreateFile(
                core::ptr::addr_of_mut!(handle),
                access,
                core::ptr::addr_of!(attributes),
                core::ptr::addr_of_mut!(status_block),
                core::ptr::null(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                FILE_OPEN,
                options,
                core::ptr::null(),
                0,
            )
        };
        if status < 0 {
            return Err(map_nt_status(status));
        }
        Ok(handle)
    }

    /// Reads this handle's own attributes and requires the
    /// directory the traversal expects: not a reparse point,
    /// and a directory.
    fn require_directory(&self) -> Result<(), TraversalError> {
        // SAFETY: the information structure is plain data,
        // zeroed here and written by the call before it is
        // read, and the handle is this handle's live directory.
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { core::mem::zeroed() };
        // SAFETY: the handle is live and `info` points to a
        // valid structure the API fills synchronously.
        let read =
            unsafe { GetFileInformationByHandle(self.handle, core::ptr::addr_of_mut!(info)) };
        if read == 0 {
            return Err(TraversalError::Io(io::Error::last_os_error()));
        }
        if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(TraversalError::Link);
        }
        if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(TraversalError::Io(io::Error::new(
                io::ErrorKind::NotADirectory,
                "the component is not a directory",
            )));
        }
        Ok(())
    }
}

impl Drop for DirHandle {
    fn drop(&mut self) {
        // SAFETY: the handle is a live directory handle owned
        // by this value, and this is its last use.
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

/// Validates one traversal component and converts it to the
/// UTF-16 the native create takes. A component is a single
/// name: never empty, never a dot segment, and never carrying
/// a separator of either platform.
fn component(name: &str) -> Result<Vec<u16>, TraversalError> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(TraversalError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a traversal component must be a single name",
        )));
    }
    Ok(name.encode_utf16().collect())
}

/// Maps a `CreateFileW` failure to the traversal error for
/// it.
fn map_create_error(source: io::Error) -> TraversalError {
    match source.raw_os_error() {
        // ERROR_FILE_NOT_FOUND and ERROR_PATH_NOT_FOUND.
        Some(2 | 3) => TraversalError::NotFound,
        _ => TraversalError::Io(source),
    }
}

/// Maps a native create status to the traversal error for
/// it.
fn map_nt_status(status: i32) -> TraversalError {
    if status == STATUS_OBJECT_NAME_NOT_FOUND || status == STATUS_OBJECT_PATH_NOT_FOUND {
        TraversalError::NotFound
    } else if status == STATUS_FILE_IS_A_DIRECTORY {
        TraversalError::NotRegularFile
    } else {
        TraversalError::Io(io::Error::other(format!(
            "the native create failed with status 0x{:08x}",
            status.cast_unsigned()
        )))
    }
}

/// Atomically renames `from` to `to`, replacing an existing
/// writable destination with write-through metadata ordering.
///
/// On Windows this calls `MoveFileExW` with
/// `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH` so the
/// platform flushes the rename's metadata before returning, the
/// strongest ordering Windows offers in the absence of directory
/// sync. Unlike `std::fs::rename`, a read-only destination is not
/// retried by clearing attributes; the store's put protocol
/// completes concurrent identical-digest races through its discard
/// path instead. On Unix the counterpart performs an atomic
/// `rename`, with durability completed by the caller's directory
/// `sync_all`.
///
/// # Errors
///
/// Returns the rename failure from the operating system.
pub fn rename_replace(from: &Path, to: &Path) -> io::Result<()> {
    let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both paths are valid nul terminated UTF-16
    // strings that outlive the call, and the flags are the
    // documented replace and write through constants. The
    // native call takes the string pointers directly, the
    // wide pointer type being a plain pointer alias in
    // windows-sys.
    let ok = unsafe {
        MoveFileExW(
            from_wide.as_ptr(),
            to_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
