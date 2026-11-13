//! verirun-core: the deterministic core of verirun.
//!
//! This crate is strictly standard-library only (DECISIONS.md Q5). It does
//! no filesystem, process, network, clock, or randomness access. Every
//! behaviour that can influence a digest lives here as pure computation
//! over bytes and values handed to it by adapter crates.
//!
//! Module map (docs/ARCHITECTURE.md section 2.1):
//!
//! - [`value`]: the JSON value model canonicalization operates on.
//! - [`parse`]: the strict JSON parser. Duplicate keys, malformed
//!   numbers, lone surrogates, and nesting past the depth limit are all
//!   rejected with byte offsets.
//! - [`canonical`]: the RFC 8785 (JCS) canonical encoder, including the
//!   ECMAScript number layout over the value-based safe-integer domain.
//! - [`digest`]: digest framing, domain separation, the
//!   [`digest::StreamingHasher`] and [`digest::HasherFactory`] seam, and
//!   the execution tuple encoding.
//! - [`cas`]: content-addressed store contracts. Path derivation and
//!   temporary file naming as pure functions, plus the put phase model.
//! - [`error`]: all core error types, each carrying diagnostic context.
//!
//! The `dag`, `paths`, and `manifest` modules of the architecture land
//! in Milestone 2 with the scheduler.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod canonical;
pub mod cas;
pub mod digest;
pub mod error;
pub mod parse;
pub mod value;
