//! Content-addressed store contracts (docs/ARCHITECTURE.md section 6).
//!
//! The pure half of the store: path derivation and temporary file
//! naming as functions of digests and attempt identity, and the put
//! phase model. All I/O lives in `verirun-store`.

use std::path::PathBuf;

use crate::digest::Digest;

/// The fixed streaming buffer size for store transfers, in bytes.
/// Memory use of a put or a get is bounded by this buffer, independent
/// of blob size (INV-CAS-8).
pub const STREAM_BUFFER_BYTES: usize = 64 * 1024;

/// The two-hex-character shard prefix of a digest: the first two
/// characters of its hex form. The fan-out prefix is a performance
/// device only and carries no meaning.
#[must_use]
pub fn shard_prefix(digest: &Digest) -> String {
    digest.to_hex()[..2].to_string()
}

/// The store-relative path of an object: `objects/<xx>/<digest>`.
#[must_use]
pub fn object_relative_path(digest: &Digest) -> PathBuf {
    PathBuf::from("objects")
        .join(shard_prefix(digest))
        .join(digest.to_hex())
}

/// The store-relative path of a by-exec cache entry:
/// `by-exec/<xx>/<exec digest>`.
#[must_use]
pub fn by_exec_relative_path(exec_digest: &Digest) -> PathBuf {
    PathBuf::from("by-exec")
        .join(shard_prefix(exec_digest))
        .join(exec_digest.to_hex())
}

/// The temporary file name for a put whose digest is already known:
/// `<digest>.<pid>.<worker>.<counter>.tmp`, created inside `<cas>/tmp/`.
/// Temporary names never enter any digest.
#[must_use]
pub fn temp_file_name_known(digest: &Digest, pid: u32, worker: u32, counter: u64) -> String {
    format!("{}.{pid}.{worker}.{counter}.tmp", digest.to_hex())
}

/// The temporary file name for a streamed put, whose digest is known
/// only at end of stream: `stream-<pid>.<worker>.<counter>.tmp`.
#[must_use]
pub fn temp_file_name_stream(pid: u32, worker: u32, counter: u64) -> String {
    format!("stream-{pid}.{worker}.{counter}.tmp")
}

/// One phase of the put state machine.
///
/// The state machine is docs/ARCHITECTURE.md section 6.2.
/// `verirun-store` drives a put through `Writing`, `FileSynced`, and
/// `Renamed` to `Durable`. A put abandoned at any point before
/// `Durable` leaves at most a temporary file for garbage collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PutPhase {
    /// Bytes are streaming into the temporary file.
    Writing,
    /// The temporary file has been synced with `sync_all`.
    FileSynced,
    /// The temporary file has been renamed to its object path.
    Renamed,
    /// The destination directory sync completed. The put may now
    /// report success.
    Durable,
    /// The put was abandoned before `Durable`.
    Abandoned,
}
