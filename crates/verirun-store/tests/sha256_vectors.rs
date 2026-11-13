//! SHA-256 pins: published NIST vectors for the raw hasher, the
//! framed blob digest against an independent in-test hash, and
//! chunking invariance through the factory seam.

use sha2::Digest as _;
use verirun_core::digest::{DomainTag, begin, hash_bytes};
use verirun_store::sha256::{Sha256Factory, Sha256Hasher};

#[test]
fn sha256_nist_vectors() {
    let cases: &[(&[u8], &str)] = &[
        (
            b"",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
    ];
    for (input, expected_hex) in cases {
        let mut hasher = Sha256Hasher::new();
        hasher.update(input);
        assert_eq!(hasher.finalize().to_hex(), *expected_hex);
    }
}

#[test]
fn framed_blob_digest_matches_independent_hash() {
    let payload = b"some blob content\x00with binary\xFF";
    let mut expected = sha2::Sha256::new();
    expected.update(b"verirun/v1/blob");
    expected.update([0_u8]);
    expected.update(payload);
    let expected_bytes: [u8; 32] = expected.finalize().into();

    let digest = hash_bytes(&Sha256Factory, DomainTag::Blob, payload);
    assert_eq!(digest.as_bytes(), &expected_bytes);
}

#[test]
fn streaming_chunking_invariance_real_hash() {
    let payload: Vec<u8> = (0_u32..100_000).map(|i| (i % 251) as u8).collect();
    let reference = hash_bytes(&Sha256Factory, DomainTag::Blob, &payload);
    for chunk in [1_usize, 13, 4096, 65_536] {
        let mut hasher = begin(&Sha256Factory, DomainTag::Blob);
        for piece in payload.chunks(chunk) {
            hasher.update(piece);
        }
        assert_eq!(hasher.finalize(), reference, "chunk size {chunk}");
    }
}
