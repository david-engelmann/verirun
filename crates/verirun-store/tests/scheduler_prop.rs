//! Worker count independence property (INV-RUN-5).
//!
//! For a generated DAG of succeeding tasks, the manifest bytes
//! are identical whether the run used one worker or four, in
//! different stores and different workspaces. Scheduling may
//! change finish order; it may never change a record.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;
use verirun_core::dag::validate_run_spec;
use verirun_core::parse::parse;
use verirun_store::cas_fs::Cas;
use verirun_store::scheduler;
use verirun_store::sha256::Sha256Factory;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_root(label: &str) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "verirun-prop-{label}-{}-{unique}",
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

/// Builds a spec from dependency masks: task `t{i}` depends on
/// task `t{j}` for each bit `j < i` set in mask `i`, and writes a
/// task-specific payload to its own declared output.
fn spec_from_masks(masks: &[u8]) -> String {
    let mut tasks = Vec::new();
    for (i, mask) in masks.iter().enumerate() {
        let deps: Vec<String> = (0..i)
            .filter(|j| mask & (1 << j) != 0)
            .map(|j| format!("\"t{j}\""))
            .collect();
        let payload = "x".repeat(3 + i * 2);
        tasks.push(format!(
            r#"{{"task_id":"t{i}","executable":"sh","args":["-c","printf '{payload}' > outputs/o{i}.txt"],"env":{{}},"inputs":[],"outputs":["o{i}.txt"],"depends_on":[{}]}}"#,
            deps.join(",")
        ));
    }
    format!(r#"{{"tasks":[{}]}}"#, tasks.join(","))
}

fn run_manifest_bytes(spec: &str, jobs: usize, label: &str) -> (Vec<u8>, String) {
    let root = fresh_root(label);
    let cas = Cas::open(root.join("cas")).expect("store opens");
    let value = parse(spec.as_bytes()).expect("spec parses");
    let dag = validate_run_spec(&value).expect("spec validates");
    let outcome = scheduler::run(
        &cas,
        &Sha256Factory,
        &root.join("work"),
        &dag,
        &BTreeMap::new(),
        jobs,
    )
    .expect("run completes");
    assert_eq!(outcome.exit_code, 0);
    let bytes = outcome
        .manifest
        .canonical_bytes()
        .expect("manifest encodes");
    let digest = outcome.manifest_digest.to_hex();
    remove_tree(&root);
    (bytes, digest)
}

proptest! {
    #![proptest_config(ProptestConfig::default())]

    #[test]
    fn prop_worker_count_independence(masks in prop::collection::vec(any::<u8>(), 2..7)) {
        let spec = spec_from_masks(&masks);
        let (bytes_one, digest_one) = run_manifest_bytes(&spec, 1, "one");
        let (bytes_many, digest_many) = run_manifest_bytes(&spec, 4, "many");
        prop_assert_eq!(bytes_one, bytes_many);
        prop_assert_eq!(digest_one, digest_many);
    }
}
