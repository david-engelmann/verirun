//! Skipped record verification oracle (INV-RUN-6, INV-MAN-5
//! groundwork).
//!
//! A manifest containing skipped records verifies clean: every
//! digest a skipped record names (its execution digest's inputs
//! aside, its two empty log blobs) names a blob that is present
//! in the store the run produced.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use verirun_cli::{run_spec_bytes, verify_manifest};
use verirun_core::digest::TaskStatus;
use verirun_store::cas_fs::Cas;

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
