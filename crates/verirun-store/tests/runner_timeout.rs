//! Timeout fault oracles (INV-RUN-4).
//!
//! A task past its deadline is killed and reaped, its drains
//! finish and are joined, its dependents are skipped, and the run
//! completes. A task that floods both pipes and then sleeps must
//! not hang the drains on the way to the deadline.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

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
        "verirun-timeout-{label}-{}-{unique}",
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

fn run_spec(
    label: &str,
    spec: &str,
) -> (
    PathBuf,
    Cas,
    verirun_store::scheduler::RunOutcome,
    std::time::Duration,
) {
    let root = fresh_root(label);
    let cas_root = root.join("cas");
    let work_root = root.join("work");
    let value = parse(spec.as_bytes()).expect("spec parses");
    let dag = validate_run_spec(&value).expect("spec validates");
    let cas = Cas::open(&cas_root).expect("store opens");
    let started = Instant::now();
    let outcome = scheduler::run(&cas, &Sha256Factory, &work_root, &dag, &BTreeMap::new(), 2)
        .expect("run completes");
    (root, cas, outcome, started.elapsed())
}

#[test]
fn fault_timeout_kills_and_reaps() {
    let spec = r#"{"tasks":[
        {"task_id":"sleeper","executable":"sh","args":["-c","sleep 30"],"env":{},"inputs":[],"outputs":[],"depends_on":[],"timeout_ms":250},
        {"task_id":"quick","executable":"sh","args":["-c","printf done > outputs/done.txt"],"env":{},"inputs":[],"outputs":["done.txt"],"depends_on":[]}
    ]}"#;
    let (root, _cas, outcome, elapsed) = run_spec("kills", spec);
    // The run finished long before the sleeper's 30 seconds: the
    // child was killed at its deadline, not waited out.
    assert!(
        elapsed.as_secs() < 25,
        "run must not wait out the sleeper: {elapsed:?}"
    );
    assert_eq!(outcome.exit_code, 1);
    let sleeper = &outcome.manifest.tasks[1];
    assert_eq!(sleeper.task_id, "sleeper");
    assert_eq!(sleeper.status, TaskStatus::TimedOut);
    assert_eq!(sleeper.exit_code, None);
    let quick = &outcome.manifest.tasks[0];
    assert_eq!(quick.task_id, "quick");
    assert_eq!(quick.status, TaskStatus::Succeeded);
    remove_tree(&root);
}

#[test]
fn fault_timeout_under_pipe_flood() {
    // Flood both pipes, then sleep past the deadline. The kill
    // must not hang the drains, and the partial capture must have
    // landed as a prefix of the full stream.
    let script = "i=0; while [ $i -lt 100 ]; do yes a | head -c 1024; yes b | head -c 1024 >&2; i=$((i+1)); done; sleep 30";
    let spec = format!(
        r#"{{"tasks":[{{"task_id":"flood","executable":"sh","args":["-c","{script}"],"env":{{}},"inputs":[],"outputs":[],"depends_on":[],"timeout_ms":1500}}]}}"#
    );
    let (root, cas, outcome, elapsed) = run_spec("flood", &spec);
    assert!(
        elapsed.as_secs() < 25,
        "run must not wait out the sleeper: {elapsed:?}"
    );
    assert_eq!(outcome.exit_code, 1);
    let record = &outcome.manifest.tasks[0];
    assert_eq!(record.status, TaskStatus::TimedOut);
    assert_eq!(record.exit_code, None);
    let full: Vec<u8> = "a\n".repeat(51_200).into_bytes();
    let stdout = cas
        .read_to_vec(&record.stdout_digest)
        .expect("partial stdout blob reads");
    assert!(
        !stdout.is_empty(),
        "the drains made progress before the kill"
    );
    assert!(
        full.starts_with(&stdout),
        "the captured stdout is a prefix of the flooded stream"
    );
    remove_tree(&root);
}
