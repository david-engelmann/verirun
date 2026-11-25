//! Skipped record verification oracle (INV-RUN-6, INV-MAN-5
//! groundwork) and the single-pass verification oracle
//! (INV-LAT-3).
//!
//! A manifest containing skipped records verifies clean: every
//! digest a skipped record names (its execution digest's inputs
//! aside, its two empty log blobs) names a blob that is present
//! in the store the run produced.
//!
//! The single-pass oracle runs a manifest whose eight tasks all
//! name one shared artifact blob and proves, through a counting
//! read observer, that verification reads the manifest and each
//! distinct named blob exactly once, however many records
//! reference it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use verirun_cli::{run_spec_bytes, verify_manifest, verify_manifest_observed};
use verirun_core::digest::{Digest, TaskStatus};
use verirun_store::cas_fs::{Cas, ReadObserver};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_root(label: &str) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "verirun-mfault-{label}-{}-{unique}",
        std::process::id()
    ))
}

fn remove_tree(root: &Path) {
    fn clear(path: &Path) {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() && !entry_path.is_symlink() {
                    clear(&entry_path);
                } else if let Ok(meta) = std::fs::metadata(&entry_path) {
                    let mut perms = meta.permissions();
                    #[allow(
                        clippy::permissions_set_readonly_false,
                        reason = "test cleanup must remove read-only fixtures"
                    )]
                    perms.set_readonly(false);
                    let _ = std::fs::set_permissions(&entry_path, perms);
                }
            }
        }
    }
    clear(root);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn fault_skipped_records_verify_clean() {
    let root = fresh_root("skipped");
    let cas_root = root.join("cas");
    let spec = r#"{"tasks":[
        {"task_id":"root","executable":"sh","args":["-c","exit 1"],"env":{},"inputs":[],"outputs":[],"depends_on":[]},
        {"task_id":"child","executable":"sh","args":["-c","printf c > outputs/c.txt"],"env":{},"inputs":[],"outputs":["c.txt"],"depends_on":["root"]},
        {"task_id":"grandchild","executable":"sh","args":["-c","printf g > outputs/g.txt"],"env":{},"inputs":[],"outputs":["g.txt"],"depends_on":["child"]}
    ]}"#;
    let outcome = run_spec_bytes(
        spec.as_bytes(),
        &cas_root,
        &root.join("work"),
        &BTreeMap::new(),
        2,
    )
    .expect("run completes");
    assert_eq!(outcome.exit_code, 1);
    let statuses: BTreeMap<&str, TaskStatus> = outcome
        .manifest
        .tasks
        .iter()
        .map(|rec| (rec.task_id.as_str(), rec.status))
        .collect();
    assert_eq!(statuses["root"], TaskStatus::Failed);
    assert_eq!(statuses["child"], TaskStatus::Skipped);
    assert_eq!(statuses["grandchild"], TaskStatus::Skipped);

    // The manifest verifies against the very store the run
    // produced, with no additions: the empty blob the skipped
    // records name was put by the run itself.
    let cas = Cas::open(&cas_root).expect("store reopens");
    let verified = verify_manifest(&cas, &outcome.manifest_digest)
        .expect("manifest with skipped records verifies");
    assert_eq!(verified, outcome.manifest);
    remove_tree(&root);
}

#[derive(Debug, Default)]
struct CountingReads {
    counts: BTreeMap<Digest, usize>,
}

impl ReadObserver for CountingReads {
    fn on_read(&mut self, digest: &Digest) {
        *self.counts.entry(*digest).or_insert(0) += 1;
    }
}

#[test]
fn fault_verify_single_pass_counting_reader() {
    let root = fresh_root("single-pass");
    let cas_root = root.join("cas");
    // Eight tasks, each writing the same artifact bytes under its
    // own output path (so all eight records name one shared blob
    // digest) and its own stdout text.
    let mut tasks = String::new();
    for index in 0..8 {
        if index > 0 {
            tasks.push(',');
        }
        write!(
            tasks,
            r#"{{"task_id": "w{index}", "executable": "sh",
                "args": ["-c", "printf shared-content > outputs/shared-{index}.bin; printf out-{index}"],
                "env": {{}}, "inputs": [], "outputs": ["shared-{index}.bin"],
                "depends_on": []}}"#
        )
        .expect("writing to a String cannot fail");
    }
    let spec = format!("{{\"tasks\":[{tasks}]}}");
    let outcome = run_spec_bytes(
        spec.as_bytes(),
        &cas_root,
        &root.join("work"),
        &BTreeMap::new(),
        4,
    )
    .expect("run completes");
    assert_eq!(outcome.exit_code, 0);
    let shared = outcome.manifest.tasks[0].artifacts[0].digest;
    assert!(
        outcome
            .manifest
            .tasks
            .iter()
            .all(|record| record.artifacts[0].digest == shared),
        "all eight records name the one shared blob"
    );

    // The distinct objects one verify run may read: the manifest
    // itself plus every distinct digest its records name.
    let mut expected: BTreeSet<Digest> = BTreeSet::new();
    expected.insert(outcome.manifest_digest);
    for record in &outcome.manifest.tasks {
        for artifact in &record.artifacts {
            expected.insert(artifact.digest);
        }
        expected.insert(record.stdout_digest);
        expected.insert(record.stderr_digest);
    }

    let cas = Cas::open(&cas_root).expect("store reopens");
    let mut counter = CountingReads::default();
    let verified = verify_manifest_observed(&cas, &outcome.manifest_digest, &mut counter)
        .expect("the shared-blob manifest verifies");
    assert_eq!(verified, outcome.manifest);
    assert_eq!(
        counter.counts.len(),
        expected.len(),
        "verify read exactly the distinct object set, no rescans"
    );
    for (digest, count) in &counter.counts {
        assert!(expected.contains(digest), "verify read an unnamed object");
        assert_eq!(*count, 1, "object {digest} was read exactly once");
    }
    assert_eq!(
        counter.counts[&shared], 1,
        "the blob shared by eight records was read once, not eight times"
    );
    remove_tree(&root);
}

#[test]
fn fault_verify_from_relocated_store() {
    // INV-MAN-2: a manifest produced against one store
    // verifies against a byte copy of that store at a
    // different absolute path, because nothing in
    // verification depends on the producing host's paths.
    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("create copy dir");
        for entry in std::fs::read_dir(from).expect("read dir").flatten() {
            let target = to.join(entry.file_name());
            if entry.file_type().expect("file type").is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).expect("copy file");
            }
        }
    }
    let root = fresh_root("relocated");
    let cas_root = root.join("cas");
    let spec = r#"{"tasks":[{"task_id":"only","executable":"sh","args":["-c","printf moved > outputs/out.txt"],"env":{},"inputs":[],"outputs":["out.txt"],"depends_on":[]}]}"#;
    let outcome = run_spec_bytes(
        spec.as_bytes(),
        &cas_root,
        &root.join("work"),
        &BTreeMap::new(),
        1,
    )
    .expect("run completes");
    assert_eq!(outcome.exit_code, 0);
    let moved_root = root.join("elsewhere").join("cas");
    copy_tree(&cas_root, &moved_root);
    let moved = Cas::open(&moved_root).expect("relocated store opens");
    let verified = verify_manifest(&moved, &outcome.manifest_digest)
        .expect("manifest verifies from the relocated store");
    assert_eq!(verified, outcome.manifest);
    remove_tree(&root);
}
