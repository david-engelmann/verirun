//! The Windows job object: created with kill on job close,
//! assigned the spawned child, and terminated as a whole.

use std::ffi::c_void;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::process::Child;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};

/// A Windows job object holding one task's process tree.
///
/// The job is created at the first attach, configured with kill
/// on job close before any process joins it, and closed on drop,
/// which ends any member still running. Assignment happens
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
            // close terminates any member still running.
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
