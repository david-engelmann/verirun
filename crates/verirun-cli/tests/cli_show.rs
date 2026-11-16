//! Show command oracles (INV-CLI-5; docs/ARCHITECTURE.md
//! sections 9.3 and 12.1).
//!
//! `verirun show` renders a manifest only after the full
//! verification of section 9.3 has passed over it. These
//! tests drive the compiled binary: a sound manifest renders
//! in text and JSON with the same content, a tampered store
//! renders nothing and exits 2, and a malformed digest exits
//! 3 like every argument error.

#![forbid(unsafe_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
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

/// Runs a one-task spec that writes `content` to `out.txt`
/// and returns (manifest digest, cas dir).
fn run_ok_spec(dir: &Path, content: &str) -> (String, PathBuf) {
    let cas = dir.join("cas");
    let spec_path = dir.join("run.json");
    let body = format!(
        "{{\"tasks\":[{{\"task_id\":\"only\",\"executable\":\"sh\",\
         \"args\":[\"-c\",\"printf {content} > outputs/out.txt\"],\
         \"env\":{{}},\"inputs\":[],\"outputs\":[\"out.txt\"],\"depends_on\":[]}}]}}"
    );
    fs::write(&spec_path, body).expect("write spec");
    let output = invoke(&[
        "run",
        spec_path.to_str().expect("utf8 path"),
        "--cas",
        cas.to_str().expect("utf8 path"),
        "--work",
        dir.join("work").to_str().expect("utf8 path"),
    ]);
    assert_eq!(code(&output), 0, "setup run failed: {}", stderr(&output));
    (stdout(&output).trim().to_string(), cas)
}

fn object_path(cas: &Path, digest_hex: &str) -> PathBuf {
    cas.join("objects").join(&digest_hex[..2]).join(digest_hex)
}

#[test]
fn fault_show_renders_verified_manifest() {
    let dir = fresh_dir("show-ok");
    let (digest, cas) = run_ok_spec(&dir, "hello");
    let cas_text = cas.to_str().expect("utf8 path");

    let text = invoke(&["show", &digest, "--cas", cas_text]);
    assert_eq!(code(&text), 0, "show failed: {}", stderr(&text));
    let out = stdout(&text);
    assert!(out.contains(&format!("manifest {digest}")), "header: {out}");
    assert!(
        out.contains("task only: succeeded, exit code 0"),
        "task line: {out}"
    );
    assert!(out.contains("artifact out.txt"), "artifact line: {out}");

    let json = invoke(&["show", &digest, "--cas", cas_text, "--json"]);
    assert_eq!(code(&json), 0, "show --json failed: {}", stderr(&json));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&json)).expect("show --json parses");
    assert_eq!(parsed["manifest"], digest);
    assert_eq!(parsed["tasks"][0]["task_id"], "only");
    assert_eq!(parsed["tasks"][0]["status"], "succeeded");
    assert_eq!(parsed["tasks"][0]["exit_code"], 0);
    assert_eq!(parsed["tasks"][0]["artifacts"][0]["path"], "out.txt");
}

#[test]
fn fault_show_verifies_before_display() {
    let dir = fresh_dir("show-tampered");
    let (digest, cas) = run_ok_spec(&dir, "hello");
    let cas_text = cas.to_str().expect("utf8 path");

    // Corrupt the artifact blob the manifest names. Check 4
    // of section 9.3 re-hashes it, so show must fail with
    // the integrity code and render nothing at all.
    let shown = invoke(&["show", &digest, "--cas", cas_text]);
    let artifact_line = stdout(&shown)
        .lines()
        .find(|line| line.contains("artifact out.txt"))
        .expect("artifact line present before tampering")
        .to_string();
    let artifact_digest = artifact_line
        .split_whitespace()
        .nth(2)
        .expect("artifact digest field")
        .to_string();
    let path = object_path(&cas, &artifact_digest);
    let mut bytes = fs::read(&path).expect("read object");
    bytes[0] ^= 0xff;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("make writable");
    fs::write(&path, &bytes).expect("write corrupted object");

    let output = invoke(&["show", &digest, "--cas", cas_text]);
    assert_eq!(code(&output), 2, "tampered show: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "",
        "an unverified manifest is never rendered"
    );
    assert!(
        stderr(&output).contains("error:"),
        "the failure is explained on stderr: {}",
        stderr(&output)
    );
}

#[test]
fn fault_show_argument_and_absence_codes() {
    let dir = fresh_dir("show-codes");
    let (_digest, cas) = run_ok_spec(&dir, "hello");
    let cas_text = cas.to_str().expect("utf8 path");

    let malformed = invoke(&["show", "not-a-digest", "--cas", cas_text]);
    assert_eq!(code(&malformed), 3, "malformed digest exits 3");

    let absent = invoke(&["show", &"0".repeat(64), "--cas", cas_text]);
    assert_eq!(code(&absent), 2, "an absent manifest is an integrity failure");
    assert_eq!(stdout(&absent), "", "nothing renders for an absent manifest");
}
