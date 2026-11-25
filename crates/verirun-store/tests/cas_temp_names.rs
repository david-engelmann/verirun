//! Temp name collision after pid reuse (audit finding F9,
//! carried item C1, perfection round P1).
//!
//! This binary holds a single test on purpose: the store's
//! temp counter is process-wide, so a lone test in a fresh
//! process knows the counter starts at zero and can plant
//! debris under exactly the names the pre-P1 scheme would
//! mint for this process.

#![forbid(unsafe_code)]

use std::io::Cursor;
use std::path::PathBuf;

use verirun_store::cas_fs::Cas;

fn fresh_root(label: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("verirun-temp-names-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    root
}

fn remove_tree(root: &std::path::Path) {
    fn clear(path: &std::path::Path) {
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

/// Debris left by a dead process whose pid this process now
/// holds must not fail a put. The planted files carry the
/// names the old scheme minted for this pid: the known shape
/// `<digest>.<pid>.<worker>.<counter>.tmp` and the stream
/// shape `stream-<pid>.<worker>.<counter>.tmp`, over the
/// counter range a fresh process reaches first.
#[test]
fn puts_survive_debris_minted_under_a_reused_pid() {
    let root = fresh_root("pid-reuse");
    let cas = Cas::open(root.join("cas")).expect("store opens");
    let pid = std::process::id();
    let payload: &[u8] = b"p1 collision payload";
    let scratch = Cas::open(root.join("scratch-cas")).expect("scratch store opens");
    let digest = scratch
        .put_bytes(payload)
        .expect("scratch put computes the digest");
    let tmp_dir = root.join("cas").join("tmp");
    for worker in 0..2 {
        for counter in 0..32 {
            std::fs::write(
                tmp_dir.join(format!("{}.{pid}.{worker}.{counter}.tmp", digest.to_hex())),
                b"debris",
            )
            .expect("known-shape debris plants");
            std::fs::write(
                tmp_dir.join(format!("stream-{pid}.{worker}.{counter}.tmp")),
                b"debris",
            )
            .expect("stream-shape debris plants");
        }
    }
    let put = cas.put_bytes(payload).expect("known put survives debris");
    assert_eq!(put, digest);
    let streamed = cas
        .put_stream(Cursor::new(payload.to_vec()))
        .expect("streamed put survives debris");
    assert_eq!(streamed, digest);
    remove_tree(&root);
}
