//! The wall clock (docs/ARCHITECTURE.md sections 10 and 14).
//!
//! This module is the single place adapter code reads
//! wall-clock time, supplying garbage collection ages and
//! per-process temporary file nonces:
//! sweep planning compares object and temporary file ages
//! against a grace threshold, and ages come from modification
//! times measured against one clock read. Deterministic code
//! never imports this module and never decides anything from a
//! clock; core planning takes ages as inputs (section 10).

use std::time::SystemTime;

/// The current wall-clock time.
///
/// # Panics
///
/// Never panics in the implementation.
#[must_use]
pub fn now() -> SystemTime {
    SystemTime::now()
}

/// The age of a file modified at `modified`, measured against
/// `now`, in whole seconds.
///
/// A modification time after `now` (a clock that moved
/// backwards, or a file written by a host with a faster clock)
/// saturates to an age of zero: the file reads as brand new,
/// which is the safe direction for a grace rule.
///
/// # Panics
///
/// Never panics in the implementation.
#[must_use]
pub fn age_secs(now: SystemTime, modified: SystemTime) -> u64 {
    now.duration_since(modified).map_or(0, |age| age.as_secs())
}
