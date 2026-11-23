//! Garbage collection execution (docs/ARCHITECTURE.md
//! section 10).
//!
//! Collection runs in three phases. Mark: every pinned
//! manifest is read and verified, and the reachable set is
//! computed by the pure planner in
//! [`verirun_core::cas::reachable_objects`]. Plan: objects and
//! temporary files are aged against one wall clock read and
//! the grace threshold, again through core planning. Sweep:
//! under `gc.lock`, the planned files are deleted. A dry run
//! stops after the plan and deletes nothing (INV-GC-4).
//!
//! Collection is a maintenance operation outside the
//! determinism boundary: it changes what is stored, never what
//! a digest means.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use verirun_core::cas::{
    by_exec_relative_path, reachable_objects, sweep_candidates, temp_is_stale,
};
use verirun_core::digest::Digest;
use verirun_core::manifest::Manifest;
use verirun_core::parse::parse;

use crate::cas_fs::{Cas, remove_stored_file};
use crate::clock;
use crate::error::{GcError, StoreError, StoreStep};

/// The outcome of one collection: what was deleted, or on a
/// dry run what would be deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcReport {
    /// Whether this run stopped after planning and deleted
    /// nothing.
    pub dry_run: bool,
    /// How many objects were (or would be) deleted.
    pub objects: usize,
    /// The total byte size of those objects.
    pub object_bytes: u64,
    /// How many temporary files were (or would be) deleted.
    pub tmp_files: usize,
    /// The total byte size of those temporary files.
    pub tmp_bytes: u64,
    /// How many objects are reachable from pins and were
    /// therefore kept regardless of age.
    pub reachable: usize,
}

impl GcReport {
    /// An empty report for a run of the given kind.
    #[must_use]
    pub const fn new(dry_run: bool) -> Self {
        Self {
            dry_run,
            objects: 0,
            object_bytes: 0,
            tmp_files: 0,
            tmp_bytes: 0,
            reachable: 0,
        }
    }

    /// The total files deleted (or planned) across objects and
    /// temporary files.
    #[must_use]
    pub const fn files(&self) -> usize {
        self.objects + self.tmp_files
    }

    /// The total bytes reclaimed (or that would be reclaimed).
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.object_bytes + self.tmp_bytes
    }
}

/// Collects garbage in `cas` (section 10).
///
/// Every age in the plan derives from file modification times
/// measured against `now`, which the caller reads once through
/// [`crate::clock::now`]; passing an explicit `now` is what
/// lets tests age files deterministically. Objects and
/// temporary files older than `grace_secs` and unreachable
/// from any pin are deleted, unless `dry_run` is set, in which
/// case the plan is reported and nothing is deleted.
///
/// # Errors
///
/// Returns [`GcError::MissingPinnedManifest`] or
/// [`GcError::InvalidPinnedManifest`] when a pin's manifest
/// cannot be produced and verified, [`GcError::LockHeld`] when
/// another sweep holds the lock on a real run, and
/// [`GcError::Store`] for store failures. On every error the
/// sweep deletes nothing.
///
/// # Panics
///
/// Never panics in the implementation.
#[allow(
    clippy::missing_const_for_fn,
    reason = "the implementation performs filesystem I/O; it cannot be const"
)]
pub fn collect(
    cas: &Cas,
    grace_secs: u64,
    dry_run: bool,
    now: SystemTime,
) -> Result<GcReport, GcError> {
    // Phase 1, mark: read and parse every pinned manifest,
    // with the by-exec record blobs those manifests use.
    let pinned = read_pinned_manifests(cas)?;
    let by_exec_records = read_by_exec_entries(cas, &pinned);
    let reachable = reachable_objects(&pinned, &by_exec_records);

    // Phase 2, plan: age every object and temporary file
    // against the one clock read.
    let mut report = GcReport::new(dry_run);
    report.reachable = reachable.len();
    let objects_dir = cas.root().join("objects");
    let mut ages: BTreeMap<Digest, u64> = BTreeMap::new();
    let mut sizes: BTreeMap<Digest, u64> = BTreeMap::new();
    for shard in list_dir(&objects_dir)? {
        if !shard.is_dir() {
            continue;
        }
        for file in list_dir(&shard)? {
            let Some(name) = file
                .file_name()
                .and_then(|name| name.to_str().map(str::to_owned))
            else {
                continue;
            };
            let Ok(digest) = Digest::from_hex(&name) else {
                continue;
            };
            let (age, size) = age_and_size(&file, now)?;
            ages.insert(digest, age);
            sizes.insert(digest, size);
        }
    }
    let candidates = sweep_candidates(&ages, &reachable, grace_secs);
    report.objects = candidates.len();
    report.object_bytes = candidates
        .iter()
        .filter_map(|digest| sizes.get(digest))
        .sum();

    let tmp_dir = cas.root().join("tmp");
    let mut tmp_plan: Vec<PathBuf> = Vec::new();
    for file in list_dir(&tmp_dir)? {
        if !file.is_file() {
            continue;
        }
        let (age, size) = age_and_size(&file, now)?;
        if temp_is_stale(age, grace_secs) {
            tmp_plan.push(file);
            report.tmp_bytes += size;
        }
    }
    report.tmp_files = tmp_plan.len();

    if dry_run {
        return Ok(report);
    }

    // Phase 3, sweep: under the lock, delete exactly the plan.
    let _lock = LockGuard::take(&cas.root().join("gc.lock"), now)?;
    for digest in &candidates {
        let path = cas.object_path(digest);
        remove_stored_file(&path).map_err(|source| GcError::Store {
            source: StoreError::Io {
                path: path.clone(),
                step: StoreStep::RemoveObject,
                source,
            },
        })?;
    }
    for path in &tmp_plan {
        remove_stored_file(path).map_err(|source| GcError::Store {
            source: StoreError::Io {
                path: path.clone(),
                step: StoreStep::RemoveTemp,
                source,
            },
        })?;
    }
    Ok(report)
}

/// Reads and parses every pinned manifest. A pin whose
/// manifest cannot be produced stops the sweep: deleting
/// around a broken pin could destroy exactly the reachability
/// the pin protects (INV-GC-1).
fn read_pinned_manifests(cas: &Cas) -> Result<Vec<(Digest, Manifest)>, GcError> {
    let mut pinned: Vec<(Digest, Manifest)> = Vec::new();
    for digest in cas.pinned_manifests()? {
        let bytes = match cas.read_to_vec(&digest) {
            Ok(bytes) => bytes,
            Err(StoreError::Missing { .. }) => {
                return Err(GcError::MissingPinnedManifest { digest });
            }
            Err(StoreError::Integrity { actual, .. }) => {
                return Err(GcError::InvalidPinnedManifest {
                    digest,
                    detail: format!("the stored bytes hash to {actual}"),
                });
            }
            Err(source) => return Err(GcError::Store { source }),
        };
        let value = parse(&bytes).map_err(|source| GcError::InvalidPinnedManifest {
            digest,
            detail: format!("the stored bytes do not parse: {source}"),
        })?;
        let manifest =
            Manifest::from_value(&value).map_err(|source| GcError::InvalidPinnedManifest {
                digest,
                detail: format!("the stored bytes are not a manifest: {source}"),
            })?;
        pinned.push((digest, manifest));
    }
    Ok(pinned)
}

/// The by-exec entries the pinned records use. An absent or
/// malformed entry names nothing, exactly as the cache lookup
/// treats it: a miss, never an error.
fn read_by_exec_entries(cas: &Cas, pinned: &[(Digest, Manifest)]) -> BTreeMap<Digest, Digest> {
    let mut by_exec_records: BTreeMap<Digest, Digest> = BTreeMap::new();
    for (_, manifest) in pinned {
        for record in &manifest.tasks {
            let entry_path = cas.root().join(by_exec_relative_path(&record.exec_digest));
            if let Ok(text) = fs::read_to_string(&entry_path)
                && let Ok(record_blob) = Digest::from_hex(text.trim())
            {
                by_exec_records.insert(record.exec_digest, record_blob);
            }
        }
    }
    by_exec_records
}

/// Lists one directory's entries as paths.
fn list_dir(dir: &Path) -> Result<Vec<PathBuf>, GcError> {
    let entries = fs::read_dir(dir).map_err(|source| GcError::Store {
        source: StoreError::Io {
            path: dir.to_path_buf(),
            step: StoreStep::ListObjects,
            source,
        },
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| GcError::Store {
            source: StoreError::Io {
                path: dir.to_path_buf(),
                step: StoreStep::ListObjects,
                source,
            },
        })?;
        paths.push(entry.path());
    }
    Ok(paths)
}

/// One file's age against `now` and its byte size, from its
/// metadata.
fn age_and_size(path: &Path, now: SystemTime) -> Result<(u64, u64), GcError> {
    let metadata = fs::metadata(path).map_err(|source| GcError::Store {
        source: StoreError::Io {
            path: path.to_path_buf(),
            step: StoreStep::ListObjects,
            source,
        },
    })?;
    let modified = metadata.modified().map_err(|source| GcError::Store {
        source: StoreError::Io {
            path: path.to_path_buf(),
            step: StoreStep::ListObjects,
            source,
        },
    })?;
    Ok((clock::age_secs(now, modified), metadata.len()))
}

/// The age past which a leftover `gc.lock` counts as stale
/// (F8). A sweep is bounded work measured in seconds, so a lock
/// a day old can only be the debris of a collection that died
/// without dropping its guard, and breaking it is safe.
const STALE_LOCK_AGE_SECS: u64 = 86_400;

/// The held `gc.lock`: an exclusive create-new file whose
/// removal on drop ends the sweep, however it ends.
struct LockGuard {
    path: PathBuf,
}

impl LockGuard {
    fn take(path: &Path, now: SystemTime) -> Result<Self, GcError> {
        match Self::create(path) {
            Ok(guard) => Ok(guard),
            Err(GcError::LockHeld { .. }) => {
                // A lock already stands. Break it only when it
                // is stale by the collector's one clock read
                // (F8), then try exactly once more: a second
                // refusal means a live collection won the race
                // in between, and its lock is fresh.
                if !Self::is_stale(path, now) {
                    return Err(GcError::LockHeld {
                        path: path.to_path_buf(),
                    });
                }
                let _ = fs::remove_file(path);
                Self::create(path)
            }
            Err(other) => Err(other),
        }
    }

    fn create(path: &Path) -> Result<Self, GcError> {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(_file) => Ok(Self {
                path: path.to_path_buf(),
            }),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Err(GcError::LockHeld {
                path: path.to_path_buf(),
            }),
            Err(source) => Err(GcError::Store {
                source: StoreError::Io {
                    path: path.to_path_buf(),
                    step: StoreStep::CreateLock,
                    source,
                },
            }),
        }
    }

    fn is_stale(path: &Path, now: SystemTime) -> bool {
        let Ok(metadata) = fs::metadata(path) else {
            // The lock vanished between the refusal and this
            // read, so the retry in take() decides.
            return true;
        };
        let Ok(modified) = metadata.modified() else {
            return false;
        };
        clock::age_secs(now, modified) > STALE_LOCK_AGE_SECS
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
