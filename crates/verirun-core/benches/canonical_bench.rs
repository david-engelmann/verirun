#![forbid(unsafe_code)]
//! Throughput benchmark for the canonical encoder (INV-LAT-1).
//!
//! Hand-rolled harness on `std::time::Instant`, so the bench adds
//! no dependencies. Run with `cargo bench -p verirun-core`; the
//! `--test` mode CI uses runs one small pass and exits.

use std::time::Instant;

use verirun_core::canonical;
use verirun_core::value::Value;

/// A wide, nested value whose canonical form is a few hundred
/// kilobytes: 256 task-like objects, each with strings, numbers,
/// and a nested array.
fn large_value() -> Value {
    let mut tasks = Vec::new();
    for index in 0..256 {
        let mut fields = Vec::new();
        for field in 0..16 {
            fields.push((
                format!("field_{field:02}"),
                Value::String(format!("value {index} {field} with some text")),
            ));
        }
        fields.push(("timeout_ms".to_string(), Value::Number(30_000.0)));
        fields.push((
            "args".to_string(),
            Value::Array(
                (0..8)
                    .map(|n| Value::String(format!("--flag-{n}")))
                    .collect(),
            ),
        ));
        tasks.push(Value::Object(fields));
    }
    Value::Object(vec![("tasks".to_string(), Value::Array(tasks))])
}

#[allow(
    clippy::cast_precision_loss,
    reason = "byte counts here are far below the exact-integer range of f64"
)]
fn bench_encode_throughput(smoke: bool) {
    let value = large_value();
    let mut sink: Vec<u8> = Vec::new();
    canonical::encode(&value, &mut sink).expect("the fixture value encodes");
    let bytes_per_pass = sink.len() as f64;
    let iterations = if smoke { 1 } else { 200 };
    let started = Instant::now();
    for _ in 0..iterations {
        sink.clear();
        canonical::encode(&value, &mut sink).expect("the fixture value encodes");
    }
    let elapsed = started.elapsed().as_secs_f64();
    let mib_per_sec = bytes_per_pass * f64::from(iterations) / elapsed / 1_048_576.0;
    let bytes = sink.len();
    println!(
        "bench_encode_throughput: {bytes} bytes x {iterations} iterations in {elapsed:.3}s = {mib_per_sec:.1} MiB/s"
    );
}

fn main() {
    let smoke = std::env::args().any(|arg| arg == "--test");
    bench_encode_throughput(smoke);
}
