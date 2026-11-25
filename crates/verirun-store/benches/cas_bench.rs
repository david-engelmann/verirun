#![forbid(unsafe_code)]
//! Throughput benchmarks for the filesystem store: hashing
//! (INV-LAT-1), verified reads (the store half of INV-LAT-3), and
//! puts (INV-CAS-8).
//!
//! Hand-rolled harness on `std::time::Instant`, so the bench adds
//! no dependencies. Run with `cargo bench -p verirun-store`; the
//! `--test` mode CI uses runs one small pass and exits.

use std::path::{Path, PathBuf};
use std::time::Instant;

use verirun_store::cas_fs::Cas;

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
                        reason = "bench cleanup must remove read-only store objects"
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

fn fresh_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("verirun-bench-{label}-{}", std::process::id()));
    remove_tree(&root);
    root
}

fn payload(size: usize, seed: u8) -> Vec<u8> {
    let mut bytes = vec![0u8; size];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = seed.wrapping_add(u8::try_from(index % 251).unwrap_or(0));
    }
    bytes
}

#[allow(
    clippy::cast_precision_loss,
    reason = "byte counts here are far below the exact-integer range of f64"
)]
fn bench_hash_throughput(smoke: bool) {
    let root = fresh_root("hash");
    let cas = Cas::open(&root).expect("store opens");
    let size = 4 * 1024 * 1024;
    let iterations: u32 = if smoke { 1 } else { 20 };
    let started = Instant::now();
    for index in 0..iterations {
        // Distinct bytes per pass, so every put hashes the full
        // payload instead of finding the object already stored.
        let seed = u8::try_from(index % 256).unwrap_or(0);
        cas.put_bytes(&payload(size, seed)).expect("put succeeds");
    }
    let elapsed = started.elapsed().as_secs_f64();
    let mib_per_sec = size as f64 * f64::from(iterations) / elapsed / 1_048_576.0;
    println!(
        "bench_hash_throughput: {size} bytes x {iterations} iterations in {elapsed:.3}s = {mib_per_sec:.1} MiB/s"
    );
    remove_tree(&root);
}

fn bench_put_throughput(smoke: bool) {
    let root = fresh_root("put");
    let cas = Cas::open(&root).expect("store opens");
    let iterations: u32 = if smoke { 1 } else { 500 };
    // Distinct bytes per pass, by construction: the iteration
    // counter is stamped into the first bytes, so no two passes
    // ever put the same payload and the presence pre-check
    // never fires. (A seed cycled mod 256 repeated payloads
    // from pass 257 on, and the back half of this bench quietly
    // measured repeat puts instead; audit finding F21.)
    let mut data = payload(64 * 1024, 3);
    let started = Instant::now();
    for index in 0..iterations {
        data[..8].copy_from_slice(&u64::from(index).to_le_bytes());
        cas.put_bytes(&data).expect("put succeeds");
    }
    let elapsed = started.elapsed().as_secs_f64();
    let puts_per_sec = f64::from(iterations) / elapsed;
    println!(
        "bench_put_throughput: {iterations} 64 KiB puts in {elapsed:.3}s = {puts_per_sec:.1} puts/s"
    );
    remove_tree(&root);
}

/// The repeat workload: the same payload put again and
/// again, which is what a warm cache does all day. The first
/// put pays the full protocol; every later put is a repeat,
/// so this bench is the one that prices the presence
/// pre-check in `put_bytes`.
fn bench_put_repeat_throughput(smoke: bool) {
    let root = fresh_root("put-repeat");
    let cas = Cas::open(&root).expect("store opens");
    let data = payload(64 * 1024, 7);
    let iterations: u32 = if smoke { 1 } else { 500 };
    let started = Instant::now();
    for _ in 0..iterations {
        cas.put_bytes(&data).expect("put succeeds");
    }
    let elapsed = started.elapsed().as_secs_f64();
    let puts_per_sec = f64::from(iterations) / elapsed;
    println!(
        "bench_put_repeat_throughput: {iterations} repeat 64 KiB puts in {elapsed:.3}s = {puts_per_sec:.1} puts/s"
    );
    remove_tree(&root);
}

#[allow(
    clippy::cast_precision_loss,
    reason = "byte counts here are far below the exact-integer range of f64"
)]
fn bench_verify_throughput(smoke: bool) {
    let root = fresh_root("verify");
    let cas = Cas::open(&root).expect("store opens");
    let data = payload(4 * 1024 * 1024, 42);
    let digest = cas.put_bytes(&data).expect("put succeeds");
    let iterations = if smoke { 1 } else { 50 };
    let started = Instant::now();
    for _ in 0..iterations {
        let back = cas.read_to_vec(&digest).expect("verified read succeeds");
        assert_eq!(back.len(), data.len());
    }
    let elapsed = started.elapsed().as_secs_f64();
    let mib_per_sec = data.len() as f64 * f64::from(iterations) / elapsed / 1_048_576.0;
    let bytes = data.len();
    println!(
        "bench_verify_throughput: {bytes} bytes x {iterations} verified reads in {elapsed:.3}s = {mib_per_sec:.1} MiB/s"
    );
    remove_tree(&root);
}

fn main() {
    let smoke = std::env::args().any(|arg| arg == "--test");
    bench_hash_throughput(smoke);
    bench_put_throughput(smoke);
    bench_put_repeat_throughput(smoke);
    bench_verify_throughput(smoke);
}
