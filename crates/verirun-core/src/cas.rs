//! Content-addressed store contracts (docs/ARCHITECTURE.md section 6).
//!
//! The pure half of the store: path derivation and temporary file
//! naming as functions of digests and attempt identity, and the put
//! phase model. All I/O lives in `verirun-store`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::digest::Digest;
use crate::manifest::Manifest;

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

/// The store relative path of the pin file for a manifest:
/// `pins/<manifest digest>.pin` (section 6.1). A pin file is
/// empty; its existence in the listing is the whole signal.
#[must_use]
pub fn pin_relative_path(digest: &Digest) -> PathBuf {
    PathBuf::from("pins").join(format!("{digest}.pin"))
}

/// The set of object digests reachable from the pinned manifests
/// (section 10, phase 1; INV-GC-1).
///
/// The set holds, for each pinned manifest: the manifest blob
/// itself, every artifact and log digest its records name, and
/// the task record blob that the by-exec entry for a record's
/// execution digest names, for every record whose execution
/// digest appears in `by_exec_records`. The caller reads the
/// pins, the manifests, and the entries; this function is the
/// pure reachability computation over what was read. Record
/// blobs reachable only through by-exec entries are part of the
/// set by design: sweeping one would silently destroy the cache
/// history of pinned work.
#[allow(
    clippy::missing_const_for_fn,
    reason = "the implementation walks manifests and inserts into a set; it cannot be const"
)]
#[must_use]
pub fn reachable_objects(
    pinned: &[(Digest, Manifest)],
    by_exec_records: &BTreeMap<Digest, Digest>,
) -> BTreeSet<Digest> {
    let mut reachable = BTreeSet::new();
    for (manifest_digest, manifest) in pinned {
        reachable.insert(*manifest_digest);
        for record in &manifest.tasks {
            for digest in record.named_digests() {
                reachable.insert(digest);
            }
            if let Some(record_blob) = by_exec_records.get(&record.exec_digest) {
                reachable.insert(*record_blob);
            }
        }
    }
    reachable
}

/// Selects the sweep candidates among the stored objects
/// (section 10, phase 2; INV-GC-2).
///
/// `objects` maps every object digest in the store to its age
/// in whole seconds, computed by the caller from modification
/// times and the one wall clock read in the adapter. An object
/// is a candidate exactly when it is not in `reachable` and its
/// age is strictly greater than `grace_secs`: an object at
/// exactly the threshold is kept. Candidates come back in
/// digest order.
#[allow(
    clippy::missing_const_for_fn,
    reason = "the implementation iterates the object map; it cannot be const"
)]
#[must_use]
pub fn sweep_candidates(
    objects: &BTreeMap<Digest, u64>,
    reachable: &BTreeSet<Digest>,
    grace_secs: u64,
) -> Vec<Digest> {
    objects
        .iter()
        .filter(|(digest, age)| !reachable.contains(digest) && **age > grace_secs)
        .map(|(digest, _)| *digest)
        .collect()
}

/// Whether a temporary file of this age is stale (section 10,
/// phase 2; INV-GC-3).
///
/// Temporary files take no reachability test: any temp file
/// older than the grace threshold is reclaimed, whatever put
/// left it and whatever its name embeds. The comparison is the
/// same strict one as for objects.
#[must_use]
pub const fn temp_is_stale(age_secs: u64, grace_secs: u64) -> bool {
    age_secs > grace_secs
}
