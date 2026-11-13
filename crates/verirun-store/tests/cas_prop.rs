//! CAS property: put and get are inverse, and a repeated put of the
//! same bytes leaves exactly one object (INV-CAS-4).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;
use sha2::Digest as _;
use verirun_store::cas_fs::Cas;

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempCas {
    root: PathBuf,
    cas: Cas,
}

impl TempCas {
    fn new(label: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "verirun-test-{label}-{}-{unique}",
            std::process::id()
        ));
        let cas = Cas::open(&root).expect("store opens");
        Self { root, cas }
    }

    fn object_file_count(&self) -> usize {
        let objects = self.root.join("objects");
        let mut count = 0;
        for shard in std::fs::read_dir(&objects).expect("objects dir") {
            let shard = shard.expect("shard entry");
            if shard.path().is_dir() {
                count += std::fs::read_dir(shard.path()).expect("shard dir").count();
            }
        }
        count
    }
}

impl Drop for TempCas {
    fn drop(&mut self) {
        // Stored objects are read-only; clear the flags so removal
        // works for non-root users too.
        fn clear_readonly(path: &Path) {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_dir() {
                        clear_readonly(&entry_path);
                    } else if let Ok(mut perms) = std::fs::metadata(&entry_path)
                        .map(|meta| meta.permissions())
                    {
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
        clear_readonly(&self.root);
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn framed_digest(bytes: &[u8]) -> verirun_core::digest::Digest {
    let mut hasher = sha2::Sha256::new();
    hasher.update(b"verirun/v1/blob");
    hasher.update([0_u8]);
    hasher.update(bytes);
    let out: [u8; 32] = hasher.finalize().into();
    verirun_core::digest::Digest::from_bytes(out)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn prop_put_get_identity(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        let store = TempCas::new("prop");
        let digest = store.cas.put_bytes(&bytes).expect("put succeeds");
        prop_assert_eq!(digest, framed_digest(&bytes));
        let back = store.cas.read_to_vec(&digest).expect("get succeeds");
        prop_assert_eq!(back, bytes.clone());
        let again = store.cas.put_bytes(&bytes).expect("second put succeeds");
        prop_assert_eq!(again, digest);
        prop_assert_eq!(store.object_file_count(), 1);
        prop_assert!(store.cas.contains(&digest));
    }
}

#[test]
fn empty_blob_roundtrip() {
    let store = TempCas::new("empty");
    let digest = store.cas.put_bytes(&[]).expect("empty put succeeds");
    assert_eq!(digest, framed_digest(&[]));
    let back = store.cas.read_to_vec(&digest).expect("empty get succeeds");
    assert!(back.is_empty());
}
