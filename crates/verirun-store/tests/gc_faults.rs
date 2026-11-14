//! Garbage collection fault oracles (INV-GC-1 through
//! INV-GC-4; docs/ARCHITECTURE.md section 10).
//!
//! Ages are deterministic without touching file times: the
//! collector takes its `now` as an input, so a test captures a
//! base instant before writing, then collects against a later
//! fabricated instant to age everything it wrote by a known
//! amount. Margins around the grace threshold are minutes
//! wide, far above any scheduling jitter in the writes.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use verirun_core::cas::by_exec_relative_path;
use verirun_core::digest::{Digest, TaskStatus};
use verirun_core::manifest::{ArtifactRecord, Manifest, TaskRecord};
use verirun_store::cas_fs::Cas;
use verirun_store::gc::collect;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "verirun-gc-{tag}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

const fn digest(byte: u8) -> Digest {
    Digest::from_bytes([byte; 32])
}

fn artifact(path: &str, content: Digest) -> ArtifactRecord {
    ArtifactRecord {
        path: path.to_string(),
        digest: content,
        bytes: 4,
        executable: false,
    }
}

fn record(
    task_id: &str,
    exec: u8,
    stdout: Digest,
    stderr: Digest,
    artifacts: Vec<ArtifactRecord>,
) -> TaskRecord {
    TaskRecord::new(
        task_id.to_string(),
        digest(exec),
        TaskStatus::Succeeded,
        Some(0),
        stdout,
        stderr,
        artifacts,
    )
}

/// Stores a manifest built from `records` and returns its blob
/// digest.
fn store_manifest(cas: &Cas, records: Vec<TaskRecord>) -> Digest {
    let manifest = Manifest {
        tasks: records,
        merkle_root: digest(250),
    };
    let bytes = manifest.canonical_bytes().expect("encode manifest");
    cas.put_bytes(&bytes).expect("store manifest")
}

/// Writes a by-exec entry naming `record_blob` for `exec`, in
/// the scheduler's on-disk form.
fn write_by_exec_entry(cas: &Cas, exec: Digest, record_blob: Digest) {
    let path = cas.root().join(by_exec_relative_path(&exec));
    fs::create_dir_all(path.parent().expect("entry has a parent")).expect("entry dir");
    fs::write(&path, record_blob.to_hex()).expect("write entry");
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let rel = path.strip_prefix(root).expect("under root").to_path_buf();
                out.insert(rel, fs::read(&path).expect("read file"));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn fault_gc_preserves_pinned_reachability() {
    let root = fresh_dir("pinned");
    let cas = Cas::open(&root).expect("open store");
    let base = SystemTime::now();

    // Two manifests sharing one artifact blob. Only the first
    // is pinned. The pinned record also has a by-exec entry
    // naming its record blob.
    let shared = cas.put_bytes(b"shared artifact").expect("put shared");
    let pinned_only = cas.put_bytes(b"pinned only").expect("put pinned only");
    let unpinned_only = cas.put_bytes(b"unpinned only").expect("put unpinned");
    let pinned_out = cas.put_bytes(b"pinned stdout").expect("put");
    let pinned_err = cas.put_bytes(b"pinned stderr").expect("put");
    let unpinned_out = cas.put_bytes(b"unpinned stdout").expect("put");
    let unpinned_err = cas.put_bytes(b"unpinned stderr").expect("put");
    let record_blob = cas.put_bytes(b"pinned task record").expect("put record");

    let pinned_manifest = store_manifest(
        &cas,
        vec![record(
            "kept",
            30,
            pinned_out,
            pinned_err,
            vec![
                artifact("out/shared", shared),
                artifact("out/own", pinned_only),
            ],
        )],
    );
    let unpinned_manifest = store_manifest(
        &cas,
        vec![record(
            "dropped",
            31,
            unpinned_out,
            unpinned_err,
            vec![
                artifact("out/shared", shared),
                artifact("out/own", unpinned_only),
            ],
        )],
    );
    write_by_exec_entry(&cas, digest(30), record_blob);
    cas.pin_manifest(&pinned_manifest).expect("pin");

    let report = collect(&cas, 3600, false, base + Duration::from_secs(7200)).expect("collect");
    assert_eq!(
        report.objects, 4,
        "the unpinned manifest and its unshared blobs"
    );
    assert_eq!(
        report.reachable, 6,
        "manifest, two artifacts, two logs, record blob"
    );
    for kept in [
        pinned_manifest,
        shared,
        pinned_only,
        pinned_out,
        pinned_err,
        record_blob,
    ] {
        assert!(cas.contains(&kept), "reachable object {kept} must survive");
    }
    for swept in [unpinned_manifest, unpinned_only, unpinned_out, unpinned_err] {
        assert!(
            !cas.contains(&swept),
            "unreachable object {swept} must be swept"
        );
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn fault_gc_grace_boundary() {
    let root = fresh_dir("grace");
    let cas = Cas::open(&root).expect("open store");
    let base = SystemTime::now();
    let blob = cas.put_bytes(b"unreachable").expect("put");

    // One second below the threshold (within write jitter):
    // kept.
    let report = collect(&cas, 3600, false, base + Duration::from_secs(3599)).expect("collect");
    assert_eq!(report.objects, 0);
    assert!(cas.contains(&blob));

    // Past the threshold: swept.
    let report = collect(&cas, 3600, false, base + Duration::from_secs(3601)).expect("collect");
    assert_eq!(report.objects, 1);
    assert!(!cas.contains(&blob));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn fault_gc_reclaims_stale_tmp_only() {
    let root = fresh_dir("tmp");
    let cas = Cas::open(&root).expect("open store");
    let base = SystemTime::now();

    let kept = cas.put_bytes(b"pinned artifact").expect("put");
    let manifest = store_manifest(
        &cas,
        vec![record("t", 30, kept, kept, vec![artifact("out/a", kept)])],
    );
    cas.pin_manifest(&manifest).expect("pin");

    // Two temporary files: an ordinary leftover, and one whose
    // name embeds the pinned manifest's digest (INV-GC-3 edge).
    let tmp_dir = root.join("tmp");
    let stray = tmp_dir.join("put-123-0-0.tmp");
    let named = tmp_dir.join(manifest.to_hex());
    fs::write(&stray, b"partial put").expect("write stray tmp");
    fs::write(&named, b"partial put").expect("write named tmp");

    // Young temporary files are not reclaimed.
    let report = collect(&cas, 3600, false, base + Duration::from_secs(10)).expect("collect");
    assert_eq!(report.tmp_files, 0);
    assert!(stray.exists() && named.exists());

    // Stale temporary files are reclaimed whatever their names
    // embed; the pinned object the name points at is untouched.
    let report = collect(&cas, 3600, false, base + Duration::from_secs(7200)).expect("collect");
    assert_eq!(report.tmp_files, 2);
    assert!(!stray.exists() && !named.exists());
    assert!(cas.contains(&manifest));
    assert!(cas.contains(&kept));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn fault_gc_dry_run_deletes_nothing() {
    let root = fresh_dir("dry");
    let cas = Cas::open(&root).expect("open store");
    let base = SystemTime::now();

    let kept = cas.put_bytes(b"pinned artifact").expect("put");
    let manifest = store_manifest(
        &cas,
        vec![record("t", 30, kept, kept, vec![artifact("out/a", kept)])],
    );
    cas.pin_manifest(&manifest).expect("pin");
    let stale = cas.put_bytes(b"unreachable and old").expect("put");
    let stray = root.join("tmp").join("leftover.tmp");
    fs::write(&stray, b"partial").expect("write tmp");

    let before = snapshot(&root);
    let dry = collect(&cas, 3600, true, base + Duration::from_secs(7200)).expect("dry collect");
    assert!(dry.dry_run);
    assert_eq!(dry.objects, 1, "the plan names the stale object");
    assert_eq!(dry.tmp_files, 1, "the plan names the stale temp file");
    assert_eq!(snapshot(&root), before, "a dry run changes no byte");

    // A real run on the unchanged store executes exactly the
    // printed plan.
    let real = collect(&cas, 3600, false, base + Duration::from_secs(7200)).expect("collect");
    assert_eq!(real.objects, dry.objects);
    assert_eq!(real.object_bytes, dry.object_bytes);
    assert_eq!(real.tmp_files, dry.tmp_files);
    assert!(!cas.contains(&stale));
    assert!(!stray.exists());
    assert!(cas.contains(&manifest) && cas.contains(&kept));
    fs::remove_dir_all(&root).ok();
}
