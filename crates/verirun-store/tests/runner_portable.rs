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

/// The full size case from the roadmap: eight mebibytes on
/// each stream in one task, both streams in flight together,
/// captured byte exactly (workstream W2).
const VOLUME_BYTES: usize = 8 * 1024 * 1024;

/// One stream's volume content: a short unit tiled and cut to
/// exactly `total` bytes, the same construction the child
/// programs are asked to produce.
fn volume_pattern(unit: &[u8], total: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(total);
    while out.len() < total {
        let take = (total - out.len()).min(unit.len());
        out.extend_from_slice(&unit[..take]);
    }
    out
}

#[test]
fn streams_at_volume_are_byte_exact() {
    let (executable, args) = volume_invocation();
    let declared = interpreter_env();
    let pairs: Vec<(&str, &str)> = declared
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let (root, cas, execution, _elapsed) = run_task("volume", executable, args, &pairs, None);
    assert_eq!(execution.exit_code, Some(0));
    let stdout = cas
        .read_to_vec(&execution.stdout_digest)
        .expect("stdout blob reads");
    let stderr = cas
        .read_to_vec(&execution.stderr_digest)
        .expect("stderr blob reads");
    assert_eq!(stdout, volume_pattern(volume_stdout_unit(), VOLUME_BYTES));
    assert_eq!(stderr, volume_pattern(volume_stderr_unit(), VOLUME_BYTES));
    remove_tree(&root);
}

/// The repeated unit the volume child leaves on stdout.
#[cfg(unix)]
const fn volume_stdout_unit() -> &'static [u8] {
    b"verirun!\n"
}

/// The repeated unit the volume child leaves on stdout.
#[cfg(windows)]
const fn volume_stdout_unit() -> &'static [u8] {
    b"verirun!\n"
}

/// The repeated unit the volume child leaves on stderr.
#[cfg(unix)]
const fn volume_stderr_unit() -> &'static [u8] {
    b"Verirun?\n"
}

/// The repeated unit the volume child leaves on stderr.
#[cfg(windows)]
const fn volume_stderr_unit() -> &'static [u8] {
    b"Verirun?\n"
}

/// The invocation that writes the volume patterns: two
/// concurrent pipelines on Unix, byte writes through the
/// standard streams on Windows.
#[cfg(unix)]
fn volume_invocation() -> (String, Vec<String>) {
    shell_invocation(
        "yes 'verirun!' | head -c 8388608 & yes 'Verirun?' | head -c 8388608 >&2 & wait",
    )
}

/// The invocation that writes the volume patterns.
#[cfg(windows)]
fn volume_invocation() -> (String, Vec<String>) {
    // The script carries no double quotes, so nothing in it
    // needs escaping through the runner's command line
    // quoting. The newline is built from its character code.
    let script = "$o=[Console]::OpenStandardOutput(); $e=[Console]::OpenStandardError(); \
                  $so=[Text.Encoding]::ASCII.GetBytes(('verirun!' + [string][char]10) * 932068); \
                  $se=[Text.Encoding]::ASCII.GetBytes(('Verirun?' + [string][char]10) * 932068); \
                  $o.Write($so, 0, 8388608); $o.Flush(); $e.Write($se, 0, 8388608); $e.Flush()";
    (
        "powershell".to_string(),
        vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            script.to_string(),
        ],
    )
}

#[test]
fn timeout_capture_is_an_exact_prefix() {
    // The child writes a known prefix, flushes it, and then
    // sleeps holding both pipes. The deadline kills the tree,
    // the pipes close, and the captured stream must be exactly
    // the prefix: nothing lost that was delivered, nothing
    // invented past it (workstream W2).
    let (executable, args) = prefix_invocation();
    let declared = interpreter_env();
    let pairs: Vec<(&str, &str)> = declared
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    // Three seconds, not the tree case's five hundred
    // milliseconds: the Windows child's interpreter needs a
    // moment to start before it can write its prefix, and the
    // deadline must land after the write, inside the sleep.
    let (root, cas, execution, elapsed) = run_task("prefix", executable, args, &pairs, Some(3000));
    assert!(execution.timed_out, "the deadline fired: {execution:?}");
    let stdout = cas
        .read_to_vec(&execution.stdout_digest)
        .expect("stdout blob reads");
    assert_eq!(stdout, expected_prefix());
    let stderr = cas
        .read_to_vec(&execution.stderr_digest)
        .expect("stderr blob reads");
    assert!(stderr.is_empty(), "stderr stayed empty: {stderr:?}");
    assert!(
        elapsed < Duration::from_secs(8),
        "execution returned well inside the drain grace: {elapsed:?}"
    );
    remove_tree(&root);
}

/// The invocation that writes the prefix and then sleeps for
/// thirty seconds holding the pipes.
#[cfg(unix)]
fn prefix_invocation() -> (String, Vec<String>) {
    shell_invocation(
        "i=0; while [ $i -lt 32 ]; do printf 'verirun-prefix-line\\n'; i=$((i+1)); done; exec sleep 30",
    )
}

/// The declared environment for the interpreter children.
///
/// A PowerShell child cannot start under the runner's
/// hermetic environment on its own: the interpreter loads
/// its managed runtime from the system root and exits with
/// 0xFFFF0000 when SystemRoot is not in its environment
/// (proven by a hosted probe that varied one variable at a
/// time). Declaring it is the spec's business, like any
/// other pair a child needs, so the interpreter cases here
/// declare the value this process itself runs under.
#[cfg(windows)]
fn interpreter_env() -> Vec<(String, String)> {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    vec![("SystemRoot".to_string(), root)]
}

/// The declared environment for the interpreter children.
#[cfg(unix)]
const fn interpreter_env() -> Vec<(String, String)> {
    Vec::new()
}

/// The invocation that writes the prefix and then sleeps for
/// thirty seconds holding the pipes. A `cmd` for loop's
/// variable does not survive the runner's command line, so
/// the prefix is written through the standard stream from
/// PowerShell, again with no double quotes in the script.
#[cfg(windows)]
fn prefix_invocation() -> (String, Vec<String>) {
    let script = "$o=[Console]::OpenStandardOutput(); \
                  $b=[Text.Encoding]::ASCII.GetBytes(('verirun-prefix-line' + [string][char]13 + [string][char]10) * 32); \
                  $o.Write($b, 0, $b.Length); $o.Flush(); Start-Sleep -Seconds 30";
    (
        "powershell".to_string(),
        vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            script.to_string(),
        ],
    )
}

/// The prefix bytes the child delivers before it sleeps.
#[cfg(unix)]
fn expected_prefix() -> Vec<u8> {
    volume_pattern(b"verirun-prefix-line\n", 32 * 20)
}

/// The prefix bytes the child delivers before it sleeps.
#[cfg(windows)]
fn expected_prefix() -> Vec<u8> {
    volume_pattern(b"verirun-prefix-line\r\n", 32 * 21)
}

/// The cutoff itself, on the one platform whose children can
/// escape the tree kill: a detached grandchild keeps writing
/// after the deadline kills the child, the drain waits out
/// the grace, and the finalized stream is an exact prefix of
/// the endless stream, with the cut noted (workstream W2).
/// Windows children cannot break out of their job object, so
/// there the grace parity rests on the shared collection
/// code and the prefix case above.
#[cfg(unix)]
#[test]
fn cut_stream_is_an_exact_prefix() {
    let (executable, args) = shell_invocation(
        "setsid sh -c 'i=0; while [ $i -lt 300 ]; do printf x; i=$((i+1)); sleep 0.1; done' & sleep 30",
    );
    let (root, cas, execution, elapsed) = run_task("cut", executable, args, &[], Some(500));
    assert!(execution.timed_out, "the deadline fired: {execution:?}");
    assert!(
        execution.failure_detail.is_some(),
        "the stream was cut after the grace: {execution:?}"
    );
    let stdout = cas
        .read_to_vec(&execution.stdout_digest)
        .expect("stdout blob reads");
    assert!(
        !stdout.is_empty() && stdout.iter().all(|byte| *byte == b'x'),
        "the cut stream is an exact prefix of the endless stream: {} bytes",
        stdout.len()
    );
    assert!(
        elapsed >= Duration::from_secs(9) && elapsed < Duration::from_secs(25),
        "the grace bounded the drain: {elapsed:?}"
    );
    remove_tree(&root);
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

/// Executes one task with declared outputs and returns its
/// execution record and the store (workstream W3).
fn run_collect_task(
    label: &str,
    executable: String,
    args: Vec<String>,
    declared_env: &[(&str, &str)],
    outputs: Vec<String>,
) -> (PathBuf, Cas, TaskExecution) {
    let root = fresh_root(label);
    let cas = Cas::open(root.join("cas")).expect("store opens");
    let runner = Runner::new(&cas);
    let task = TaskSpec {
        task_id: "collect".to_string(),
        executable,
        args,
        env: declared_env
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect(),
        inputs: Vec::new(),
        outputs,
        depends_on: Vec::new(),
        timeout_ms: None,
    };
    let execution = runner
        .execute(&task, &[], &root.join("sandbox"))
        .expect("execution completes");
    (root, cas, execution)
}

#[test]
fn nested_outputs_are_collected() {
    // Collection walks declared outputs through held handles
    // (workstream W3); a nested output and a top level output
    // both land with their exact bytes.
    let (executable, args) = collect_invocation();
    let declared = interpreter_env();
    let pairs: Vec<(&str, &str)> = declared
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let outputs = vec!["sub/inner.txt".to_string(), "top.txt".to_string()];
    let (root, cas, execution) = run_collect_task("nested", executable, args, &pairs, outputs);
    assert!(
        execution.failure_detail.is_none(),
        "collection succeeded: {execution:?}"
    );
    assert_eq!(execution.artifacts.len(), 2);
    let inner = &execution.artifacts[0];
    assert_eq!(inner.path, "sub/inner.txt");
    assert_eq!(inner.bytes, 11);
    assert_eq!(
        cas.read_to_vec(&inner.digest).expect("inner blob reads"),
        b"inner-bytes"
    );
    let top = &execution.artifacts[1];
    assert_eq!(top.path, "top.txt");
    assert_eq!(top.bytes, 9);
    assert_eq!(
        cas.read_to_vec(&top.digest).expect("top blob reads"),
        b"top-bytes"
    );
    remove_tree(&root);
}

#[test]
fn linked_output_component_is_rejected() {
    // A declared output that passes through a link component
    // is a collection failure on both platforms, and nothing
    // is collected (workstream W3, INV-SBX-3).
    let (executable, args) = linked_invocation();
    let declared = interpreter_env();
    let pairs: Vec<(&str, &str)> = declared
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let outputs = vec!["linkdir/inner.txt".to_string()];
    let (root, _cas, execution) = run_collect_task("linked", executable, args, &pairs, outputs);
    let detail = execution
        .failure_detail
        .expect("collection through a link fails the task");
    assert!(
        detail.contains("passes through a symbolic link"),
        "the failure names the link: {detail}"
    );
    assert!(execution.artifacts.is_empty(), "nothing was collected");
    remove_tree(&root);
}

/// The invocation that writes one nested and one top level
/// output with known bytes.
#[cfg(unix)]
fn collect_invocation() -> (String, Vec<String>) {
    shell_invocation(
        "mkdir -p outputs/sub && printf 'inner-bytes' > outputs/sub/inner.txt && printf 'top-bytes' > outputs/top.txt",
    )
}

/// The invocation that writes one nested and one top level
/// output with known bytes.
#[cfg(windows)]
fn collect_invocation() -> (String, Vec<String>) {
    let script = "New-Item -ItemType Directory -Force outputs/sub | Out-Null; \
                  [IO.File]::WriteAllText('outputs/sub/inner.txt', 'inner-bytes'); \
                  [IO.File]::WriteAllText('outputs/top.txt', 'top-bytes')";
    (
        "powershell".to_string(),
        vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            script.to_string(),
        ],
    )
}

/// The invocation that plants a link as an intermediate
/// component of a declared output.
#[cfg(unix)]
fn linked_invocation() -> (String, Vec<String>) {
    shell_invocation(
        "mkdir -p outputs/decoy && printf 'x' > outputs/decoy/inner.txt && ln -s decoy outputs/linkdir",
    )
}

/// The invocation that plants a link as an intermediate
/// component of a declared output.
#[cfg(windows)]
fn linked_invocation() -> (String, Vec<String>) {
    let script = "New-Item -ItemType Directory -Force outputs/decoy | Out-Null; \
                  [IO.File]::WriteAllText('outputs/decoy/inner.txt', 'x'); \
                  New-Item -ItemType Junction -Path outputs/linkdir -Target outputs/decoy | Out-Null";
    (
        "powershell".to_string(),
        vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            script.to_string(),
        ],
    )
}
