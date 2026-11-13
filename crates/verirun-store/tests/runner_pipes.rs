//! Pipe drain stress oracle (INV-RUN-3).
//!
//! Both streams are filled far past the OS pipe capacity in
//! alternating chunks. A runner that drained the streams
//! sequentially, or waited for exit before draining, would
//! deadlock here: the child blocks on a full pipe while the
//! runner blocks on the other stream or on the child.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use verirun_core::dag::validate_run_spec;
use verirun_core::digest::TaskStatus;
use verirun_core::parse::parse;
use verirun_store::cas_fs::Cas;
use verirun_store::scheduler;
use verirun_store::sha256::Sha256Factory;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_root(label: &str) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "verirun-pipes-{label}-{}-{unique}",
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
fn stress_both_pipes_past_capacity() {
    let root = fresh_root("flood");
    let cas_root = root.join("cas");
    let work_root = root.join("work");
    // 100 chunks of 1024 bytes per stream, alternating: 102,400
    // bytes on each stream, past the 64 KiB pipe capacity.
    let script = "i=0; while [ $i -lt 100 ]; do yes a | head -c 1024; yes b | head -c 1024 >&2; i=$((i+1)); done";
    let spec = format!(
        r#"{{"tasks":[{{"task_id":"flood","executable":"sh","args":["-c","{script}"],"env":{{}},"inputs":[],"outputs":[],"depends_on":[]}}]}}"#
    );
    let value = parse(spec.as_bytes()).expect("spec parses");
    let dag = validate_run_spec(&value).expect("spec validates");
    let cas = Cas::open(&cas_root).expect("store opens");
    let outcome = scheduler::run(&cas, &Sha256Factory, &work_root, &dag, &BTreeMap::new(), 1)
        .expect("run completes without deadlock");
    let record = &outcome.manifest.tasks[0];
    assert_eq!(record.status, TaskStatus::Succeeded);

    // Raw bytes are preserved exactly: the streams are the
    // repeating two-byte pattern of the generator, unterminated
    // and unnormalized.
    let expected_out: Vec<u8> = "a\n".repeat(51_200).into_bytes();
    let expected_err: Vec<u8> = "b\n".repeat(51_200).into_bytes();
    assert_eq!(expected_out.len(), 102_400);
    let stdout = cas
        .read_to_vec(&record.stdout_digest)
        .expect("stdout blob reads");
    let stderr = cas
        .read_to_vec(&record.stderr_digest)
        .expect("stderr blob reads");
    assert_eq!(stdout, expected_out);
    assert_eq!(stderr, expected_err);
    remove_tree(&root);
}
