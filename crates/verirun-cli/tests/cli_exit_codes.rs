//! Exit code fault oracles (INV-CLI-1, INV-CLI-2;
//! docs/ARCHITECTURE.md section 12.2).
//!
//! These tests drive the compiled `verirun` binary as a
//! subprocess and assert the process exit code of every
//! outcome class, plus the text and JSON renderings of one
//! integrity failure. The mapping under test is total: 0
//! success or verified or equivalent, 1 task failure or a
//! differing diff, 2 integrity failure, 3 invalid input of
//! every kind, including argument parsing failures, which must
//! never report as 2.

#![forbid(unsafe_code)]

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Makes a stored object writable so a test can corrupt
/// it, in the platform's own terms, mode bits on Unix and
/// the read only attribute on Windows.
fn make_writable(path: &Path) {
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o644)).expect("make writable");
    #[cfg(windows)]
    {
        let mut perms = fs::metadata(path).expect("object exists").permissions();
        #[allow(
            clippy::permissions_set_readonly_false,
            reason = "the corruption fixture must write over the object"
        )]
        perms.set_readonly(false);
        fs::set_permissions(path, perms).expect("make writable");
    }
}
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "verirun-cli-{tag}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn invoke(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_verirun"))
        .args(args)
        .env_remove("SIM_SEED")
        .output()
        .expect("spawn verirun")
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("exit code, not a signal")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is utf8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is utf8")
}

fn write_spec(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join(format!(
        "run-{}.json",
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, body).expect("write spec");
    path
}

fn task(
    task_id: &str,
    script: &str,
    outputs: &[&str],
    depends_on: &[&str],
    timeout_ms: Option<u64>,
) -> String {
    let outputs = outputs
        .iter()
        .map(|path| format!("\"{path}\""))
        .collect::<Vec<_>>()
        .join(",");
    let deps = depends_on
        .iter()
        .map(|dep| format!("\"{dep}\""))
        .collect::<Vec<_>>()
        .join(",");
    let timeout = timeout_ms.map_or(String::new(), |ms| format!(",\"timeout_ms\":{ms}"));
    format!(
        "{{\"task_id\":\"{task_id}\",\"executable\":\"sh\",\"args\":[\"-c\",\"{script}\"],\
         \"env\":{{}},\"inputs\":[],\"outputs\":[{outputs}],\"depends_on\":[{deps}]{timeout}}}"
    )
}

fn spec(tasks: &[String]) -> String {
    format!("{{\"tasks\":[{}]}}", tasks.join(","))
}

/// Runs a one-task spec that writes `content` to `out.txt` in
/// `dir`'s store and returns the manifest digest the binary
/// prints.
fn run_ok_spec(dir: &Path, content: &str) -> String {
    let cas = dir.join("cas");
    let spec_path = write_spec(
        dir,
        &spec(&[task(
            "only",
            &format!("printf {content} > outputs/out.txt"),
            &["out.txt"],
            &[],
            None,
        )]),
    );
    let output = invoke(&[
        "run",
        spec_path.to_str().expect("utf8 path"),
        "--cas",
        cas.to_str().expect("utf8 path"),
        "--work",
        dir.join("work").to_str().expect("utf8 path"),
    ]);
    assert_eq!(code(&output), 0, "setup run failed: {}", stderr(&output));
    stdout(&output).trim().to_string()
}

fn object_path(cas: &Path, digest_hex: &str) -> PathBuf {
    cas.join("objects").join(&digest_hex[..2]).join(digest_hex)
}

/// Corrupts one byte of the object file for `digest_hex` and
/// returns the hex of the digest the corrupted bytes hash to,
/// computed by putting the same bytes through a scratch store.
fn corrupt_object(scratch: &Path, cas: &Path, digest_hex: &str) -> String {
    let path = object_path(cas, digest_hex);
    let mut bytes = fs::read(&path).expect("read object");
    bytes[0] ^= 0xff;
    make_writable(&path);
    fs::write(&path, &bytes).expect("write corrupted object");
    blob_digest(scratch, &bytes)
}

/// The blob digest of known bytes, through a scratch store.
fn blob_digest(scratch: &Path, bytes: &[u8]) -> String {
    let scratch_cas =
        verirun_store::cas_fs::Cas::open(scratch.join("scratch-cas")).expect("scratch cas");
    scratch_cas.put_bytes(bytes).expect("put").to_hex()
}

#[test]
fn fault_exit_code_table() {
    let dir = fresh_dir("table");
    let cas = dir.join("cas");
    let cas_arg = cas.to_str().expect("utf8 path").to_string();

    // Argument parsing failures exit 3, never 2.
    assert_eq!(code(&invoke(&[])), 3, "no arguments");
    assert_eq!(code(&invoke(&["frobnicate"])), 3, "unknown command");
    assert_eq!(
        code(&invoke(&["verify", "not-a-digest", "--cas", &cas_arg])),
        3,
        "malformed digest argument"
    );
    assert_eq!(
        code(&invoke(&[
            "run",
            "/nonexistent/run.json",
            "--cas",
            &cas_arg,
            "--work",
            "w"
        ])),
        3,
        "missing run file"
    );

    // A cyclic graph is invalid input: 3.
    let cyclic = write_spec(
        &dir,
        &spec(&[
            task("a", "true", &[], &["b"], None),
            task("b", "true", &[], &["a"], None),
        ]),
    );
    let output = invoke(&[
        "run",
        cyclic.to_str().expect("utf8 path"),
        "--cas",
        &cas_arg,
        "--work",
        dir.join("wc").to_str().expect("utf8 path"),
    ]);
    assert_eq!(code(&output), 3, "cyclic graph: {}", stderr(&output));

    // A failing task exits 1; a timed out task exits 1.
    let failing = write_spec(&dir, &spec(&[task("bad", "exit 1", &[], &[], None)]));
    let output = invoke(&[
        "run",
        failing.to_str().expect("utf8 path"),
        "--cas",
        &cas_arg,
        "--work",
        dir.join("w1").to_str().expect("utf8 path"),
    ]);
    assert_eq!(code(&output), 1, "failing task: {}", stderr(&output));

    let slow = write_spec(&dir, &spec(&[task("slow", "sleep 5", &[], &[], Some(100))]));
    let output = invoke(&[
        "run",
        slow.to_str().expect("utf8 path"),
        "--cas",
        &cas_arg,
        "--work",
        dir.join("w2").to_str().expect("utf8 path"),
    ]);
    assert_eq!(code(&output), 1, "timed out task: {}", stderr(&output));

    // A clean run exits 0 and prints the manifest digest.
    let manifest = run_ok_spec(&dir, "hello");
    assert_eq!(manifest.len(), 64, "manifest digest is 64 hex");

    // Verify: the clean manifest exits 0.
    let output = invoke(&["verify", &manifest, "--cas", &cas_arg]);
    assert_eq!(code(&output), 0, "clean verify: {}", stderr(&output));

    // Verify of a well-formed but absent digest is an integrity
    // failure: 2.
    let absent = "0".repeat(64);
    let output = invoke(&["verify", &absent, "--cas", &cas_arg]);
    assert_eq!(code(&output), 2, "absent manifest: {}", stderr(&output));

    // Verify with a corrupted manifest blob exits 2.
    let dir2 = fresh_dir("table-tamper");
    let manifest2 = run_ok_spec(&dir2, "world");
    corrupt_object(&dir2, &dir2.join("cas"), &manifest2);
    let output = invoke(&[
        "verify",
        &manifest2,
        "--cas",
        dir2.join("cas").to_str().expect("utf8 path"),
    ]);
    assert_eq!(code(&output), 2, "tampered manifest: {}", stderr(&output));

    // Diff: identical runs exit 0, differing runs exit 1.
    let output = invoke(&["diff", &manifest, &manifest, "--cas", &cas_arg]);
    assert_eq!(code(&output), 0, "identical diff: {}", stderr(&output));
    let other = run_ok_spec(&dir, "different");
    let output = invoke(&["diff", &manifest, &other, "--cas", &cas_arg]);
    assert_eq!(code(&output), 1, "differing diff: {}", stderr(&output));

    // GC on the store exits 0, in dry run and for real.
    let output = invoke(&["gc", "--cas", &cas_arg, "--dry-run"]);
    assert_eq!(code(&output), 0, "gc dry run: {}", stderr(&output));
    let output = invoke(&["gc", "--cas", &cas_arg]);
    assert_eq!(code(&output), 0, "gc: {}", stderr(&output));

    fs::remove_dir_all(&dir).ok();
    fs::remove_dir_all(&dir2).ok();
}

#[test]
fn fault_json_parity_on_failure() {
    let dir = fresh_dir("parity");
    let cas = dir.join("cas");
    let cas_arg = cas.to_str().expect("utf8 path").to_string();
    let manifest = run_ok_spec(&dir, "parity-content");

    // Corrupt the artifact blob (content "parity-content") and
    // learn both digests the failure must name.
    let expected = blob_digest(&dir, b"parity-content");
    let actual = corrupt_object(&dir, &cas, &expected);

    let text = invoke(&["verify", &manifest, "--cas", &cas_arg]);
    assert_eq!(code(&text), 2);
    let text_err = stderr(&text);
    assert!(
        text_err.contains(&expected),
        "text names the expected digest: {text_err}"
    );
    assert!(
        text_err.contains(&actual),
        "text names the actual digest: {text_err}"
    );

    let json_run = invoke(&["verify", &manifest, "--cas", &cas_arg, "--json"]);
    assert_eq!(code(&json_run), 2);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&json_run)).expect("stdout is one JSON object");
    assert_eq!(
        parsed["expected"], expected,
        "JSON names the expected digest"
    );
    assert_eq!(parsed["actual"], actual, "JSON names the actual digest");
    assert_eq!(parsed["task_id"], "only");
    assert_eq!(parsed["path"], "out.txt");
    // The JSON form is never the only explanation: the human
    // readable failure also appears on stderr.
    assert!(
        stderr(&json_run).contains(&expected),
        "stderr still explains the failure under --json"
    );
    fs::remove_dir_all(&dir).ok();
}
