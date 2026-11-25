//! CLI ergonomics oracles for `pin`, `unpin`, `run --pin`, and
//! `run --input` (docs/ARCHITECTURE.md sections 10 and 12).
//!
//! `pin` verifies the manifest before writing its pin file so a
//! missing or corrupted manifest exits 2 and leaves no broken
//! pin behind. `unpin` removes the pin unconditionally so a
//! broken pin can always be cleared. `run --pin` pins the
//! produced manifest atomically after scheduling, and
//! `run --input <REL_PATH>=<HOST_FILE>` binds host files into
//! the store after upfront validation (INV-LAT-4).

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "verirun-pin-in-{tag}-{}-{}",
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
    output.status.code().expect("exit code")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is utf8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is utf8")
}

#[test]
fn pin_and_unpin_govern_garbage_collection() {
    let dir = fresh_dir("pin-gc");
    let cas = dir.join("cas");
    let cas_arg = cas.to_str().expect("utf8");
    let spec_path = dir.join("run.json");
    fs::write(
        &spec_path,
        r#"{"tasks":[{"task_id":"only","executable":"sh","args":["-c","printf keep-me > outputs/out.txt"],"env":{},"inputs":[],"outputs":["out.txt"],"depends_on":[]}]}"#,
    )
    .expect("write spec");

    let run_out = invoke(&[
        "run",
        spec_path.to_str().expect("utf8"),
        "--cas",
        cas_arg,
        "--work",
        dir.join("work").to_str().expect("utf8"),
    ]);
    assert_eq!(code(&run_out), 0, "run failed: {}", stderr(&run_out));
    let manifest = stdout(&run_out).trim().to_string();

    // Pinning an absent manifest exits 2 and writes no pin file.
    let absent = "0".repeat(64);
    let bad_pin = invoke(&["pin", "--cas", cas_arg, &absent, "--json"]);
    assert_eq!(code(&bad_pin), 2);
    assert!(!cas.join("pins").join(format!("{absent}.pin")).exists());
    let bad_json: serde_json::Value =
        serde_json::from_str(&stdout(&bad_pin)).expect("bad pin JSON");
    assert_eq!(bad_json["kind"], "manifest_missing");

    // Pinning the valid manifest succeeds in text and JSON.
    let pin_out = invoke(&["pin", "--cas", cas_arg, &manifest]);
    assert_eq!(code(&pin_out), 0, "pin failed: {}", stderr(&pin_out));
    assert_eq!(stdout(&pin_out).trim(), format!("pinned {manifest}"));
    assert!(cas.join("pins").join(format!("{manifest}.pin")).exists());

    let pin_json_out = invoke(&["pin", "--cas", cas_arg, &manifest, "--json"]);
    assert_eq!(code(&pin_json_out), 0);
    let pin_json: serde_json::Value =
        serde_json::from_str(&stdout(&pin_json_out)).expect("pin JSON");
    assert_eq!(pin_json["manifest"], manifest);
    assert_eq!(pin_json["pinned"], true);

    // Wait 1.1s so objects age > 0 whole seconds, then sweep with
    // `--grace-secs 0`: everything reachable from the pin survives.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let gc_pinned = invoke(&["gc", "--cas", cas_arg, "--grace-secs", "0", "--json"]);
    assert_eq!(code(&gc_pinned), 0, "gc failed: {}", stderr(&gc_pinned));
    let gc_json: serde_json::Value = serde_json::from_str(&stdout(&gc_pinned)).expect("gc JSON");
    assert_eq!(gc_json["objects"], 0);
    assert!(gc_json["reachable"].as_u64().unwrap_or(0) >= 3);

    // Unpinning removes the pin and lets GC sweep the unreferenced
    // objects; unpinning again is idempotent.
    let unpin_out = invoke(&["unpin", "--cas", cas_arg, &manifest, "--json"]);
    assert_eq!(code(&unpin_out), 0);
    let unpin_json: serde_json::Value =
        serde_json::from_str(&stdout(&unpin_out)).expect("unpin JSON");
    assert_eq!(unpin_json["manifest"], manifest);
    assert_eq!(unpin_json["pinned"], false);
    assert!(!cas.join("pins").join(format!("{manifest}.pin")).exists());

    let unpin_again = invoke(&["unpin", "--cas", cas_arg, &manifest]);
    assert_eq!(code(&unpin_again), 0);
    assert_eq!(stdout(&unpin_again).trim(), format!("unpinned {manifest}"));

    let gc_swept = invoke(&["gc", "--cas", cas_arg, "--grace-secs", "0", "--json"]);
    assert_eq!(code(&gc_swept), 0);
    let swept_json: serde_json::Value =
        serde_json::from_str(&stdout(&gc_swept)).expect("swept JSON");
    assert!(swept_json["objects"].as_u64().unwrap_or(0) >= 3);
    assert_eq!(swept_json["reachable"], 0);

    remove_tree(&dir);
}

#[test]
fn run_with_inputs_and_pin_flag() {
    let dir = fresh_dir("run-inputs");
    let cas = dir.join("cas");
    let cas_arg = cas.to_str().expect("utf8");
    let host_in = dir.join("external.txt");
    fs::write(&host_in, b"bound host bytes").expect("write host input");

    let spec_path = dir.join("consume.json");
    fs::write(
        &spec_path,
        r#"{"tasks":[{"task_id":"upper","executable":"sh","args":["-c","tr a-z A-Z < inputs/nested/in.txt > outputs/out.txt"],"env":{},"inputs":["nested/in.txt"],"outputs":["out.txt"],"depends_on":[]}]}"#,
    )
    .expect("write spec");

    let binding = format!("./nested/in.txt={}", host_in.display());
    let run_out = invoke(&[
        "run",
        spec_path.to_str().expect("utf8"),
        "--cas",
        cas_arg,
        "--work",
        dir.join("work1").to_str().expect("utf8"),
        "--input",
        &binding,
        "--pin",
        "--json",
    ]);
    assert_eq!(code(&run_out), 0, "run failed: {}", stderr(&run_out));
    let parsed: serde_json::Value = serde_json::from_str(&stdout(&run_out)).expect("run JSON");
    assert_eq!(parsed["exit_code"], 0);
    assert_eq!(parsed["pinned"], true);
    assert_eq!(parsed["tasks"][0]["status"], "succeeded");
    let manifest_hex = parsed["manifest"]
        .as_str()
        .expect("manifest str")
        .to_string();
    assert!(
        cas.join("pins")
            .join(format!("{manifest_hex}.pin"))
            .exists()
    );

    // Rerun with identical host input bytes hits the execution cache.
    let cached_out = invoke(&[
        "run",
        spec_path.to_str().expect("utf8"),
        "--cas",
        cas_arg,
        "--work",
        dir.join("work2").to_str().expect("utf8"),
        "--input",
        &binding,
        "--json",
    ]);
    assert_eq!(code(&cached_out), 0);
    let cached_json: serde_json::Value =
        serde_json::from_str(&stdout(&cached_out)).expect("cached JSON");
    assert_eq!(cached_json["pinned"], false);
    assert_eq!(cached_json["tasks"][0]["status"], "cached");

    // Verify the collected output matches `"BOUND HOST BYTES"`.
    let cas_store = verirun_store::cas_fs::Cas::open(&cas).expect("open store");
    let verified = verirun_cli::verify_manifest(
        &cas_store,
        &verirun_core::digest::Digest::from_hex(&manifest_hex).expect("digest"),
    )
    .expect("verifies");
    assert_eq!(
        cas_store
            .read_to_vec(&verified.tasks[0].artifacts[0].digest)
            .expect("artifact reads"),
        b"BOUND HOST BYTES"
    );

    remove_tree(&dir);
}

#[test]
fn invalid_input_bindings_leave_store_and_work_untouched() {
    // INV-LAT-4 extended to `--input`: malformed bindings, unsafe
    // sandbox paths, duplicate normalized paths, and missing host
    // files all exit 3 before creating `cas` or `work`.
    let dir = fresh_dir("bad-inputs");
    let spec_path = dir.join("spec.json");
    fs::write(
        &spec_path,
        r#"{"tasks":[{"task_id":"t","executable":"sh","args":["-c","true"],"env":{},"inputs":["in.txt"],"outputs":[],"depends_on":[]}]}"#,
    )
    .expect("write spec");
    let existing = dir.join("real.txt");
    fs::write(&existing, b"ok").expect("write existing");

    let cases: Vec<Vec<String>> = vec![
        vec!["--input".to_string(), "../escape.txt=real.txt".to_string()],
        vec![
            "--input".to_string(),
            format!("in.txt={}", dir.join("missing.txt").display()),
        ],
        vec!["--input".to_string(), format!("in.txt={}", dir.display())],
        vec![
            "--input".to_string(),
            format!("in.txt={}", existing.display()),
            "--input".to_string(),
            format!("./in.txt={}", existing.display()),
        ],
    ];
    for (idx, extra) in cases.iter().enumerate() {
        let cas = dir.join(format!("cas-{idx}"));
        let work = dir.join(format!("work-{idx}"));
        let mut args: Vec<&str> = vec![
            "run",
            spec_path.to_str().expect("utf8"),
            "--cas",
            cas.to_str().expect("utf8"),
            "--work",
            work.to_str().expect("utf8"),
        ];
        for item in extra {
            args.push(item.as_str());
        }
        let out = invoke(&args);
        assert_eq!(code(&out), 3, "case {idx}: {}", stderr(&out));
        assert!(!cas.exists(), "case {idx} must not create cas");
        assert!(!work.exists(), "case {idx} must not create work");
    }

    remove_tree(&dir);
}
