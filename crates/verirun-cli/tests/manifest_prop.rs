//! Manifest determinism property (INV-MAN-1).
//!
//! The same spec, the same input bytes, and the same task
//! behaviour give the same manifest digest at a fixed cache
//! disposition, across worker counts and across host workspaces.
//! A warm run differs from a cold one in exactly the status
//! fields the design carves out (cached in place of succeeded)
//! and in nothing else: execution digests, artifact digests, and
//! log digests are identical.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;
use verirun_cli::run_spec_bytes;
use verirun_core::digest::TaskStatus;
use verirun_store::cas_fs::Cas;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_root(label: &str) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "verirun-manifest-{label}-{}-{unique}",
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

fn spec_from_masks(masks: &[u8]) -> String {
    let mut tasks = Vec::new();
    for (i, mask) in masks.iter().enumerate() {
        let deps: Vec<String> = (0..i)
            .filter(|j| mask & (1 << j) != 0)
            .map(|j| format!("\"t{j}\""))
            .collect();
        let payload = "y".repeat(2 + i * 3);
        tasks.push(format!(
            r#"{{"task_id":"t{i}","executable":"sh","args":["-c","printf '{payload}' > outputs/o{i}.txt"],"env":{{}},"inputs":[],"outputs":["o{i}.txt"],"depends_on":[{}]}}"#,
            deps.join(",")
        ));
    }
    format!(r#"{{"tasks":[{}]}}"#, tasks.join(","))
}

proptest! {
    #![proptest_config(ProptestConfig::default())]

    #[test]
    fn prop_manifest_determinism_across_workers_and_workspaces(
        masks in prop::collection::vec(any::<u8>(), 2..6)
    ) {
        let spec = spec_from_masks(&masks);
        let root_a = fresh_root("alpha");
        let root_b = fresh_root("beta");
        let cas_a = root_a.join("cas");
        let cold_a = run_spec_bytes(
            spec.as_bytes(),
            &cas_a,
            &root_a.join("workspace-alpha"),
            &BTreeMap::new(),
            1,
        )
        .expect("cold run A completes");
        let cold_b = run_spec_bytes(
            spec.as_bytes(),
            &root_b.join("cas"),
            &root_b.join("workspace-beta"),
            &BTreeMap::new(),
            3,
        )
        .expect("cold run B completes");
        prop_assert_eq!(cold_a.manifest_digest, cold_b.manifest_digest);
        prop_assert_eq!(
            cold_a.manifest.canonical_bytes().expect("encodes"),
            cold_b.manifest.canonical_bytes().expect("encodes")
        );

        // Warm run over store A: only the status fields move.
        let warm = run_spec_bytes(
            spec.as_bytes(),
            &cas_a,
            &root_a.join("workspace-alpha-again"),
            &BTreeMap::new(),
            2,
        )
        .expect("warm run completes");
        prop_assert_eq!(warm.manifest.tasks.len(), cold_a.manifest.tasks.len());
        for (cold, warm_rec) in cold_a.manifest.tasks.iter().zip(warm.manifest.tasks.iter()) {
            prop_assert_eq!(cold.task_id.clone(), warm_rec.task_id.clone());
            prop_assert_eq!(cold.exec_digest, warm_rec.exec_digest);
            prop_assert_eq!(cold.artifacts.clone(), warm_rec.artifacts.clone());
            prop_assert_eq!(cold.stdout_digest, warm_rec.stdout_digest);
            prop_assert_eq!(cold.stderr_digest, warm_rec.stderr_digest);
            match (cold.status, warm_rec.status) {
                (TaskStatus::Succeeded, TaskStatus::Cached) => {}
                (cold_status, warm_status) => {
                    prop_assert_eq!(cold_status, warm_status);
                }
            }
        }
        // The warm run really did consult the store it reused.
        prop_assert!(Cas::open(&cas_a).expect("store reopens").contains(&warm.manifest_digest));
        remove_tree(&root_a);
        remove_tree(&root_b);
    }
}
