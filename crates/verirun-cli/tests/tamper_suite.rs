#![forbid(unsafe_code)]
//! Tamper suite (INV-MAN-3, INV-MAN-4, INV-MAN-5, and the
//! root-field half of INV-MAN-6).
//!
//! Every corruption an attacker with filesystem access can make
//! to a stored run is caught by verification with exit code 2:
//! single-byte flips in leaf blobs at the first, middle, and last
//! positions, truncated and extended blobs, exhaustive single-byte
//! flips across a whole small manifest, digest pointer swaps
//! (caught at the manifest re-hash when the swap is written over
//! the original object, and at the Merkle root recomputation when
//! the swapped manifest is stored as its own object with the
//! honest root left in place), and an edited Merkle root field.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use verirun_cli::{run_spec_bytes, verify_manifest};
use verirun_core::digest::Digest;
use verirun_core::error::VerifyError;
use verirun_core::manifest::Manifest;
use verirun_store::cas_fs::Cas;
use verirun_store::scheduler::RunOutcome;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_root(label: &str) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "verirun-tamper-{label}-{}-{unique}",
        std::process::id()
    ))
}

fn remove_tree(root: &Path) {
    fn clear(path: &Path) {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() && !entry_path.is_symlink() {
                    clear(&entry_path);
                } else if let Ok(meta) = fs::metadata(&entry_path) {
                    let mut perms = meta.permissions();
                    #[allow(
                        clippy::permissions_set_readonly_false,
                        reason = "test cleanup must remove read-only fixtures"
                    )]
                    perms.set_readonly(false);
                    let _ = fs::set_permissions(&entry_path, perms);
                }
            }
        }
    }
    clear(root);
    let _ = fs::remove_dir_all(root);
}

fn run_spec(root: &Path, spec: &str) -> (Cas, RunOutcome) {
    let cas_root = root.join("cas");
    let outcome = run_spec_bytes(
        spec.as_bytes(),
        &cas_root,
        &root.join("work"),
        &BTreeMap::new(),
        2,
    )
    .expect("the fixture run succeeds");
    let cas = Cas::open(&cas_root).expect("store reopens");
    (cas, outcome)
}

/// Sets the object file writable or read only in the
/// platform's own terms, mode bits on Unix and the read
/// only attribute on Windows.
fn set_writable(path: &Path, writable: bool) {
    let mut perms = fs::metadata(path).expect("object exists").permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(if writable { 0o644 } else { 0o444 });
    }
    #[cfg(windows)]
    {
        #[allow(
            clippy::permissions_set_readonly_false,
            reason = "the tamper fixture must restore the read only attribute"
        )]
        perms.set_readonly(!writable);
    }
    fs::set_permissions(path, perms).expect("object permissions change");
}

/// Writes `bytes` over the object file for `digest`, bypassing the
/// store's read-only mode the way an attacker with filesystem
/// access would.
fn overwrite_object(cas: &Cas, digest: &Digest, bytes: &[u8]) {
    let path = cas.object_path(digest);
    set_writable(&path, true);
    fs::write(&path, bytes).expect("object overwritten");
    set_writable(&path, false);
}

fn flip_byte(cas: &Cas, digest: &Digest, position: usize) -> Vec<u8> {
    let original = cas
        .read_to_vec(digest)
        .expect("object reads before the flip");
    let mut flipped = original.clone();
    flipped[position] ^= 0xff;
    overwrite_object(cas, digest, &flipped);
    original
}

fn run_binary_verify(cas_root: &Path, digest: &Digest) -> Output {
    Command::new(env!("CARGO_BIN_EXE_verirun"))
        .args([
            "verify",
            &digest.to_hex(),
            "--cas",
            cas_root.to_str().expect("path is UTF-8"),
        ])
        .output()
        .expect("the binary runs")
}

const LEAF_SPEC: &str = r#"{"tasks":[{
    "task_id": "leaf",
    "executable": "sh",
    "args": ["-c", "printf leaf-artifact-content > outputs/a.txt; printf leaf-out; printf leaf-err >&2"],
    "env": {}, "inputs": [], "outputs": ["a.txt"], "depends_on": []
}]}"#;

#[test]
fn fault_bitflip_leaf_blobs() {
    let root = fresh_root("leaf");
    let (cas, outcome) = run_spec(&root, LEAF_SPEC);
    let record = &outcome.manifest.tasks[0];
    let roles: [(Digest, &str); 3] = [
        (record.artifacts[0].digest, "a.txt"),
        (record.stdout_digest, "stdout"),
        (record.stderr_digest, "stderr"),
    ];
    for (digest, path) in roles {
        let bytes = cas.read_to_vec(&digest).expect("blob reads");
        let len = bytes.len();
        for position in [0, len / 2, len - 1] {
            let original = flip_byte(&cas, &digest, position);
            let error = verify_manifest(&cas, &outcome.manifest_digest)
                .expect_err("a flipped blob byte must fail verification");
            assert!(
                matches!(
                    error,
                    VerifyError::BlobMismatch { expected, task_id: ref tid, path: ref failed_path, .. }
                    if expected == digest && tid == "leaf" && failed_path == path
                ),
                "flip at {position} in {path} must name the blob, got {error:?}"
            );
            overwrite_object(&cas, &digest, &original);
        }
        // A truncated blob and an extended blob are caught the
        // same way.
        let mut extended = bytes.clone();
        extended.push(0xaa);
        for damaged in [&bytes[..len - 1], &extended[..]] {
            overwrite_object(&cas, &digest, damaged);
            let error = verify_manifest(&cas, &outcome.manifest_digest)
                .expect_err("a resized blob must fail verification");
            assert!(
                matches!(error, VerifyError::BlobMismatch { expected, .. } if expected == digest),
                "resized {path} blob must mismatch, got {error:?}"
            );
            overwrite_object(&cas, &digest, &bytes);
        }
        assert!(
            verify_manifest(&cas, &outcome.manifest_digest).is_ok(),
            "the restored {path} blob verifies again"
        );
    }
    // One representative flip through the compiled binary: the
    // failure is exit code 2 and names the expected digest.
    let (artifact_digest, _) = roles[0];
    let original = flip_byte(&cas, &artifact_digest, 0);
    let output = run_binary_verify(&root.join("cas"), &outcome.manifest_digest);
    assert_eq!(
        output.status.code(),
        Some(2),
        "tampering reports as integrity failure, stderr {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(&artifact_digest.to_hex()),
        "the report names the corrupted digest"
    );
    overwrite_object(&cas, &artifact_digest, &original);
    remove_tree(&root);
}

#[test]
fn fault_bitflip_manifest_exhaustive_small() {
    let root = fresh_root("manifest");
    let spec = r#"{"tasks":[{
        "task_id": "only", "executable": "true", "args": [],
        "env": {}, "inputs": [], "outputs": [], "depends_on": []
    }]}"#;
    let (cas, outcome) = run_spec(&root, spec);
    let original = cas
        .read_to_vec(&outcome.manifest_digest)
        .expect("manifest reads");
    assert!(
        original.len() < 512,
        "the exhaustive fixture stays small, got {} bytes",
        original.len()
    );
    for position in 0..original.len() {
        let mut flipped = original.clone();
        flipped[position] ^= 0xff;
        overwrite_object(&cas, &outcome.manifest_digest, &flipped);
        assert!(
            verify_manifest(&cas, &outcome.manifest_digest).is_err(),
            "a flip at manifest byte {position} must be caught"
        );
        overwrite_object(&cas, &outcome.manifest_digest, &original);
    }
    assert!(verify_manifest(&cas, &outcome.manifest_digest).is_ok());
    // Through the binary, one flip is exit code 2.
    let mut flipped = original.clone();
    flipped[0] ^= 0xff;
    overwrite_object(&cas, &outcome.manifest_digest, &flipped);
    let output = run_binary_verify(&root.join("cas"), &outcome.manifest_digest);
    assert_eq!(output.status.code(), Some(2));
    overwrite_object(&cas, &outcome.manifest_digest, &original);
    remove_tree(&root);
}

const SWAP_SPEC: &str = r#"{"tasks":[
    {"task_id": "s1", "executable": "sh", "args": ["-c", "printf AAAA"],
     "env": {}, "inputs": [], "outputs": [], "depends_on": []},
    {"task_id": "s2", "executable": "sh", "args": ["-c", "printf BBBB"],
     "env": {}, "inputs": [], "outputs": [], "depends_on": []}
]}"#;

fn swapped_manifest(manifest: &Manifest) -> Manifest {
    let mut swapped = manifest.clone();
    let (first, rest) = swapped.tasks.split_at_mut(1);
    std::mem::swap(&mut first[0].stdout_digest, &mut rest[0].stdout_digest);
    swapped
}

#[test]
fn fault_digest_pointer_swaps() {
    // (a) The swap written over the original manifest object is
    // caught at the manifest blob re-hash.
    let root = fresh_root("swap-file");
    let (cas, outcome) = run_spec(&root, SWAP_SPEC);
    let swapped = swapped_manifest(&outcome.manifest);
    overwrite_object(
        &cas,
        &outcome.manifest_digest,
        &swapped.canonical_bytes().expect("swapped manifest encodes"),
    );
    let error = verify_manifest(&cas, &outcome.manifest_digest)
        .expect_err("a swapped manifest must fail verification");
    assert!(
        matches!(
            error,
            VerifyError::ManifestDigestMismatch { expected, .. }
            if expected == outcome.manifest_digest
        ),
        "the over-written swap fails the re-hash, got {error:?}"
    );
    let output = run_binary_verify(&root.join("cas"), &outcome.manifest_digest);
    assert_eq!(output.status.code(), Some(2));
    remove_tree(&root);

    // (b) The swapped manifest stored as its own object, with the
    // honest Merkle root left in place, is caught at the root
    // recomputation. The two stdout blobs have identical lengths,
    // so no length check could catch this swap.
    let root = fresh_root("swap-object");
    let (cas, outcome) = run_spec(&root, SWAP_SPEC);
    let swapped = swapped_manifest(&outcome.manifest);
    assert_eq!(swapped.merkle_root, outcome.manifest.merkle_root);
    let swapped_digest = cas
        .put_bytes(&swapped.canonical_bytes().expect("swapped manifest encodes"))
        .expect("swapped manifest stores");
    let error = verify_manifest(&cas, &swapped_digest)
        .expect_err("a swapped manifest must fail verification");
    assert!(
        matches!(
            error,
            VerifyError::RootMismatch { stored, recomputed }
            if stored == outcome.manifest.merkle_root && recomputed != stored
        ),
        "the stored swap fails the root recomputation, got {error:?}"
    );
    let output = run_binary_verify(&root.join("cas"), &swapped_digest);
    assert_eq!(output.status.code(), Some(2));
    remove_tree(&root);
}

#[test]
fn fault_root_field_edited() {
    let root = fresh_root("root-edit");
    let (cas, outcome) = run_spec(&root, SWAP_SPEC);
    let mut edited = outcome.manifest.clone();
    edited.merkle_root = Digest::from_bytes([0xee; 32]);
    let edited_digest = cas
        .put_bytes(&edited.canonical_bytes().expect("edited manifest encodes"))
        .expect("edited manifest stores");
    let error = verify_manifest(&cas, &edited_digest)
        .expect_err("an edited root field must fail verification");
    assert!(
        matches!(
            error,
            VerifyError::RootMismatch { stored, recomputed }
            if stored == edited.merkle_root && recomputed == outcome.manifest.merkle_root
        ),
        "the edited root names both roots, got {error:?}"
    );
    let output = run_binary_verify(&root.join("cas"), &edited_digest);
    assert_eq!(output.status.code(), Some(2));
    remove_tree(&root);
}
