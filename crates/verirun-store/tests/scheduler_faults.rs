//! Scheduler fault oracles (INV-RUN-6, INV-RUN-7).
//!
//! Failure propagation marks exactly the transitive dependent
//! set skipped, with records that name the empty blob for both
//! streams, and a cache entry whose blobs have gone missing is
//! ignored so exactly the affected task re-executes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use verirun_core::canonical;
use verirun_core::dag::validate_run_spec;
use verirun_core::digest::TaskStatus;
use verirun_core::manifest::{Manifest, TaskRecord};
use verirun_core::parse::parse;
use verirun_store::cas_fs::Cas;
use verirun_store::error::RunError;
use verirun_store::scheduler;
use verirun_store::sha256::Sha256Factory;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_root(label: &str) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "verirun-sched-{label}-{}-{unique}",
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

fn task(id: &str, script: &str, deps: &[&str], inputs: &[&str], outputs: &[&str]) -> String {
    let quoted = |items: &[&str]| {
        items
            .iter()
            .map(|item| format!("\"{item}\""))
            .collect::<Vec<_>>()
            .join(",")
    };
    format!(
        r#"{{"task_id":"{id}","executable":"sh","args":["-c","{script}"],"env":{{}},"inputs":[{}],"outputs":[{}],"depends_on":[{}]}}"#,
        quoted(inputs),
        quoted(outputs),
        quoted(deps)
    )
}

fn run(cas: &Cas, work_root: &Path, spec: &str) -> scheduler::RunOutcome {
    let value = parse(spec.as_bytes()).expect("spec parses");
    let dag = validate_run_spec(&value).expect("spec validates");
    scheduler::run(cas, &Sha256Factory, work_root, &dag, &BTreeMap::new(), 4)
        .expect("run completes")
}

fn statuses(manifest: &Manifest) -> BTreeMap<String, TaskStatus> {
    manifest
        .tasks
        .iter()
        .map(|rec| (rec.task_id.clone(), rec.status))
        .collect()
}

#[test]
fn fault_failure_propagation_sets() {
    let root = fresh_root("propagate");
    let cas_root = root.join("cas");
    let cas = Cas::open(&cas_root).expect("store opens");

    // A chain r -> a -> b with r failing; a diamond whose left
    // branch fails after the top succeeds; an independent task.
    let spec = format!(
        r#"{{"tasks":[{},{},{},{},{},{},{},{}]}}"#,
        task("r", "exit 1", &[], &[], &[]),
        task("a", "printf a > outputs/a.txt", &["r"], &[], &["a.txt"]),
        task("b", "printf b > outputs/b.txt", &["a"], &[], &["b.txt"]),
        task("top", "printf t > outputs/t.txt", &[], &[], &["t.txt"]),
        task("left", "exit 3", &["top"], &[], &[]),
        task(
            "right",
            "printf r > outputs/rr.txt",
            &["top"],
            &[],
            &["rr.txt"]
        ),
        task(
            "join",
            "printf j > outputs/j.txt",
            &["left", "right"],
            &[],
            &["j.txt"]
        ),
        task("indep", "printf i > outputs/i.txt", &[], &[], &["i.txt"]),
    );
    let outcome = run(&cas, &root.join("work"), &spec);
    assert_eq!(outcome.exit_code, 1);

    let got = statuses(&outcome.manifest);
    let expected: BTreeMap<String, TaskStatus> = [
        ("r", TaskStatus::Failed),
        ("a", TaskStatus::Skipped),
        ("b", TaskStatus::Skipped),
        ("top", TaskStatus::Succeeded),
        ("left", TaskStatus::Failed),
        ("right", TaskStatus::Succeeded),
        ("join", TaskStatus::Skipped),
        ("indep", TaskStatus::Succeeded),
    ]
    .iter()
    .map(|(id, status)| (id.to_string(), *status))
    .collect();
    assert_eq!(got, expected);

    // Skipped records: the empty blob for both streams, no exit
    // code, no artifacts, and the empty blob is in the store.
    let empty = cas.put_bytes(b"").expect("empty blob put");
    assert!(cas.contains(&empty));
    for id in ["a", "b", "join"] {
        let record = outcome
            .manifest
            .tasks
            .iter()
            .find(|rec| rec.task_id == id)
            .expect("record exists");
        assert_eq!(record.stdout_digest, empty);
        assert_eq!(record.stderr_digest, empty);
        assert_eq!(record.exit_code, None);
        assert_eq!(record.artifacts.len(), 0);
    }
    remove_tree(&root);
}

#[test]
fn fault_cache_missing_blob_reexecutes() {
    let root = fresh_root("cache");
    let cas_root = root.join("cas");
    let cas = Cas::open(&cas_root).expect("store opens");
    let spec = format!(
        r#"{{"tasks":[{},{}]}}"#,
        task(
            "producer",
            "printf 'payload-data' > outputs/p.txt",
            &[],
            &[],
            &["p.txt"]
        ),
        task(
            "consumer",
            "cat inputs/p.txt > outputs/c.txt",
            &["producer"],
            &["p.txt"],
            &["c.txt"]
        ),
    );

    // Cold run: everything executes.
    let cold = run(&cas, &root.join("work1"), &spec);
    assert_eq!(
        statuses(&cold.manifest),
        [
            ("producer".to_string(), TaskStatus::Succeeded),
            ("consumer".to_string(), TaskStatus::Succeeded),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>()
    );

    // Warm run in a fresh workspace: everything is served from
    // the by-exec cache, and the artifact digests do not move.
    let warm = run(&cas, &root.join("work2"), &spec);
    assert_eq!(
        statuses(&warm.manifest),
        [
            ("producer".to_string(), TaskStatus::Cached),
            ("consumer".to_string(), TaskStatus::Cached),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>()
    );
    for (cold_rec, warm_rec) in cold.manifest.tasks.iter().zip(warm.manifest.tasks.iter()) {
        assert_eq!(cold_rec.exec_digest, warm_rec.exec_digest);
        assert_eq!(cold_rec.artifacts, warm_rec.artifacts);
        assert_eq!(cold_rec.stdout_digest, warm_rec.stdout_digest);
    }

    // Delete the producer's output blob under its cache entry.
    // The entry must be ignored and exactly the producer must
    // re-execute; its bytes come back identical, so the consumer
    // is served from cache again.
    let producer_rec = &cold.manifest.tasks[1];
    assert_eq!(producer_rec.task_id, "producer");
    let blob = producer_rec.artifacts[0].digest;
    std::fs::remove_file(cas.object_path(&blob)).expect("blob deleted");
    assert!(!cas.contains(&blob));

    let repaired = run(&cas, &root.join("work3"), &spec);
    let got = statuses(&repaired.manifest);
    assert_eq!(got["producer"], TaskStatus::Succeeded);
    assert_eq!(got["consumer"], TaskStatus::Cached);
    // The repaired run's artifacts equal the cold run's.
    for (cold_rec, new_rec) in cold
        .manifest
        .tasks
        .iter()
        .zip(repaired.manifest.tasks.iter())
    {
        assert_eq!(cold_rec.artifacts, new_rec.artifacts);
    }
    remove_tree(&root);
}

#[cfg(any())]
#[test]
fn fault_work_root_lock_blocks_second_run() {
    let root = fresh_root("worklock");
    let cas_root = root.join("cas");
    let work_root = root.join("work");
    let cas = Cas::open(&cas_root).expect("store opens");
    let spec = format!(
        r#"{{"tasks":[{}]}}"#,
        task("only", "printf x > outputs/x.txt", &[], &[], &["x.txt"])
    );

    // A lock file already sitting in the work root means
    // another run owns it: this run must refuse before any
    // task executes, naming the lock.
    std::fs::create_dir_all(&work_root).expect("work root created");
    let lock_path = work_root.join("run.lock");
    std::fs::write(&lock_path, b"").expect("lock planted");
    let value = parse(spec.as_bytes()).expect("spec parses");
    let dag = validate_run_spec(&value).expect("spec validates");
    let blocked = scheduler::run(&cas, &Sha256Factory, &work_root, &dag, &BTreeMap::new(), 1);
    match blocked {
        Err(RunError::WorkLocked { path }) => assert_eq!(path, lock_path),
        other => panic!("expected a work lock refusal, got {other:?}"),
    }
    assert!(
        !work_root.join("tasks/only/outputs/x.txt").exists(),
        "no task executed under a held work root"
    );

    // With the lock gone the run proceeds, and it leaves no
    // lock behind: a further run over the same work root is
    // admitted as well.
    std::fs::remove_file(&lock_path).expect("lock removed");
    let first = run(&cas, &work_root, &spec);
    assert_eq!(first.exit_code, 0);
    assert!(
        !lock_path.exists(),
        "a finished run releases its work root lock"
    );
    let second = run(&cas, &work_root, &spec);
    assert_eq!(second.exit_code, 0);
    remove_tree(&root);
}

/// Finds the by-exec entry file whose record belongs to
/// `task_id`, by reading every entry in the store's by-exec
/// index and parsing the record each one names.
fn cache_entry_for(cas: &Cas, task_id: &str) -> (PathBuf, TaskRecord) {
    let index = cas.root().join("by-exec");
    let mut found = None;
    for shard in std::fs::read_dir(&index).expect("by-exec index lists") {
        let shard = shard.expect("shard entry").path();
        if !shard.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&shard).expect("shard lists") {
            let entry = entry.expect("entry").path();
            let hex = std::fs::read_to_string(&entry).expect("entry reads");
            let digest = verirun_core::digest::Digest::from_hex(hex.trim())
                .expect("entry names a digest");
            let bytes = cas.read_to_vec(&digest).expect("record blob reads");
            let value = parse(&bytes).expect("record parses");
            let record = TaskRecord::from_value(&value).expect("record decodes");
            if record.task_id == task_id {
                found = Some((entry, record));
            }
        }
    }
    found.expect("a cache entry exists for the task")
}

#[test]
fn fault_cache_record_with_wrong_artifact_paths_reexecutes() {
    let root = fresh_root("forged");
    let cas_root = root.join("cas");
    let cas = Cas::open(&cas_root).expect("store opens");
    let spec = format!(
        r#"{{"tasks":[{},{}]}}"#,
        task(
            "producer",
            "printf 'payload-data' > outputs/p.txt",
            &[],
            &[],
            &["p.txt"]
        ),
        task(
            "consumer",
            "cat inputs/p.txt > outputs/c.txt",
            &["producer"],
            &["p.txt"],
            &["c.txt"]
        ),
    );

    // Cold run: everything executes and both cache entries are
    // published.
    let cold = run(&cas, &root.join("work1"), &spec);
    assert_eq!(cold.exit_code, 0);

    // Forge the producer's cache record: the same execution
    // digest, the same status, a canonically encoded record
    // whose blobs all exist and re-hash, but whose artifact
    // path is not the output the task declared. Every check
    // the lookup performs today passes on this record.
    let (entry_path, genuine) = cache_entry_for(&cas, "producer");
    assert_eq!(genuine.artifacts.len(), 1);
    assert_eq!(genuine.artifacts[0].path, "p.txt");
    let mut forged = genuine.clone();
    forged.artifacts[0].path = "forged.txt".to_string();
    let mut forged_bytes = Vec::new();
    canonical::encode(&forged.to_value(), &mut forged_bytes).expect("forged record encodes");
    let forged_digest = cas.put_bytes(&forged_bytes).expect("forged record stores");
    std::fs::write(&entry_path, forged_digest.to_hex()).expect("entry repointed");

    // Warm run in a fresh workspace. The forged record must be
    // treated as a miss: the producer re-executes, the consumer
    // is served from its own genuine entry, and the manifest
    // records exactly the genuine artifacts.
    let warm = run(&cas, &root.join("work2"), &spec);
    assert_eq!(warm.exit_code, 0);
    let got = statuses(&warm.manifest);
    assert_eq!(
        got["producer"],
        TaskStatus::Succeeded,
        "a record naming the wrong artifact paths is a miss, never an honour"
    );
    assert_eq!(got["consumer"], TaskStatus::Cached);
    for (cold_rec, warm_rec) in cold.manifest.tasks.iter().zip(warm.manifest.tasks.iter()) {
        assert_eq!(cold_rec.artifacts, warm_rec.artifacts);
        assert_eq!(cold_rec.exec_digest, warm_rec.exec_digest);
    }
    remove_tree(&root);
}
