//! verirun-platform: the platform seam of verirun.
//!
//! One safe API over the two process control mechanisms the
//! runner needs: a Unix process group and a Windows job object.
//! The rest of the workspace forbids unsafe code, and the
//! Windows job object interface is a foreign function interface,
//! so this crate exists to hold the workspace's only unsafe
//! blocks, each one small, behind a safe API, and carrying its
//! safety argument. On Unix the crate needs no unsafe code at
//! all: the group kill goes through the shell's builtin `kill`,
//! the mechanism the runner has used since its first build,
//! moved here unchanged so both platforms present the same
//! shape to the runner.
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

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::ProcessGroup;
#[cfg(windows)]
pub use windows::ProcessGroup;
