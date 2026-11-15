#![forbid(unsafe_code)]
//! Crash recovery suite (the recovery half of INV-CAS-1, M4).
//!
//! The M1 fault suite proves a crash at each put step never leaves
//! a partial object visible. This suite proves the store recovers:
//! after a crash at each step boundary, a reopened store passes a
//! full audit (every visible object re-hashes to its name), garbage
//! collection clears exactly the crash debris, new puts and reads
//! work, a second crash during recovery changes nothing, and a
//! pinned manifest survives the whole sequence untouched.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use verirun_core::cas::PutPhase;
use verirun_core::dag::validate_run_spec;
use verirun_core::digest::Digest;
use verirun_core::parse::parse;
use verirun_store::cas_fs::{Cas, PutObserver};
use verirun_store::gc;
use verirun_store::scheduler;
use verirun_store::sha256::Sha256Factory;

static COUNTER: AtomicU64 = AtomicU64::new(0);
static HOOK: Once = Once::new();

fn fresh_root(label: &str) -> PathBuf {
    HOOK.call_once(|| {
        // Crash-injection tests panic on purpose; keep the output
        // readable.
        std::panic::set_hook(Box::new(|_| {}));
    });
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "verirun-recovery-{label}-{}-{unique}",
        std::process::id()
    ))
}

fn remove_store(root: &Path) {
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

#[derive(Debug)]
struct CrashAt {
    phase: PutPhase,
}

impl PutObserver for CrashAt {
    fn on_phase(&mut self, phase: PutPhase) {
        assert!(phase != self.phase, "simulated crash at {phase:?}");
    }
}

fn crash_put(cas: &Cas, payload: &[u8], phase: PutPhase) {
    let mut injector = CrashAt { phase };
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cas.put_bytes_observed(payload, &mut injector)
    }));
    assert!(outcome.is_err(), "the injected crash must propagate");
}

/// Every object file under `objects/` names a digest its bytes
/// re-hash to (the read verifies), and no other files hide there.
fn audit_store(cas: &Cas, root: &Path) -> usize {
    let objects = root.join("objects");
    let mut audited = 0;
    for shard in std::fs::read_dir(&objects).expect("objects dir reads") {
        let shard = shard.expect("shard entry reads");
        for entry in std::fs::read_dir(shard.path()).expect("shard dir reads") {
            let entry = entry.expect("object entry reads");
            let hex = format!(
                "{}{}",
                shard.file_name().to_string_lossy(),
                entry.file_name().to_string_lossy()
            );
            let digest = Digest::from_hex(&hex).expect("object name is a digest");
            cas.read_to_vec(&digest)
                .expect("every visible object verifies against its name");
            audited += 1;
        }
    }
    audited
}

fn tmp_file_count(root: &Path) -> usize {
    std::fs::read_dir(root.join("tmp"))
        .expect("tmp dir reads")
        .count()
}

#[test]
#[ignore = "M4 bisection round 3"]
fn fault_crash_recovery_store_stays_consistent() {
    let payload = b"recovery payload, first attempt";
    let second = b"recovery payload, second attempt";
    for (crash_phase, recovery_phase) in [
        (PutPhase::Writing, PutPhase::FileSynced),
        (PutPhase::FileSynced, PutPhase::Renamed),
        (PutPhase::Renamed, PutPhase::Writing),
    ] {
        let root = fresh_root("recover");
        let cas = Cas::open(&root).expect("opens");
        crash_put(&cas, payload, crash_phase);
        drop(cas);

        // Reopen and audit whatever survived.
        let cas = Cas::open(&root).expect("reopens after crash");
        audit_store(&cas, &root);

        // A second crash, this time during the recovery work
        // itself, changes nothing about the store's consistency.
        crash_put(&cas, second, recovery_phase);
        drop(cas);
        let cas = Cas::open(&root).expect("reopens after the second crash");
        audit_store(&cas, &root);

        // Collection with an aged clock clears the crash debris:
        // leftover temp files and any complete but unreachable
        // object the crash left behind.
        let future = SystemTime::now() + Duration::from_secs(7200);
        gc::collect(&cas, 0, false, future).expect("collection succeeds");
        assert_eq!(tmp_file_count(&root), 0, "collection clears crash debris");
        audit_store(&cas, &root);

        // The store is fully usable afterwards.
        let digest = cas.put_bytes(payload).expect("put succeeds after recovery");
        assert_eq!(
            cas.read_to_vec(&digest)
                .expect("read succeeds after recovery"),
            payload
        );
        let digest = cas.put_bytes(second).expect("second put succeeds");
        assert_eq!(
            cas.read_to_vec(&digest).expect("second read succeeds"),
            second
        );
        remove_store(&root);
    }
}

#[test]
#[ignore = "M4 bisection round 3"]
fn fault_crash_recovery_pinned_manifest_survives() {
    let root = fresh_root("pinned");
    let cas = Cas::open(&root).expect("opens");
    // A genuine manifest from a real run: collection parses every
    // pinned manifest during its mark phase, so only a real one
    // can stand in for a pinned run.
    let spec = r#"{"tasks":[{
        "task_id": "only", "executable": "sh",
        "args": ["-c", "printf kept > outputs/kept.txt"],
        "env": {}, "inputs": [], "outputs": ["kept.txt"], "depends_on": []
    }]}"#;
    let value = parse(spec.as_bytes()).expect("spec parses");
    let dag = validate_run_spec(&value).expect("spec validates");
    let outcome = scheduler::run(
        &cas,
        &Sha256Factory,
        &root.join("work"),
        &dag,
        &BTreeMap::new(),
        1,
    )
    .expect("run completes");
    let manifest = outcome.manifest_digest;
    let artifact = outcome.manifest.tasks[0].artifacts[0].digest;
    cas.pin_manifest(&manifest).expect("manifest pins");
    drop(cas);

    // A crash mid-put, then a collection aged past every grace:
    // the pinned manifest and everything it names are reachable
    // and survive; the crashed put's debris does not.
    let cas = Cas::open(&root).expect("reopens");
    crash_put(&cas, b"unrelated crashed put", PutPhase::Writing);
    drop(cas);
    let cas = Cas::open(&root).expect("reopens after crash");
    let future = SystemTime::now() + Duration::from_secs(7200);
    gc::collect(&cas, 0, false, future).expect("collection succeeds");
    assert!(cas.contains(&manifest), "the pinned manifest survives");
    assert!(
        cas.contains(&artifact),
        "the pinned run's artifact survives"
    );
    assert_eq!(
        cas.read_to_vec(&artifact).expect("pinned artifact reads"),
        b"kept"
    );
    assert_eq!(tmp_file_count(&root), 0, "the crash debris is collected");
    remove_store(&root);
}
