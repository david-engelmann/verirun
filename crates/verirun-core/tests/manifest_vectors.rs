//! Merkle shape vectors (INV-MAN-6).
//!
//! The shapes 0, 1, 2, 3, and 5 pin the construction of
//! section 9.2 against hand-composed node digests: odd leaves are
//! promoted unchanged, never duplicated, a single leaf is its own
//! root, and zero tasks give the empty digest. A deterministic
//! mixing hasher stands in for SHA-256 so the tree logic is what
//! is under test; the real factory is pinned by the adapter
//! suites.

use verirun_core::digest::{
    Digest, DomainTag, HasherFactory, StreamingHasher, TaskStatus, empty_root, hash_bytes,
    node_digest,
};
use verirun_core::manifest::{Manifest, TaskRecord};

struct ShapeHasher {
    acc: u64,
    len: u64,
}

impl Default for ShapeHasher {
    fn default() -> Self {
        Self {
            acc: 0xcbf2_9ce4_8422_2325,
            len: 0,
        }
    }
}

const fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

impl StreamingHasher for ShapeHasher {
    fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.acc ^= u64::from(*byte);
            self.acc = self.acc.wrapping_mul(0x0000_0100_0000_01b3);
            self.len += 1;
        }
    }

    fn finalize(self: Box<Self>) -> Digest {
        let mut state = self.acc ^ self.len.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut out = [0u8; 32];
        for chunk in out.chunks_exact_mut(8) {
            chunk.copy_from_slice(&splitmix(&mut state).to_le_bytes());
        }
        Digest::from_bytes(out)
    }
}

#[derive(Clone, Default)]
struct ShapeFactory;

impl HasherFactory for ShapeFactory {
    fn new_hasher(&self, _tag: DomainTag) -> Box<dyn StreamingHasher> {
        Box::<ShapeHasher>::default()
    }
}

fn record(factory: &ShapeFactory, id: &str) -> TaskRecord {
    let empty = hash_bytes(factory, DomainTag::Blob, b"");
    TaskRecord::new(
        id.to_string(),
        hash_bytes(factory, DomainTag::Exec, id.as_bytes()),
        TaskStatus::Succeeded,
        Some(0),
        empty,
        empty,
        Vec::new(),
    )
}

#[test]
fn vec_merkle_shapes_0_1_2_3_5() {
    let factory = ShapeFactory;

    // Zero tasks: the empty digest.
    let m0 = Manifest::build(&factory, Vec::new());
    assert_eq!(m0.tasks.len(), 0);
    assert_eq!(m0.merkle_root, empty_root(&factory));

    let records: Vec<TaskRecord> = ["a", "b", "c", "d", "e"]
        .iter()
        .map(|id| record(&factory, id))
        .collect();
    let leaves: Vec<Digest> = records
        .iter()
        .map(|rec| rec.record_digest(&factory))
        .collect();

    // One task: the single leaf is its own root.
    let m1 = Manifest::build(&factory, records[..1].to_vec());
    assert_eq!(m1.merkle_root, leaves[0]);

    // Two tasks: one interior node.
    let m2 = Manifest::build(&factory, records[..2].to_vec());
    assert_eq!(
        m2.merkle_root,
        node_digest(&factory, &leaves[0], &leaves[1])
    );

    // Three tasks: the odd leaf is promoted unchanged and paired
    // at the next level, never duplicated.
    let m3 = Manifest::build(&factory, records[..3].to_vec());
    let n01 = node_digest(&factory, &leaves[0], &leaves[1]);
    assert_eq!(m3.merkle_root, node_digest(&factory, &n01, &leaves[2]));

    // Five tasks: promotion at two levels.
    let m5 = Manifest::build(&factory, records.clone());
    let n23 = node_digest(&factory, &leaves[2], &leaves[3]);
    let n0123 = node_digest(&factory, &n01, &n23);
    assert_eq!(m5.merkle_root, node_digest(&factory, &n0123, &leaves[4]));

    // Build sorts records by task id: reversed input produces the
    // identical manifest.
    let mut shuffled = records;
    shuffled.reverse();
    assert_eq!(Manifest::build(&factory, shuffled), m5);

    // The stored root equals the root recomputed from the fields.
    assert_eq!(m5.recompute_root(&factory), m5.merkle_root);

    // The value form round-trips into the identical manifest.
    let parsed = Manifest::from_value(&m5.to_value()).expect("manifest value parses");
    assert_eq!(parsed, m5);

    // The manifest digest is the blob digest of its canonical
    // bytes.
    let bytes = m5.canonical_bytes().expect("manifest encodes");
    assert_eq!(
        m5.digest(&factory),
        hash_bytes(&factory, DomainTag::Blob, &bytes)
    );
}
