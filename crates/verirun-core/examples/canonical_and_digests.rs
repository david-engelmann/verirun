//! Canonical encoding and digest framing, the two things
//! verirun-core exists to make deterministic.
//!
//! Run with: `cargo run -p verirun-core --example canonical_and_digests`
//!
//! The example parses one JSON text and builds the same value
//! through the constructors with its keys in the other order,
//! then shows both produce byte-identical canonical output. It
//! then frames a blob digest through core's domain separation.
//! Core ships no hash function (it is standard library only), so
//! the framing is demonstrated with a small FNV-1a teaching
//! double. Real runs use the SHA-256 factory in verirun-store,
//! as the store example shows.
#![forbid(unsafe_code)]

use verirun_core::canonical;
use verirun_core::digest::{Digest, DomainTag, HasherFactory, StreamingHasher, hash_bytes};
use verirun_core::parse;
use verirun_core::value::Value;

/// An FNV-1a hasher stretched to 32 bytes, present only so this
/// example can show core's framing without a real hash crate.
/// It is not collision resistant and must never address content.
struct FnvHasher {
    state: [u64; 4],
}

impl FnvHasher {
    const fn new() -> Self {
        Self {
            state: [
                0xcbf2_9ce4_8422_2325,
                0x8422_2325_cbf2_9ce4,
                0x9ce4_8422_2325_cbf2,
                0x2325_cbf2_9ce4_8422,
            ],
        }
    }
}

impl StreamingHasher for FnvHasher {
    fn update(&mut self, bytes: &[u8]) {
        for (index, byte) in bytes.iter().enumerate() {
            let lane = index % 4;
            self.state[lane] ^= u64::from(*byte);
            self.state[lane] = self.state[lane].wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn finalize(self: Box<Self>) -> Digest {
        let mut out = [0u8; 32];
        for (index, lane) in self.state.iter().enumerate() {
            out[index * 8..index * 8 + 8].copy_from_slice(&lane.to_le_bytes());
        }
        Digest::from_bytes(out)
    }
}

/// Hands out the teaching hasher, one per digest.
struct FnvFactory;

impl HasherFactory for FnvFactory {
    fn new_hasher(&self, _tag: DomainTag) -> Box<dyn StreamingHasher> {
        Box::new(FnvHasher::new())
    }
}

fn main() {
    let parsed = parse::parse(br#"{"b":[true,null],"a":1}"#).expect("the sample text parses");
    let built = Value::object(vec![
        (
            "a".to_string(),
            Value::number(1.0).expect("one is in the number domain"),
        ),
        (
            "b".to_string(),
            Value::Array(vec![Value::Bool(true), Value::Null]),
        ),
    ])
    .expect("the sample value is well formed");

    let parsed_bytes = canonical::to_vec(&parsed).expect("the parsed value encodes");
    let built_bytes = canonical::to_vec(&built).expect("the built value encodes");
    assert_eq!(
        parsed_bytes, built_bytes,
        "key order must not change the canonical bytes"
    );
    println!(
        "canonical form: {}",
        String::from_utf8(parsed_bytes.clone()).expect("canonical output is UTF-8")
    );

    let digest = hash_bytes(&FnvFactory, DomainTag::Blob, &parsed_bytes);
    println!("blob digest under the teaching hash: {}", digest.to_hex());
    println!("digests are framed by domain tag, so a blob and an exec record");
    println!("over the same bytes can never collide in the store's address space.");
}
