//! Concurrent writer stress: identical and distinct puts racing on
//! one store must all hold (INV-CAS-5).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

use sha2::Digest as _;
use verirun_core::digest::Digest;
use verirun_store::cas_fs::Cas;

fn framed_digest(bytes: &[u8]) -> Digest {
    let mut hasher = sha2::Sha256::new();
    hasher.update(b"verirun/v1/blob");
    hasher.update([0_u8]);
    hasher.update(bytes);
    let out: [u8; 32] = hasher.finalize().into();
    Digest::from_bytes(out)
}

fn remove_store(root: &Path) {
    fn clear(path: &Path) {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    clear(&entry_path);
                } else if let Ok(meta) = std::fs::metadata(&entry_path) {
                    let mut perms = meta.permissions();
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
    clear(root);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stress_parallel_identical_and_distinct_puts() {
    let root: PathBuf = std::env::temp_dir().join(format!("verirun-stress-{}", std::process::id()));
    let cas = Arc::new(Cas::open(&root).expect("opens"));
    let shared: Arc<Vec<u8>> = Arc::new((0_u32..50_000).map(|i| (i % 249) as u8).collect());

    let threads = 32;
    let barrier = Arc::new(Barrier::new(threads));
    let mut handles = Vec::new();
    for index in 0..threads {
        let cas = Arc::clone(&cas);
        let barrier = Arc::clone(&barrier);
        let shared = Arc::clone(&shared);
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            if index % 2 == 0 {
                cas.put_bytes(&shared).expect("shared put")
            } else {
                let distinct = format!("distinct payload {index}").into_bytes().repeat(200);
                cas.put_bytes(&distinct).expect("distinct put")
            }
        }));
    }
    let mut digests = Vec::new();
    for handle in handles {
        digests.push(handle.join().expect("worker thread"));
    }

    let shared_digest = framed_digest(&shared);
    for (index, digest) in digests.iter().enumerate() {
        if index % 2 == 0 {
            assert_eq!(*digest, shared_digest, "identical content, one digest");
        }
        let back = cas.read_to_vec(digest).expect("every put verifies");
        assert_eq!(framed_digest(&back), *digest);
    }

    // Sweep the store: every object file's name must equal the framed
    // digest of its content, and there are exactly 1 + 16 objects.
    let mut count = 0;
    let objects = root.join("objects");
    for dir_entry in std::fs::read_dir(&objects).expect("objects dir") {
        let dir = dir_entry.expect("shard entry");
        if dir.path().is_dir() {
            for object in std::fs::read_dir(dir.path()).expect("shard dir") {
                let object = object.expect("object");
                let content = std::fs::read(object.path()).expect("object bytes");
                let name = object.file_name().to_string_lossy().into_owned();
                assert_eq!(framed_digest(&content).to_hex(), name);
                count += 1;
            }
        }
    }
    assert_eq!(count, 17);
    remove_store(&root);
}
