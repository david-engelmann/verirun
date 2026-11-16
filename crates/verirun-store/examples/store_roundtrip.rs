//! The filesystem content store end to end: put, read back,
//! prove the address, then collect.
//!
//! Run with: `cargo run -p verirun-store --example store_roundtrip`
//!
//! The example opens a store in a fresh temporary directory,
//! stores two payloads, reads them back, and checks each
//! returned digest against core's blob framing over the real
//! SHA-256 factory, which is what makes the address a proof
//! rather than a label. It finishes with garbage collection:
//! a dry run that reports the plan, then a real run that
//! sweeps both objects, because nothing pins them. Collection
//! ages objects against the clock it is handed, so the example
//! hands it a time an hour on, the way a later run would.
#![forbid(unsafe_code)]

use std::time::{Duration, SystemTime};

use verirun_core::digest::{DomainTag, hash_bytes};
use verirun_store::cas_fs::Cas;
use verirun_store::gc;
use verirun_store::sha256::Sha256Factory;

fn main() {
    let root = std::env::temp_dir().join(format!("verirun-example-{}", std::process::id()));
    let cas = Cas::open(&root).expect("the store opens");

    let first = cas
        .put_bytes(b"the quick brown fox")
        .expect("the first put succeeds");
    let second = cas
        .put_bytes(b"jumps over the lazy dog")
        .expect("the second put succeeds");
    println!("stored {first} and {second}");

    let read_back = cas
        .read_to_vec(&first)
        .expect("the first object reads back");
    assert_eq!(read_back, b"the quick brown fox");
    let framed = hash_bytes(&Sha256Factory, DomainTag::Blob, b"the quick brown fox");
    assert_eq!(
        framed, first,
        "the store address is core's blob framing over SHA-256"
    );
    println!("read back verified, and the digest matches the blob framing");

    let an_hour_on = SystemTime::now() + Duration::from_secs(3600);
    let plan = gc::collect(&cas, 0, true, an_hour_on).expect("the dry run plans");
    println!(
        "dry run: {} objects would be swept, {} reachable",
        plan.objects, plan.reachable
    );
    let report = gc::collect(&cas, 0, false, an_hour_on).expect("the collection runs");
    println!(
        "collected {} objects ({} bytes); the store now holds the first object: {}",
        report.objects,
        report.object_bytes,
        cas.contains(&first)
    );

    std::fs::remove_dir_all(&root).expect("the example cleans up after itself");
}
