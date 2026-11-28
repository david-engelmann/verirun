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

fn remove_tree(root: &Path) {
    fn clear(path: &Path) {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() && !entry_path.is_symlink() {
                    clear(&entry_path);
                } else if let Ok(meta) = fs::metadata(&entry_path) {
                    let mut perms = meta.permissions();
                    #[allow(
                        clippy::permissions_set_readonly_false,
                        reason = "test cleanup must remove read-only fixtures"
                    )]
                    perms.set_readonly(false);
                    let _ = fs::set_permissions(&entry_path, perms);
                }
            }
        }
    }
    clear(root);
    let _ = fs::remove_dir_all(root);
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

    remove_tree(&dir);
    remove_tree(&dir2);
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
    remove_tree(&dir);
}

#[test]
fn fault_cli_integer_out_of_range_exits_3() {
    // INV-CAN-3 at the CLI level: a run file carrying an
    // integer one past the safe bound is rejected at parse,
    // and the exit is 3, never 2.
    let dir = fresh_dir("int-range");
    let cas = dir.join("cas");
    let cas_arg = cas.to_str().expect("utf8 path").to_string();
    let spec_path = write_spec(
        &dir,
        "{\"tasks\":[{\"task_id\":\"t\",\"executable\":\"sh\",\"args\":[\"-c\",\"true\"],\"env\":{},\"inputs\":[],\"outputs\":[],\"depends_on\":[],\"timeout_ms\":9007199254740992}]}",
    );
    let output = invoke(&[
        "run",
        spec_path.to_str().expect("utf8 path"),
        "--cas",
        &cas_arg,
        "--work",
        dir.join("work").to_str().expect("utf8 path"),
    ]);
    assert_eq!(
        code(&output),
        3,
        "out of range integer: {}",
        stderr(&output)
    );
    remove_tree(&dir);
}

#[test]
fn fault_duplicate_env_key_in_run_file() {
    // INV-CAN-7 at the CLI level: an env object that repeats a
    // key is rejected at parse, and the exit is 3.
    let dir = fresh_dir("dup-env");
    let cas = dir.join("cas");
    let cas_arg = cas.to_str().expect("utf8 path").to_string();
    let spec_path = write_spec(
        &dir,
        "{\"tasks\":[{\"task_id\":\"t\",\"executable\":\"sh\",\"args\":[\"-c\",\"true\"],\"env\":{\"A\":\"1\",\"A\":\"2\"},\"inputs\":[],\"outputs\":[],\"depends_on\":[]}]}",
    );
    let output = invoke(&[
        "run",
        spec_path.to_str().expect("utf8 path"),
        "--cas",
        &cas_arg,
        "--work",
        dir.join("work").to_str().expect("utf8 path"),
    ]);
    assert_eq!(code(&output), 3, "duplicate env key: {}", stderr(&output));
    remove_tree(&dir);
}

#[test]
fn fault_case_colliding_outputs_exit_3() {
    // INV-DAG-7 at the CLI level: two declared outputs that
    // differ only in letter case are refused at validation, the
    // exit is 3, and the error names both paths.
    let dir = fresh_dir("case-collide");
    let cas = dir.join("cas");
    let cas_arg = cas.to_str().expect("utf8 path").to_string();
    let spec_path = write_spec(
        &dir,
        "{\"tasks\":[{\"task_id\":\"t\",\"executable\":\"sh\",\"args\":[\"-c\",\"true\"],\"env\":{},\"inputs\":[],\"outputs\":[\"A.txt\",\"a.txt\"],\"depends_on\":[]}]}",
    );
    let output = invoke(&[
        "run",
        spec_path.to_str().expect("utf8 path"),
        "--cas",
        &cas_arg,
        "--work",
        dir.join("work").to_str().expect("utf8 path"),
    ]);
    assert_eq!(code(&output), 3, "case collision: {}", stderr(&output));
    let err = stderr(&output);
    assert!(err.contains("\"A.txt\""), "names the first path: {err}");
    assert!(err.contains("\"a.txt\""), "names the second path: {err}");
    remove_tree(&dir);
}

#[test]
fn fault_trailing_colliding_outputs_exit_3() {
    // INV-DAG-8 at the CLI level: two declared outputs that
    // differ only in a trailing space are refused at
    // validation, the exit is 3, and the error names both
    // paths.
    let dir = fresh_dir("trailing-collide");
    let cas = dir.join("cas");
    let cas_arg = cas.to_str().expect("utf8 path").to_string();
    let spec_path = write_spec(
        &dir,
        "{\"tasks\":[{\"task_id\":\"t\",\"executable\":\"sh\",\"args\":[\"-c\",\"true\"],\"env\":{},\"inputs\":[],\"outputs\":[\"out.txt \",\"out.txt\"],\"depends_on\":[]}]}",
    );
    let output = invoke(&[
        "run",
        spec_path.to_str().expect("utf8 path"),
        "--cas",
        &cas_arg,
        "--work",
        dir.join("work").to_str().expect("utf8 path"),
    ]);
    assert_eq!(code(&output), 3, "trailing collision: {}", stderr(&output));
    let err = stderr(&output);
    assert!(err.contains("\"out.txt \""), "names the first path: {err}");
    assert!(err.contains("\"out.txt\""), "names the second path: {err}");
    remove_tree(&dir);
}

#[test]
fn fault_case_colliding_task_ids_exit_3() {
    // INV-DAG-9 at the CLI level: two task ids that differ
    // only in letter case are refused at validation, the exit
    // is 3, and the error names both ids.
    let dir = fresh_dir("id-case-collide");
    let cas = dir.join("cas");
    let cas_arg = cas.to_str().expect("utf8 path").to_string();
    let spec_path = write_spec(
        &dir,
        "{\"tasks\":[{\"task_id\":\"Build\",\"executable\":\"sh\",\"args\":[\"-c\",\"true\"],\"env\":{},\"inputs\":[],\"outputs\":[],\"depends_on\":[]},{\"task_id\":\"build\",\"executable\":\"sh\",\"args\":[\"-c\",\"true\"],\"env\":{},\"inputs\":[],\"outputs\":[],\"depends_on\":[]}]}",
    );
    let output = invoke(&[
        "run",
        spec_path.to_str().expect("utf8 path"),
        "--cas",
        &cas_arg,
        "--work",
        dir.join("work").to_str().expect("utf8 path"),
    ]);
    assert_eq!(code(&output), 3, "task id collision: {}", stderr(&output));
    let err = stderr(&output);
    assert!(err.contains("\"Build\""), "names the first id: {err}");
    assert!(err.contains("\"build\""), "names the second id: {err}");
    remove_tree(&dir);
}

#[test]
fn fault_trailing_colliding_task_ids_exit_3() {
    // INV-DAG-10 at the CLI level: two task ids that differ
    // only in trailing spaces or dots are refused at
    // validation, the exit is 3, and the error names both
    // ids.
    let dir = fresh_dir("id-trailing-collide");
    let cas = dir.join("cas");
    let cas_arg = cas.to_str().expect("utf8 path").to_string();
    let spec_path = write_spec(
        &dir,
        "{\"tasks\":[{\"task_id\":\"build\",\"executable\":\"sh\",\"args\":[\"-c\",\"true\"],\"env\":{},\"inputs\":[],\"outputs\":[],\"depends_on\":[]},{\"task_id\":\"build \",\"executable\":\"sh\",\"args\":[\"-c\",\"true\"],\"env\":{},\"inputs\":[],\"outputs\":[],\"depends_on\":[]}]}",
    );
    let output = invoke(&[
        "run",
        spec_path.to_str().expect("utf8 path"),
        "--cas",
        &cas_arg,
        "--work",
        dir.join("work").to_str().expect("utf8 path"),
    ]);
    assert_eq!(code(&output), 3, "task id collision: {}", stderr(&output));
    let err = stderr(&output);
    assert!(err.contains("\"build\""), "names the first id: {err}");
    assert!(err.contains("\"build \""), "names the second id: {err}");
    remove_tree(&dir);
}

#[test]
fn fault_declared_lookup_env_keys_exit_3() {
    // A declared PATH or HOME is refused at validation with
    // exit code 3 naming the key, because a declared PATH
    // steers the child's own command lookups and a declared
    // HOME its file lookups.
    let dir = fresh_dir("env-lookup-keys");
    let cas = dir.join("cas");
    let cas_arg = cas.to_str().expect("utf8 path").to_string();
    let work_arg = dir.join("work").to_str().expect("utf8 path").to_string();
    for (key, value) in [("PATH", "/opt/tools/bin"), ("HOME", "/home/fake")] {
        let body = format!(
            "{{\"tasks\":[{{\"task_id\":\"a\",\"executable\":\"sh\",\"args\":[\"-c\",\"true\"],\"env\":{{\"{key}\":\"{value}\"}},\"inputs\":[],\"outputs\":[],\"depends_on\":[]}}]}}"
        );
        let spec_path = write_spec(&dir, &body);
        let output = invoke(&[
            "run",
            spec_path.to_str().expect("utf8 path"),
            "--cas",
            &cas_arg,
            "--work",
            &work_arg,
        ]);
        assert_eq!(code(&output), 3, "declared {key}: {}", stderr(&output));
        let err = stderr(&output);
        assert!(
            err.contains(&format!("\"{key}\"")),
            "names the refused key {key}: {err}"
        );
    }
    remove_tree(&dir);
}

#[test]
fn fault_json_parity_on_argument_and_seed_errors() {
    // INV-CLI-2: malformed digest arguments and invalid SIM_SEED
    // values must emit structured `error_json` on stdout when
    // `--json` is passed, alongside the stderr message.
    let dir = fresh_dir("json-arg-parity");
    let cas_arg = dir.join("cas").to_str().expect("utf8 path").to_string();
    let work_arg = dir.join("work").to_str().expect("utf8 path").to_string();
    let spec_path = write_spec(&dir, &spec(&[task("only", "true", &[], &[], None)]));
    let spec_arg = spec_path.to_str().expect("utf8 path").to_string();
    let valid_hex = "a".repeat(64);

    let arg_cases: Vec<Vec<&str>> = vec![
        vec!["verify", "bad-digest", "--cas", &cas_arg, "--json"],
        vec!["show", "bad-digest", "--cas", &cas_arg, "--json"],
        vec![
            "diff",
            "bad-digest",
            &valid_hex,
            "--cas",
            &cas_arg,
            "--json",
        ],
        vec![
            "diff",
            &valid_hex,
            "bad-digest",
            "--cas",
            &cas_arg,
            "--json",
        ],
        vec!["pin", "bad-digest", "--cas", &cas_arg, "--json"],
        vec!["unpin", "bad-digest", "--cas", &cas_arg, "--json"],
        vec![
            "run",
            &spec_arg,
            "--cas",
            &cas_arg,
            "--work",
            &work_arg,
            "--input",
            "missing-equals",
            "--json",
        ],
    ];
    for args in arg_cases {
        let output = invoke(&args);
        assert_eq!(code(&output), 3, "case {args:?}: {}", stderr(&output));
        assert!(
            stderr(&output).contains("error:"),
            "stderr carries human error for {args:?}: {}",
            stderr(&output)
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&stdout(&output)).unwrap_or_else(|e| {
                panic!(
                    "stdout for {args:?} must be JSON ({e}): {:?}",
                    stdout(&output)
                )
            });
        assert_eq!(parsed["exit_code"], 3);
        assert!(
            parsed["error"].as_str().is_some_and(|s| !s.is_empty()),
            "error text present in JSON for {args:?}: {parsed}"
        );
    }

    let seed_out = Command::new(env!("CARGO_BIN_EXE_verirun"))
        .args(["gc", "--cas", &cas_arg, "--dry-run", "--json"])
        .env("SIM_SEED", "not-an-integer")
        .output()
        .expect("spawn verirun with invalid SIM_SEED");
    assert_eq!(code(&seed_out), 3);
    assert!(stderr(&seed_out).contains("SIM_SEED"));
    let seed_json: serde_json::Value =
        serde_json::from_str(&stdout(&seed_out)).expect("SIM_SEED --json error parses");
    assert_eq!(seed_json["exit_code"], 3);
    assert!(
        seed_json["error"]
            .as_str()
            .is_some_and(|s| s.contains("SIM_SEED"))
    );

    remove_tree(&dir);
}
