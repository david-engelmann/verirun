//! Pipe drain stress oracle (INV-RUN-3) and the M4 pipe and
//! timeout stress suite (INV-LAT-2, plus the INV-RUN-4
//! escaped-grandchild corner as documented behaviour).
//!
//! Both streams are filled far past the OS pipe capacity in
//! alternating chunks. A runner that drained the streams
//! sequentially, or waited for exit before draining, would
//! deadlock here: the child blocks on a full pipe while the
//! runner blocks on the other stream or on the child.
//!
//! The M4 stresses run the scheduler on a worker thread under a
//! hard test deadline: exceeding the deadline fails the test
//! instead of hanging the suite, which is what makes a drain
//! deadlock observable as a failure at volume.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

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

/// Runs one spec on a worker thread and fails the test if the run
/// does not finish within `deadline`.
fn run_under_deadline(root: &Path, spec: &str, jobs: usize, deadline: Duration) -> RunOutcome {
    let cas_root = root.join("cas");
    let work_root = root.join("work");
    let spec = spec.to_string();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let value = parse(spec.as_bytes()).expect("spec parses");
        let dag = validate_run_spec(&value).expect("spec validates");
        let cas = Cas::open(&cas_root).expect("store opens");
        let outcome = scheduler::run(
            &cas,
            &Sha256Factory,
            &work_root,
            &dag,
            &BTreeMap::new(),
            jobs,
        )
        .expect("run completes");
        let _ = sender.send(outcome);
    });
    receiver
        .recv_timeout(deadline)
        .expect("the run finishes before the deadline; a deadlock fails here")
}

fn pattern(unit: &[u8], total: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(total);
    while bytes.len() < total {
        bytes.extend_from_slice(unit);
    }
    bytes.truncate(total);
    bytes
}

#[test]
fn stress_unbounded_output_under_deadline() {
    let root = fresh_root("unbounded");
    // Four mebibytes on stdout and three on stderr at once, many
    // pipe buffers over, both streams carrying a repeating pattern
    // whose every byte is checked afterwards.
    let spec = r#"{"tasks":[{
        "task_id": "loud",
        "executable": "sh",
        "args": ["-c", "yes ABCDEFGH | head -c 4194304; yes 12345678 | head -c 3145728 >&2"],
        "env": {}, "inputs": [], "outputs": [], "depends_on": [], "timeout_ms": 120000
    }]}"#;
    let outcome = run_under_deadline(&root, spec, 1, Duration::from_secs(90));
    assert_eq!(outcome.exit_code, 0, "the loud task succeeds");
    let cas = Cas::open(root.join("cas")).expect("store reopens");
    let record = &outcome.manifest.tasks[0];
    let stdout = cas
        .read_to_vec(&record.stdout_digest)
        .expect("stdout blob reads");
    assert_eq!(
        stdout,
        pattern(b"ABCDEFGH\n", 4_194_304),
        "stdout is byte exact"
    );
    let stderr = cas
        .read_to_vec(&record.stderr_digest)
        .expect("stderr blob reads");
    assert_eq!(
        stderr,
        pattern(b"12345678\n", 3_145_728),
        "stderr is byte exact"
    );
    remove_tree(&root);
}

#[test]
fn stress_never_closes_pipes_at_full_size() {
    let root = fresh_root("never-closes");
    // The INV-LAT-2 edge case at its named size: 16 MiB split
    // across both streams from a child that then holds both
    // streams open and never closes them. Only the task
    // timeout can end the run, and the drain must deliver
    // every written byte before the kill, never deadlocking
    // against the full pipes.
    let spec = r#"{"tasks":[{
        "task_id": "holding",
        "executable": "sh",
        "args": ["-c", "yes ABCDEFGH | head -c 8388608; yes 12345678 | head -c 8388608 >&2; sleep 60"],
        "env": {}, "inputs": [], "outputs": [], "depends_on": [], "timeout_ms": 5000
    }]}"#;
    let outcome = run_under_deadline(&root, spec, 1, Duration::from_secs(60));
    assert_eq!(outcome.exit_code, 1, "the run reports the timeout");
    let cas = Cas::open(root.join("cas")).expect("store reopens");
    let record = &outcome.manifest.tasks[0];
    assert_eq!(record.status, TaskStatus::TimedOut);
    let stdout = cas
        .read_to_vec(&record.stdout_digest)
        .expect("stdout blob reads");
    assert_eq!(
        stdout,
        pattern(b"ABCDEFGH\n", 8_388_608),
        "stdout is byte exact at 8 MiB"
    );
    let stderr = cas
        .read_to_vec(&record.stderr_digest)
        .expect("stderr blob reads");
    assert_eq!(
        stderr,
        pattern(b"12345678\n", 8_388_608),
        "stderr is byte exact at 8 MiB"
    );
    remove_tree(&root);
}

#[test]
fn stress_timeout_volume_with_grandchildren() {
    let root = fresh_root("timeouts");
    // Twelve tasks, each holding a background grandchild inside
    // its own process group, all with a 200 millisecond timeout.
    // The group kill takes the grandchildren with the child, so
    // every drain finishes and the whole run completes quickly.
    let mut tasks = String::new();
    for index in 0..12 {
        if index > 0 {
            tasks.push(',');
        }
        write!(
            tasks,
            r#"{{"task_id": "t{index}", "executable": "sh",
                "args": ["-c", "sleep 30 & sleep 30"],
                "env": {{}}, "inputs": [], "outputs": [], "depends_on": [],
                "timeout_ms": 200}}"#
        )
        .expect("writing to a String cannot fail");
    }
    let spec = format!("{{\"tasks\":[{tasks}]}}");
    let outcome = run_under_deadline(&root, &spec, 4, Duration::from_secs(60));
    assert_eq!(outcome.exit_code, 1, "timed out tasks fail the run");
    assert_eq!(outcome.manifest.tasks.len(), 12);
    for record in &outcome.manifest.tasks {
        assert_eq!(
            record.status,
            TaskStatus::TimedOut,
            "task {} timed out",
            record.task_id
        );
    }
    remove_tree(&root);
}

#[test]
fn stress_escaped_grandchild_output_collected() {
    let root = fresh_root("escaped");
    // The documented INV-RUN-4 corner: a grandchild escapes into
    // its own session with setsid, keeps the stdout pipe, and
    // writes after the child has exited cleanly. The run waits for
    // the pipe to close and collects the escaped bytes in full.
    let spec = r#"{"tasks":[{
        "task_id": "escape",
        "executable": "sh",
        "args": ["-c", "setsid sh -c 'sleep 2; printf escaped-tail' & exit 0"],
        "env": {}, "inputs": [], "outputs": [], "depends_on": [], "timeout_ms": 60000
    }]}"#;
    let outcome = run_under_deadline(&root, spec, 1, Duration::from_secs(30));
    assert_eq!(outcome.exit_code, 0, "the escaped run still succeeds");
    let record = &outcome.manifest.tasks[0];
    assert_eq!(record.status, TaskStatus::Succeeded);
    let cas = Cas::open(root.join("cas")).expect("store reopens");
    let stdout = cas
        .read_to_vec(&record.stdout_digest)
        .expect("stdout blob reads");
    assert_eq!(
        stdout, b"escaped-tail",
        "the escaped bytes are collected in full"
    );
    remove_tree(&root);
}
