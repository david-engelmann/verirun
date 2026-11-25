//! Hand-rolled manifest verification benchmark (the
//! workspace pins no benchmark framework; see DECISIONS.md
//! Q5 and the M4 bench design). Run with:
//!
//! ```text
//! cargo bench -p verirun-cli --bench verify_bench
//! ```
//!
//! The workload is the one roadmap lane R4 prices: a
//! manifest naming many distinct blobs, verified end to
//! end. Setup runs a 128-task spec whose tasks each write
//! a distinct 256 KiB artifact, so one verification pass
//! re-hashes about 32 MiB across 129 distinct objects. The
//! timed region is `verify_manifest` alone.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use verirun_cli::{run_spec_bytes, verify_manifest};
use verirun_store::cas_fs::Cas;

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
                        reason = "bench cleanup must remove read-only store objects"
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

fn fresh_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "verirun-bench-verify-{label}-{}",
        std::process::id()
    ));
    remove_tree(&root);
    root
}

/// Builds the spec: `tasks` independent tasks, each writing
/// an artifact filled with a repeating unit that names the
/// task, so every artifact digest is distinct at any task
/// count.
fn spec(tasks: u32) -> String {
    let mut text = String::from("{\"tasks\":[");
    for index in 0..tasks {
        if index > 0 {
            text.push(',');
        }
        let _ = write!(
            text,
            "{{\"task_id\":\"t{index}\",\"executable\":\"sh\",\"args\":[\"-c\",\"yes 'task-{index}' | head -c 262144 > outputs/a{index}.txt\"],\"env\":{{}},\"inputs\":[],\"outputs\":[\"a{index}.txt\"],\"depends_on\":[]}}"
        );
    }
    text.push_str("]}");
    text
}

fn bench_verify_manifest(smoke: bool) {
    let root = fresh_root("manifest");
    let cas_root = root.join("cas");
    let outcome = run_spec_bytes(
        spec(128).as_bytes(),
        &cas_root,
        &root.join("work"),
        &BTreeMap::new(),
        4,
    )
    .expect("the bench run succeeds");
    let cas = Cas::open(&cas_root).expect("store opens");
    let iterations = if smoke { 1 } else { 20 };
    let started = Instant::now();
    for _ in 0..iterations {
        let manifest = verify_manifest(&cas, &outcome.manifest_digest).expect("verifies");
        assert_eq!(manifest.tasks.len(), 128);
    }
    let elapsed = started.elapsed().as_secs_f64();
    let per_sec = f64::from(iterations) / elapsed;
    println!(
        "bench_verify_manifest: {iterations} verifications of a 128-task manifest in {elapsed:.3}s = {per_sec:.2} verifications/s"
    );
    remove_tree(&root);
}

fn main() {
    let smoke = std::env::args().any(|arg| arg == "--test");
    bench_verify_manifest(smoke);
}
