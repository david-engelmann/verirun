#![forbid(unsafe_code)]
//! Platform identity suite (INV-PLAT-1, perfection W5).
//!
//! The tutorial spec from docs/TUTORIAL.md runs through the
//! scheduler on this platform and its manifest digest must
//! equal the pinned digest, the same constant on Linux and
//! on Windows. A second run of the same spec must come back
//! fully cached, which covers the by-exec records on both
//! platforms as well.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use verirun_core::dag::validate_run_spec;
use verirun_core::digest::TaskStatus;
use verirun_core::parse::parse;
use verirun_store::cas_fs::Cas;
use verirun_store::scheduler;
use verirun_store::sha256::Sha256Factory;

/// The manifest digest of the tutorial spec, identical on
/// every platform (INV-PLAT-1). Learned from the hosted runs
/// of both platforms and pinned here.
const EXPECTED_HEX: &str = "e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a";

/// The tutorial spec, as a fallback copy.
///
/// The test reads the fenced block from docs/TUTORIAL.md
/// when it can, so the suite tracks the tutorial, and
/// falls back to this copy when the file is not present.
const TUTORIAL_SPEC: &str = r#"{"tasks":[
  {"task_id":"greet","executable":"sh","args":["-c","printf 'hello from verirun\\n' > outputs/greeting.txt"],"env":{},"inputs":[],"outputs":["greeting.txt"],"depends_on":[]},
  {"task_id":"count","executable":"sh","args":["-c","wc -c < inputs/greeting.txt > outputs/size.txt"],"env":{},"inputs":["greeting.txt"],"outputs":["size.txt"],"depends_on":["greet"]}
]}"#;

/// Reads the tutorial spec from docs/TUTORIAL.md.
///
/// Returns None when the file cannot be read or carries
/// no fenced json block.
fn tutorial_spec_from_docs() -> Option<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/TUTORIAL.md");
    let text = std::fs::read_to_string(path).ok()?;
    let start = text.find("```json")? + "```json".len();
    let rest = &text[start..];
    let end = rest.find("```")?;
    Some(rest[..end].trim().to_string())
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_root(label: &str) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "verirun-identity-{label}-{}-{unique}",
        std::process::id()
    ))
}

fn remove_store(root: &Path) {
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
fn tutorial_spec_manifest_digest_is_platform_identical() {
    let root = fresh_root("tutorial");
    let cas = Cas::open(&root).expect("store opens");
    let spec_text = tutorial_spec_from_docs().unwrap_or_else(|| TUTORIAL_SPEC.to_string());
    let value = parse(spec_text.as_bytes()).expect("spec parses");
    let dag = validate_run_spec(&value).expect("spec validates");
    let outcome = scheduler::run(
        &cas,
        &Sha256Factory,
        &root.join("work"),
        &dag,
        &BTreeMap::new(),
        2,
    )
    .expect("the tutorial run completes");
    for task in &outcome.manifest.tasks {
        assert_eq!(task.status, TaskStatus::Succeeded);
    }
    let digest_hex = outcome.manifest_digest.to_hex();
    println!("MANIFEST DIGEST {digest_hex}");
    assert_eq!(
        digest_hex, EXPECTED_HEX,
        "the tutorial manifest digest is platform identical (INV-PLAT-1)"
    );

    let cached = scheduler::run(
        &cas,
        &Sha256Factory,
        &root.join("work-again"),
        &dag,
        &BTreeMap::new(),
        2,
    )
    .expect("the second tutorial run completes");
    for task in &cached.manifest.tasks {
        assert_eq!(task.status, TaskStatus::Cached);
    }
    remove_store(&root);
}
