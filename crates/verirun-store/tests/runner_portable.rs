//! Portable runner oracles (perfection programme, workstream
//! W1).
//!
//! The runner's contract, proven with the same test text on
//! Unix and on Windows: exit codes propagate, the environment
//! is hermetic, both streams are captured byte exactly, and a
//! deadline terminates the whole process tree rather than only
//! the direct child. The child programs differ by platform
//! (a shell here, `cmd` there); the assertions do not.
//!
//! Before workstream W1 these tests did not compile on Windows,
//! because the runner module was fenced to Unix. That compile
//! failure was the slice's red state; the backend is what turns
//! them green.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use verirun_core::dag::TaskSpec;
use verirun_store::cas_fs::Cas;
use verirun_store::runner::{Runner, TaskExecution};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_root(label: &str) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "verirun-portable-{label}-{}-{unique}",
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

/// Executes one task and returns its execution record and the
/// store, so tests can read the captured streams back.
fn run_task(
    label: &str,
    executable: String,
    args: Vec<String>,
    declared_env: &[(&str, &str)],
    timeout_ms: Option<u64>,
) -> (PathBuf, Cas, TaskExecution, Duration) {
    let root = fresh_root(label);
    let cas = Cas::open(root.join("cas")).expect("store opens");
    let runner = Runner::new(&cas);
    let task = TaskSpec {
        task_id: "portable".to_string(),
        executable,
        args,
        env: declared_env
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        depends_on: Vec::new(),
        timeout_ms,
    };
    let started = Instant::now();
    let execution = runner
        .execute(&task, &[], &root.join("sandbox"))
        .expect("execution completes");
    (root, cas, execution, started.elapsed())
}

/// The platform's shell invocation for a one-line script.
#[cfg(unix)]
fn shell_invocation(script: &str) -> (String, Vec<String>) {
    ("sh".to_string(), vec!["-c".to_string(), script.to_string()])
}

/// The platform's shell invocation for a one-line script.
#[cfg(windows)]
fn shell_invocation(script: &str) -> (String, Vec<String>) {
    (
        "cmd".to_string(),
        vec!["/C".to_string(), script.to_string()],
    )
}

#[test]
fn exit_code_is_propagated() {
    let (executable, args) = shell_invocation(exit_script());
    let (root, _cas, execution, _elapsed) = run_task("exit", executable, args, &[], None);
    assert_eq!(execution.exit_code, Some(7));
    assert!(!execution.timed_out);
    remove_tree(&root);
}

/// The script that exits with code 7 on each platform.
#[cfg(unix)]
const fn exit_script() -> &'static str {
    "exit 7"
}

/// The script that exits with code 7 on each platform.
#[cfg(windows)]
const fn exit_script() -> &'static str {
    "exit 7"
}

#[test]
fn environment_is_hermetic() {
    let (executable, args) = env_invocation();
    let (root, cas, execution, _elapsed) =
        run_task("env", executable, args, &[("DECLARED_ONE", "1")], None);
    assert_eq!(execution.exit_code, Some(0));
    let stdout = cas
        .read_to_vec(&execution.stdout_digest)
        .expect("stdout blob reads");
    let text = String::from_utf8(stdout).expect("environment dump is UTF-8");
    let mut seen = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if let Some((key, value)) = line.split_once('=') {
            seen.insert(key.to_string(), value.to_string());
        }
    }
    #[cfg(windows)]
    {
        // `cmd` synthesizes these three for itself when they
        // are absent, the same shell bookkeeping that makes
        // the Unix side run `env` directly rather than a
        // shell. They are not the runner's doing, so they
        // leave the map before the exact comparison below.
        for key in ["COMSPEC", "PATHEXT", "PROMPT"] {
            seen.remove(key);
        }
    }
    assert_eq!(seen.get("LC_ALL").map(String::as_str), Some("C"));
    assert_eq!(seen.get("SOURCE_DATE_EPOCH").map(String::as_str), Some("0"));
    assert_eq!(seen.get("TZ").map(String::as_str), Some("UTC"));
    assert_eq!(seen.get("DECLARED_ONE").map(String::as_str), Some("1"));
    assert!(seen.contains_key("TMPDIR"), "the sandbox TMPDIR is set");
    assert_eq!(
        seen.len(),
        5,
        "nothing but the fixed pairs, the declared pair, and TMPDIR is present: {seen:?}"
    );
    remove_tree(&root);
}

/// The invocation that prints the process environment: the
/// `env` program itself on Unix (a shell would add its own
/// variables), and `cmd`'s builtin `set` on Windows.
#[cfg(unix)]
fn env_invocation() -> (String, Vec<String>) {
    ("env".to_string(), Vec::new())
}

/// The invocation that prints the process environment.
#[cfg(windows)]
fn env_invocation() -> (String, Vec<String>) {
    ("cmd".to_string(), vec!["/C".to_string(), "set".to_string()])
}

#[test]
fn streams_are_captured_byte_exactly() {
    let (executable, args) = shell_invocation(stream_script());
    let (root, cas, execution, _elapsed) = run_task("streams", executable, args, &[], None);
    assert_eq!(execution.exit_code, Some(0));
    let stdout = cas
        .read_to_vec(&execution.stdout_digest)
        .expect("stdout blob reads");
    let stderr = cas
        .read_to_vec(&execution.stderr_digest)
        .expect("stderr blob reads");
    assert_eq!(stdout, expected_stdout());
    assert_eq!(stderr, expected_stderr());
    remove_tree(&root);
}

/// The script that writes a known line to each stream.
#[cfg(unix)]
const fn stream_script() -> &'static str {
    "printf 'hello out'; printf 'hello err' >&2"
}

/// The script that writes a known line to each stream.
#[cfg(windows)]
const fn stream_script() -> &'static str {
    "echo hello out & echo hello err>&2"
}

/// What the stream script leaves on stdout, per platform.
#[cfg(unix)]
fn expected_stdout() -> Vec<u8> {
    b"hello out".to_vec()
}

/// What the stream script leaves on stdout, per platform.
#[cfg(windows)]
fn expected_stdout() -> Vec<u8> {
    b"hello out \r\n".to_vec()
}

/// What the stream script leaves on stderr, per platform.
#[cfg(unix)]
fn expected_stderr() -> Vec<u8> {
    b"hello err".to_vec()
}

/// What the stream script leaves on stderr, per platform.
#[cfg(windows)]
fn expected_stderr() -> Vec<u8> {
    b"hello err\r\n".to_vec()
}

#[test]
fn timeout_kills_the_whole_tree() {
    // The script leaves a grandchild holding the stdout pipe
    // while the child sleeps. If the deadline killed only the
    // child, the drain would wait out the ten second grace and
    // cut the stream; the elapsed bound and the missing cut
    // note together prove the whole tree died at the deadline.
    let (executable, args) = shell_invocation(tree_script());
    let (root, _cas, execution, elapsed) = run_task("tree", executable, args, &[], Some(500));
    assert!(execution.timed_out, "the deadline fired: {execution:?}");
    assert_eq!(execution.exit_code, None);
    assert!(
        execution.failure_detail.is_none(),
        "no stream was cut, so the pipes closed with the tree: {execution:?}"
    );
    assert!(
        elapsed < Duration::from_secs(8),
        "execution returned well inside the drain grace: {elapsed:?}"
    );
    remove_tree(&root);
}

/// The script that sleeps for thirty seconds with a grandchild
/// holding the stdout pipe open.
#[cfg(unix)]
const fn tree_script() -> &'static str {
    "sleep 30 & wait"
}

/// The script that sleeps for thirty seconds with a grandchild
/// holding the stdout pipe open. The started `ping` inherits
/// the pipe; the foreground `ping` is redirected away so only
/// the grandchild's membership in the job is under test.
#[cfg(windows)]
const fn tree_script() -> &'static str {
    "start /B ping -n 30 127.0.0.1 & ping -n 30 127.0.0.1 > NUL"
}
