//! verirun-store: the I/O adapter crate of verirun.
//!
//! - [`sha256`]: the concrete SHA-256 implementation behind the core
//!   [`verirun_core::digest::HasherFactory`] seam. This is the only
//!   module in the workspace that names a hash implementation.
//! - [`cas_fs`]: the filesystem content-addressed store. The 4-step
//!   atomic put protocol, streaming verified reads, and the by-exec
//!   cache index paths of docs/ARCHITECTURE.md section 6.
//! - [`runner`]: hermetic subprocess execution. Environment
//!   construction, executable resolution, concurrent pipe drains,
//!   timeout enforcement, and output collection under the sandbox
//!   rules of section 8.
//! - [`scheduler`]: the worker pool over a validated DAG. Lexicographic
//!   ready order, failure propagation, the by-exec execution cache,
//!   and manifest production (section 7.3).
//! - [`gc`]: garbage collection execution. Mark from pins, plan
//!   against the grace threshold, sweep under the lock, with a dry
//!   run that deletes nothing (section 10).
//! - [`clock`]: the single wall clock read in the workspace, used
//!   only for garbage collection ages.
//! - [`error`]: the store and run error types, carrying the CAS path
//!   and the protocol step in progress on every failure.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod cas_fs;
pub mod clock;
pub mod error;
pub mod gc;
pub mod runner;
pub mod scheduler;
pub mod sha256;
