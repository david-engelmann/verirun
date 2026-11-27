#![no_main]

use libfuzzer_sys::fuzz_target;
use verirun_core::digest::{Digest, DomainTag, HasherFactory, StreamingHasher};
use verirun_core::manifest::{ArtifactRecord, Manifest, TaskRecord, diff_manifests};
use verirun_core::parse;

struct FnvMixer {
    lanes: [u64; 4],
}

impl FnvMixer {
    const fn new(seed: u8) -> Self {
        let base = 0xcbf2_9ce4_8422_2325_u64 ^ (seed as u64);
        Self {
            lanes: [
                base,
                base.wrapping_mul(0x9e37_79b9_7f4a_7c15),
                base.rotate_left(17),
                base.rotate_right(13),
            ],
        }
    }
}

impl StreamingHasher for FnvMixer {
    fn update(&mut self, bytes: &[u8]) {
        for (idx, &byte) in bytes.iter().enumerate() {
            let lane = &mut self.lanes[idx & 3];
            *lane ^= u64::from(byte);
            *lane = lane.wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn finalize(self: Box<Self>) -> Digest {
        let mut out = [0_u8; 32];
        for (i, lane) in self.lanes.iter().enumerate() {
            out[i * 8..(i + 1) * 8].copy_from_slice(&lane.to_le_bytes());
        }
        Digest::from_bytes(out)
    }
}

struct FuzzHasherFactory;

impl HasherFactory for FuzzHasherFactory {
    fn new_hasher(&self, tag: DomainTag) -> Box<dyn StreamingHasher> {
        let seed = tag.as_bytes().last().copied().unwrap_or(0);
        Box::new(FnvMixer::new(seed))
    }
}

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = core::str::from_utf8(data) {
        let _ = Digest::from_hex(text);
    }

    if let Ok(value) = parse::parse(data) {
        let _ = ArtifactRecord::from_value(&value);
        let _ = TaskRecord::from_value(&value);

        if let Ok(manifest) = Manifest::from_value(&value) {
            let canonical_bytes = manifest
                .canonical_bytes()
                .expect("parsed Manifest fields are validated into canonical domain");
            let reparsed_val =
                parse::parse(&canonical_bytes).expect("canonical manifest bytes must parse");
            let reparsed_manifest = Manifest::from_value(&reparsed_val)
                .expect("canonical manifest value must parse as Manifest");
            assert_eq!(reparsed_manifest, manifest);

            let _ = manifest.recompute_root(&FuzzHasherFactory);
            let _ = manifest.digest(&FuzzHasherFactory);
            assert!(diff_manifests(&manifest, &manifest).is_empty());
        }
    }
});
