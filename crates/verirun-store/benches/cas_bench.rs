#![forbid(unsafe_code)]
//! Throughput benchmarks for the filesystem store: hashing
//! (INV-LAT-1), verified reads (the store half of INV-LAT-3), and
//! puts (INV-CAS-8).
//!
//! Hand-rolled harness on `std::time::Instant`, so the bench adds
//! no dependencies. Run with `cargo bench -p verirun-store`; the
//! `--test` mode CI uses runs one small pass and exits.

use std::path::PathBuf;
use std::time::Instant;

use verirun_store::cas_fs::Cas;

fn fresh_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("verirun-bench-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
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
    let _ = std::fs::remove_dir_all(&root);
}

fn bench_put_throughput(smoke: bool) {
    let root = fresh_root("put");
    let cas = Cas::open(&root).expect("store opens");
    let iterations: u32 = if smoke { 1 } else { 500 };
    let started = Instant::now();
    for index in 0..iterations {
        let seed = u8::try_from(index % 256).unwrap_or(0);
        cas.put_bytes(&payload(64 * 1024, seed))
            .expect("put succeeds");
    }
    let elapsed = started.elapsed().as_secs_f64();
    let puts_per_sec = f64::from(iterations) / elapsed;
    println!(
        "bench_put_throughput: {iterations} 64 KiB puts in {elapsed:.3}s = {puts_per_sec:.1} puts/s"
    );
    let _ = std::fs::remove_dir_all(&root);
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
    let _ = std::fs::remove_dir_all(&root);
}

fn main() {
    let smoke = std::env::args().any(|arg| arg == "--test");
    bench_hash_throughput(smoke);
    bench_put_throughput(smoke);
    bench_verify_throughput(smoke);
}
