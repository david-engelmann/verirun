//! Verify access failure oracles (audit findings F18 and F24).
//!
//! A manifest or blob that exists but cannot be read, here by
//! permission, is reported as unreadable with the store failure
//! named, never as missing: absence and access failure send
//! the operator to different remedies. Both forms still exit 2.

#![forbid(unsafe_code)]

use std::fs;
#[cfg(unix)]
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

/// Denies every read of the file, in the platform's own
/// terms, the mode bits removed on Unix and a deny entry
/// in the access control list on Windows.
#[cfg(unix)]
fn make_unreadable(path: &Path) {
    fs::set_permissions(path, fs::Permissions::from_mode(0o000)).expect("deny reads");
}

#[cfg(windows)]
fn make_unreadable(path: &Path) {
    let status = std::process::Command::new("icacls")
        .arg(path)
        .args(["/deny", "Everyone:(R)"])
        .status()
        .expect("icacls runs");
    assert!(status.success(), "icacls denies reads");
}

#[cfg(unix)]
fn restore(path: &Path) {
    fs::set_permissions(path, fs::Permissions::from_mode(0o444)).expect("restore reads");
}

#[cfg(windows)]
fn restore(path: &Path) {
    let status = std::process::Command::new("icacls")
        .arg(path)
        .args(["/remove:d", "Everyone"])
        .status()
        .expect("icacls runs");
    assert!(status.success(), "icacls restores reads");
}

#[test]
fn fault_verify_reports_unreadable_manifest_not_missing() {
    let dir = fresh_dir("verify-unreadable-manifest");
    let (digest, cas) = run_ok_spec(&dir, "hello");
    let cas_text = cas.to_str().expect("utf8 path");
    let path = object_path(&cas, &digest);
    make_unreadable(&path);

    let output = invoke(&["verify", &digest, "--cas", cas_text]);
    assert_eq!(code(&output), 2, "unreadable manifest: {}", stderr(&output));
    let err = stderr(&output);
    assert!(
        err.contains("could not be read"),
        "the failure names access, not absence: {err}"
    );
    assert!(
        !err.contains("no manifest object"),
        "an existing manifest is never reported missing: {err}"
    );

    let json = invoke(&["verify", &digest, "--cas", cas_text, "--json"]);
    assert_eq!(code(&json), 2, "unreadable manifest json");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&json)).expect("error json parses");
    assert_eq!(parsed["kind"], "manifest_unreadable");
    assert_eq!(parsed["digest"], digest);

    restore(&path);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fault_verify_reports_unreadable_blob_not_missing() {
    let dir = fresh_dir("verify-unreadable-blob");
    let (digest, cas) = run_ok_spec(&dir, "hello");
    let cas_text = cas.to_str().expect("utf8 path");

    // Find the artifact blob digest through show, then deny
    // reads on that object only; the manifest stays readable,
    // so checks 1 to 3 pass and check 4 meets the access
    // failure.
    let shown = invoke(&["show", &digest, "--cas", cas_text]);
    let artifact_line = stdout(&shown)
        .lines()
        .find(|line| line.contains("artifact out.txt"))
        .expect("artifact line present")
        .to_string();
    let artifact_digest = artifact_line
        .split_whitespace()
        .nth(2)
        .expect("artifact digest field")
        .to_string();
    let path = object_path(&cas, &artifact_digest);
    make_unreadable(&path);

    let output = invoke(&["verify", &digest, "--cas", cas_text]);
    assert_eq!(code(&output), 2, "unreadable blob: {}", stderr(&output));
    let err = stderr(&output);
    assert!(
        err.contains("could not be read"),
        "the failure names access, not absence: {err}"
    );
    assert!(
        !err.contains("is missing"),
        "an existing blob is never reported missing: {err}"
    );

    let json = invoke(&["verify", &digest, "--cas", cas_text, "--json"]);
    assert_eq!(code(&json), 2, "unreadable blob json");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&json)).expect("error json parses");
    assert_eq!(parsed["kind"], "blob_unreadable");
    assert_eq!(parsed["digest"], artifact_digest);
    assert_eq!(parsed["task_id"], "only");
    assert_eq!(parsed["path"], "out.txt");

    restore(&path);
    let _ = fs::remove_dir_all(&dir);
}
