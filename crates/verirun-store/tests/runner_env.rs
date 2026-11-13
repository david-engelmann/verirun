//! Hermetic environment fault oracles (INV-RUN-1, INV-RUN-2).
//!
//! The assertions must observe a runner process whose own
//! environment is hostile, and this repository forbids unsafe
//! code in tests, so the tests cannot set variables in-process.
//! Instead each test re-executes the test binary with the hostile
//! environment applied to the child process (a safe API) and a
//! marker variable; the inner invocation runs the real assertion
//! against a task that dumps its environment.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use verirun_core::dag::validate_run_spec;
use verirun_core::digest::TaskStatus;
use verirun_core::parse::parse;
use verirun_store::cas_fs::Cas;
use verirun_store::scheduler::{self, RunOutcome};
use verirun_store::sha256::Sha256Factory;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_root(label: &str) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "verirun-env-{label}-{}-{unique}",
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

/// Runs a single task that dumps its environment and returns the
/// dumped pairs.
fn dump_environment(label: &str) -> BTreeMap<String, String> {
    let root = fresh_root(label);
    let cas_root = root.join("cas");
    let work_root = root.join("work");
    let spec = r#"{"tasks":[{"task_id":"envtask","executable":"env","args":[],"env":{"DECLARED_ONE":"1","ZZ_LAST":"z"},"inputs":[],"outputs":[],"depends_on":[]}]}"#;
    let value = parse(spec.as_bytes()).expect("spec parses");
    let dag = validate_run_spec(&value).expect("spec validates");
    let cas = Cas::open(&cas_root).expect("store opens");
    let outcome: RunOutcome =
        scheduler::run(&cas, &Sha256Factory, &work_root, &dag, &BTreeMap::new(), 1)
            .expect("run completes");
    let record = &outcome.manifest.tasks[0];
    assert_eq!(record.status, TaskStatus::Succeeded);
    let stdout = cas
        .read_to_vec(&record.stdout_digest)
        .expect("stdout blob reads");
    remove_tree(&root);
    let text = String::from_utf8(stdout).expect("env output is UTF-8");
    let mut seen = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line.split_once('=').expect("env line holds a pair");
        seen.insert(key.to_string(), value.to_string());
    }
    seen
}

fn rerun_inner(test_name: &str, hostile: &[(&str, &str)]) -> bool {
    let exe = std::env::current_exe().expect("test binary path");
    let mut command = std::process::Command::new(exe);
    command
        .args(["--exact", test_name, "--nocapture"])
        .env("VR_RUNNER_ENV_INNER", "1");
    for (key, value) in hostile {
        command.env(key, value);
    }
    command.status().expect("inner run spawns").success()
}

#[test]
fn fault_sentinel_variable_absent() {
    if std::env::var_os("VR_RUNNER_ENV_INNER").is_none() {
        assert!(rerun_inner(
            "fault_sentinel_variable_absent",
            &[("VR_SENTINEL_XYZ", "leak-me-not")]
        ));
        return;
    }
    // Inner invocation: this process carries the sentinel in its
    // own environment. The child must observe none of it.
    assert_eq!(
        std::env::var("VR_SENTINEL_XYZ"),
        Ok("leak-me-not".to_string())
    );
    let seen = dump_environment("sentinel");
    assert!(
        !seen.contains_key("VR_SENTINEL_XYZ"),
        "host sentinel leaked into the child: {seen:?}"
    );
    assert!(!seen.contains_key("PATH"), "PATH leaked: {seen:?}");
    assert!(!seen.contains_key("HOME"), "HOME leaked: {seen:?}");
    assert_eq!(seen.get("DECLARED_ONE").map(String::as_str), Some("1"));
    assert_eq!(seen.get("ZZ_LAST").map(String::as_str), Some("z"));
    assert_eq!(seen.get("TZ").map(String::as_str), Some("UTC"));
    assert_eq!(seen.get("LC_ALL").map(String::as_str), Some("C"));
    assert_eq!(seen.get("SOURCE_DATE_EPOCH").map(String::as_str), Some("0"));
    let tmpdir = seen.get("TMPDIR").expect("TMPDIR is set by the runner");
    assert!(
        tmpdir.ends_with("tasks/envtask/tmp"),
        "TMPDIR names the task's private sandbox directory: {tmpdir}"
    );
    // Exactly the declared pairs, the three fixed defaults, and
    // TMPDIR: nothing else exists in the child's environment.
    assert_eq!(seen.len(), 6, "exact environment: {seen:?}");
}

#[test]
fn fault_fixed_defaults_under_hostile_host_env() {
    if std::env::var_os("VR_RUNNER_ENV_INNER").is_none() {
        assert!(rerun_inner(
            "fault_fixed_defaults_under_hostile_host_env",
            &[
                ("TZ", "Pacific/Kiritimati"),
                ("LC_ALL", "xx_YY.invalid"),
                ("SOURCE_DATE_EPOCH", "1899999999"),
            ]
        ));
        return;
    }
    // Inner invocation: the runner's own process breathes the
    // hostile values. Fixed means fixed: the child still observes
    // the defaults.
    assert_eq!(std::env::var("TZ"), Ok("Pacific/Kiritimati".to_string()));
    let seen = dump_environment("hostile");
    assert_eq!(seen.get("TZ").map(String::as_str), Some("UTC"));
    assert_eq!(seen.get("LC_ALL").map(String::as_str), Some("C"));
    assert_eq!(seen.get("SOURCE_DATE_EPOCH").map(String::as_str), Some("0"));
}
