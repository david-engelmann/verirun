//! Output collection and input materialization fault oracles
//! (INV-SBX-3, INV-SBX-4, INV-SBX-5).
//!
//! Collection walks the declared path component by component and
//! rejects any symlink component itself, so a linked output's
//! content never enters the store. Collection records exactly the
//! declared outputs, with digest, byte length, and the executable
//! bit, and nothing else. A corrupted input blob aborts the run
//! before the consuming task spawns.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use verirun_core::dag::validate_run_spec;
use verirun_core::digest::{DomainTag, TaskStatus, hash_bytes};
use verirun_core::parse::parse;
use verirun_store::cas_fs::Cas;
use verirun_store::error::{RunError, StoreError};
use verirun_store::scheduler;
use verirun_store::sha256::Sha256Factory;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_root(label: &str) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "verirun-collect-{label}-{}-{unique}",
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

fn task_json(id: &str, script: &str, outputs: &[&str]) -> String {
    let outputs = outputs
        .iter()
        .map(|o| format!("\"{o}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"task_id":"{id}","executable":"sh","args":["-c","{script}"],"env":{{}},"inputs":[],"outputs":[{outputs}],"depends_on":[]}}"#
    )
}

#[test]
fn fault_symlink_output_rejected() {
    let root = fresh_root("symlink");
    let secret = root.join("secret.txt");
    std::fs::create_dir_all(&root).expect("test root");
    std::fs::write(&secret, b"store-must-never-see-this").expect("secret written");
    let cas_root = root.join("cas");
    let work_root = root.join("work");

    // Three tasks, three link shapes: a final component linking
    // out of the sandbox, an intermediate component linking
    // within it, and a dangling final component.
    let final_link = task_json(
        "finallink",
        &format!("ln -s {} outputs/out.txt", secret.display()),
        &["out.txt"],
    );
    let middle_link = task_json(
        "middlelink",
        "mkdir outputs/real; ln -s real outputs/link; printf x > outputs/link/f.txt",
        &["link/f.txt"],
    );
    let dangling = task_json(
        "dangling",
        "ln -s no-such-target outputs/dangling.txt",
        &["dangling.txt"],
    );
    let spec = format!(r#"{{"tasks":[{final_link},{middle_link},{dangling}]}}"#);
    let value = parse(spec.as_bytes()).expect("spec parses");
    let dag = validate_run_spec(&value).expect("spec validates");
    let cas = Cas::open(&cas_root).expect("store opens");
    let outcome = scheduler::run(&cas, &Sha256Factory, &work_root, &dag, &BTreeMap::new(), 3)
        .expect("run completes");
    assert_eq!(outcome.exit_code, 1);
    assert_eq!(outcome.manifest.tasks.len(), 3);
    for record in &outcome.manifest.tasks {
        assert_eq!(
            record.status,
            TaskStatus::Failed,
            "task {} must fail at collection",
            record.task_id
        );
        assert_eq!(
            record.artifacts.len(),
            0,
            "a rejected collection records no artifacts"
        );
    }
    // The linked content never entered the store under any role.
    let secret_digest = hash_bytes(
        &Sha256Factory,
        DomainTag::Blob,
        b"store-must-never-see-this",
    );
    assert!(!cas.contains(&secret_digest));
    remove_tree(&root);
}

#[test]
fn fault_collection_exactness_and_metadata() {
    let root = fresh_root("exact");
    let cas_root = root.join("cas");
    let work_root = root.join("work");
    // Declared out of order on purpose. The writer also leaves an
    // undeclared file in the output area.
    let writer = task_json(
        "writer",
        "printf 'B-content' > outputs/b.txt; printf '#!/bin/sh\\ntrue' > outputs/a.sh; chmod +x outputs/a.sh; printf 'extra' > outputs/extra.txt",
        &["b.txt", "a.sh"],
    );
    let missing = task_json("missing", "true", &["gone.txt"]);
    let dirtask = task_json("dirtask", "mkdir outputs/d", &["d"]);
    let spec = format!(r#"{{"tasks":[{writer},{missing},{dirtask}]}}"#);
    let value = parse(spec.as_bytes()).expect("spec parses");
    let dag = validate_run_spec(&value).expect("spec validates");
    let cas = Cas::open(&cas_root).expect("store opens");
    let outcome = scheduler::run(&cas, &Sha256Factory, &work_root, &dag, &BTreeMap::new(), 3)
        .expect("run completes");
    assert_eq!(outcome.exit_code, 1);
    let by_id: BTreeMap<&str, &verirun_core::manifest::TaskRecord> = outcome
        .manifest
        .tasks
        .iter()
        .map(|rec| (rec.task_id.as_str(), rec))
        .collect();

    let writer_rec = by_id["writer"];
    assert_eq!(writer_rec.status, TaskStatus::Succeeded);
    // Exactly the declared outputs, sorted by path, each with its
    // digest, byte length, and executable flag. The undeclared
    // extra.txt is not among them.
    assert_eq!(writer_rec.artifacts.len(), 2);
    let a = &writer_rec.artifacts[0];
    assert_eq!(a.path, "a.sh");
    assert_eq!(a.bytes, 14);
    assert!(a.executable);
    assert_eq!(
        cas.read_to_vec(&a.digest).expect("a.sh blob"),
        b"#!/bin/sh\ntrue"
    );
    let b = &writer_rec.artifacts[1];
    assert_eq!(b.path, "b.txt");
    assert_eq!(b.bytes, 9);
    assert!(!b.executable);
    assert_eq!(
        cas.read_to_vec(&b.digest).expect("b.txt blob"),
        b"B-content"
    );

    // A promised output that was never produced fails the task,
    // and so does a promised output that is a directory.
    assert_eq!(by_id["missing"].status, TaskStatus::Failed);
    assert_eq!(by_id["missing"].artifacts.len(), 0);
    assert_eq!(by_id["dirtask"].status, TaskStatus::Failed);
    assert_eq!(by_id["dirtask"].artifacts.len(), 0);
    remove_tree(&root);
}

#[test]
fn fault_corrupt_input_aborts_before_spawn() {
    let root = fresh_root("corrupt");
    let cas_root = root.join("cas");
    let work_root = root.join("work");
    let cas = Cas::open(&cas_root).expect("store opens");
    let digest = cas.put_bytes(b"input-bytes").expect("input put");

    // Corrupt the stored object in place: flip one byte of its
    // content. The digest it is addressed by no longer matches.
    let object_path = cas.object_path(&digest);
    let mut perms = std::fs::metadata(&object_path)
        .expect("object metadata")
        .permissions();
    #[allow(
        clippy::permissions_set_readonly_false,
        reason = "the test must rewrite a read-only object to corrupt it"
    )]
    perms.set_readonly(false);
    std::fs::set_permissions(&object_path, perms).expect("object writable");
    let mut bytes = std::fs::read(&object_path).expect("object reads");
    bytes[0] ^= 0xFF;
    std::fs::write(&object_path, &bytes).expect("corruption written");

    let spec = r#"{"tasks":[{"task_id":"consumer","executable":"sh","args":["-c","printf ran > outputs/ran.txt"],"env":{},"inputs":["in.txt"],"outputs":["ran.txt"],"depends_on":[]}]}"#;
    let value = parse(spec.as_bytes()).expect("spec parses");
    let dag = validate_run_spec(&value).expect("spec validates");
    let mut bindings = BTreeMap::new();
    bindings.insert("in.txt".to_string(), digest);
    let outcome = scheduler::run(&cas, &Sha256Factory, &work_root, &dag, &bindings, 1);
    match outcome {
        Err(RunError::Store {
            source: StoreError::Integrity { expected, .. },
        }) => assert_eq!(expected, digest),
        other => panic!("expected an integrity abort naming the digest, got {other:?}"),
    }
    // The task never spawned: nothing was collected or produced.
    assert!(
        !work_root.join("tasks/consumer/outputs/ran.txt").exists(),
        "the consuming task must not run"
    );
    remove_tree(&root);
}
