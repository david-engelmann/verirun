//! Diff semantics fault oracle (INV-CLI-4;
//! docs/ARCHITECTURE.md section 9.4).
//!
//! Drives the compiled binary: equivalent runs exit 0, runs
//! differing in any compared field exit 1 with the differing
//! tasks named in task id order, and a side that fails
//! verification exits 2, never reported as a mere difference.

#![forbid(unsafe_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "verirun-diff-{tag}-{}-{}",
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

fn task(task_id: &str, script: &str, outputs: &[&str]) -> String {
    let outputs = outputs
        .iter()
        .map(|path| format!("\"{path}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"task_id\":\"{task_id}\",\"executable\":\"sh\",\"args\":[\"-c\",\"{script}\"],\
         \"env\":{{}},\"inputs\":[],\"outputs\":[{outputs}],\"depends_on\":[]}}"
    )
}

/// Runs a spec in `dir`'s store and returns the manifest
/// digest.
fn run_spec(dir: &Path, tasks: &[String]) -> String {
    let spec_path = dir.join(format!(
        "spec-{}.json",
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&spec_path, format!("{{\"tasks\":[{}]}}", tasks.join(","))).expect("write spec");
    let output = invoke(&[
        "run",
        spec_path.to_str().expect("utf8 path"),
        "--cas",
        dir.join("cas").to_str().expect("utf8 path"),
        "--work",
        dir.join(format!("work-{}", COUNTER.fetch_add(1, Ordering::Relaxed)))
            .to_str()
            .expect("utf8 path"),
    ]);
    // A run whose tasks fail still produces a manifest; only
    // exit 0 and 1 are acceptable setup outcomes.
    assert!(
        matches!(code(&output), 0 | 1),
        "setup run failed hard: {}",
        stderr(&output)
    );
    stdout(&output).trim().to_string()
}

fn diff(dir: &Path, first: &str, second: &str) -> Output {
    invoke(&[
        "diff",
        first,
        second,
        "--cas",
        dir.join("cas").to_str().expect("utf8 path"),
    ])
}

#[test]
fn fault_diff_semantics_table() {
    let dir = fresh_dir("semantics");
    // Baseline: two tasks, both succeeding.
    let baseline = run_spec(
        &dir,
        &[
            task("alpha", "printf A > outputs/a.txt", &["a.txt"]),
            task("beta", "printf B > outputs/b.txt", &["b.txt"]),
        ],
    );

    // Identical runs are equivalent: exit 0.
    let output = diff(&dir, &baseline, &baseline);
    assert_eq!(code(&output), 0, "identical: {}", stderr(&output));

    // A task present on one side only: exit 1, named.
    let extended = run_spec(
        &dir,
        &[
            task("alpha", "printf A > outputs/a.txt", &["a.txt"]),
            task("beta", "printf B > outputs/b.txt", &["b.txt"]),
            task("gamma", "printf G > outputs/g.txt", &["g.txt"]),
        ],
    );
    let output = diff(&dir, &baseline, &extended);
    assert_eq!(code(&output), 1, "one-sided task: {}", stderr(&output));
    assert!(
        stdout(&output).contains("gamma"),
        "the one-sided task is named: {}",
        stdout(&output)
    );

    // A status change with the same task set: exit 1, named.
    // (beta now fails, so its record also loses its artifacts;
    // the status field alone already forces the difference.)
    let status_changed = run_spec(
        &dir,
        &[
            task("alpha", "printf A > outputs/a.txt", &["a.txt"]),
            task("beta", "exit 1", &["b.txt"]),
        ],
    );
    let output = diff(&dir, &baseline, &status_changed);
    assert_eq!(code(&output), 1, "status change: {}", stderr(&output));
    assert!(
        stdout(&output).contains("beta"),
        "the changed task is named: {}",
        stdout(&output)
    );

    // Artifact content changes on both tasks: both named, in
    // task id order.
    let content_changed = run_spec(
        &dir,
        &[
            task("alpha", "printf A2 > outputs/a.txt", &["a.txt"]),
            task("beta", "printf B2 > outputs/b.txt", &["b.txt"]),
        ],
    );
    let output = diff(&dir, &baseline, &content_changed);
    assert_eq!(code(&output), 1, "content change: {}", stderr(&output));
    let text = stdout(&output);
    let alpha_at = text.find("alpha").expect("alpha named");
    let beta_at = text.find("beta").expect("beta named");
    assert!(
        alpha_at < beta_at,
        "differences are in task id order: {text}"
    );

    // A corrupted side fails verification: exit 2, and the
    // output does not present the run as merely different.
    let manifest_path = dir
        .join("cas")
        .join("objects")
        .join(&baseline[..2])
        .join(&baseline);
    let mut bytes = fs::read(&manifest_path).expect("read manifest object");
    bytes[0] ^= 0xff;
    fs::set_permissions(&manifest_path, fs::Permissions::from_mode(0o644)).expect("make writable");
    fs::write(&manifest_path, &bytes).expect("corrupt manifest object");
    let output = diff(&dir, &baseline, &baseline);
    assert_eq!(code(&output), 2, "corrupted side: {}", stderr(&output));

    fs::remove_dir_all(&dir).ok();
}
