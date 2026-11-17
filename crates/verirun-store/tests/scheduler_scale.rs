//! Scale proof for run planning (audit finding F14).
//!
//! A chain of a thousand tasks, each consuming its predecessor's
//! output, plans and runs end to end, and the content written by
//! the first task arrives intact in the last task's artifact.
//! Planning resolves every input against the producer map and
//! the accumulated dependency sets, so the shape that used to
//! cost a graph scan and a graph walk per input is exercised at
//! the size where that cost compounded.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use verirun_core::dag::validate_run_spec;
use verirun_core::digest::TaskStatus;
use verirun_core::parse::parse;
use verirun_store::cas_fs::Cas;
use verirun_store::scheduler;
use verirun_store::sha256::Sha256Factory;

static COUNTER: AtomicU64 = AtomicU64::new(0);

const CHAIN_LEN: usize = 1_000;

fn fresh_root(label: &str) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "verirun-scale-{label}-{}-{unique}",
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

fn chain_spec() -> String {
    let mut tasks = String::new();
    for i in 0..CHAIN_LEN {
        if i > 0 {
            tasks.push(',');
        }
        let (script, deps, inputs) = if i == 0 {
            (
                "printf seed > outputs/f0.txt".to_string(),
                String::new(),
                String::new(),
            )
        } else {
            (
                format!("cat inputs/f{}.txt > outputs/f{i}.txt", i - 1),
                format!("\"t{}\"", i - 1),
                format!("\"f{}.txt\"", i - 1),
            )
        };
        write!(
            tasks,
            r#"{{"task_id":"t{i}","executable":"sh","args":["-c","{script}"],"env":{{}},"inputs":[{inputs}],"outputs":["f{i}.txt"],"depends_on":[{deps}]}}"#
        )
        .expect("writing to a string cannot fail");
    }
    format!(r#"{{"tasks":[{tasks}]}}"#)
}

#[test]
fn scale_chain_of_a_thousand_tasks_plans_and_runs() {
    let root = fresh_root("chain");
    let cas = Cas::open(root.join("cas")).expect("store opens");
    let work_root = root.join("work");
    let spec = chain_spec();
    let value = parse(spec.as_bytes()).expect("spec parses");
    let dag = validate_run_spec(&value).expect("spec validates");
    let outcome = scheduler::run(&cas, &Sha256Factory, &work_root, &dag, &BTreeMap::new(), 4)
        .expect("run completes");
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.manifest.tasks.len(), CHAIN_LEN);
    assert!(
        outcome
            .manifest
            .tasks
            .iter()
            .all(|rec| rec.status == TaskStatus::Succeeded),
        "every task in the chain succeeded"
    );
    let record = |task_id: &str| {
        outcome
            .manifest
            .tasks
            .iter()
            .find(|rec| rec.task_id == task_id)
            .expect("task has a record")
    };
    let first = record("t0");
    let last = record(&format!("t{}", CHAIN_LEN - 1));
    assert_eq!(first.artifacts.len(), 1);
    assert_eq!(last.artifacts.len(), 1);
    assert_eq!(
        first.artifacts[0].digest, last.artifacts[0].digest,
        "the seed content survives the whole chain"
    );
    let bytes = cas
        .read_to_vec(&last.artifacts[0].digest)
        .expect("final artifact reads back");
    assert_eq!(bytes, b"seed");
    remove_tree(&root);
}
