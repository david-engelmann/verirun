//! CAS fault oracles: crash states at each put step, call order,
//! read-only objects, cross-device refusal, source mutation after
//! put, and the streaming chunk bound.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::sync::atomic::{AtomicU64, Ordering};

use verirun_core::cas::PutPhase;
use verirun_store::cas_fs::{Cas, PutObserver};
use verirun_store::error::StoreError;

static COUNTER: AtomicU64 = AtomicU64::new(0);
static HOOK: Once = Once::new();

fn fresh_root(label: &str) -> PathBuf {
    HOOK.call_once(|| {
        // Crash-injection tests panic on purpose; keep the output
        // readable.
        std::panic::set_hook(Box::new(|_| {}));
    });
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "verirun-fault-{label}-{}-{unique}",
        std::process::id()
    ))
}

fn remove_store(root: &Path) {
    fn clear(path: &Path) {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() && !entry_path.is_symlink() {
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

#[derive(Default)]
struct RecordingObserver {
    phases: Vec<PutPhase>,
    chunks: Vec<usize>,
}

impl PutObserver for RecordingObserver {
    fn on_chunk(&mut self, byte_len: usize) {
        self.chunks.push(byte_len);
    }

    fn on_phase(&mut self, phase: PutPhase) {
        self.phases.push(phase);
    }
}

struct CrashAt {
    phase: PutPhase,
}

impl PutObserver for CrashAt {
    fn on_phase(&mut self, phase: PutPhase) {
        assert!(phase != self.phase, "simulated crash at {phase:?}");
    }
}

#[test]
fn fault_put_call_order_recorded() {
    let root = fresh_root("order");
    let cas = Cas::open(&root).expect("opens");
    let mut observer = RecordingObserver::default();
    let digest = cas
        .put_bytes_observed(b"ordered put", &mut observer)
        .expect("put succeeds");
    assert_eq!(
        observer.phases,
        vec![
            PutPhase::Writing,
            PutPhase::FileSynced,
            PutPhase::Renamed,
            PutPhase::Durable,
        ]
    );
    assert_eq!(observer.chunks.iter().sum::<usize>(), b"ordered put".len());
    assert!(observer.chunks.iter().all(|len| *len <= 65_536));
    assert!(cas.contains(&digest));
    remove_store(&root);
}

#[test]
fn fault_crash_at_each_put_step() {
    let payload = b"crash test payload";
    for crash_phase in [PutPhase::Writing, PutPhase::FileSynced, PutPhase::Renamed] {
        let root = fresh_root("crash");
        let cas = Cas::open(&root).expect("opens");
        let mut injector = CrashAt { phase: crash_phase };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cas.put_bytes_observed(payload, &mut injector)
        }));
        assert!(outcome.is_err(), "the injected crash must propagate");
        drop(cas);

        // The caller treats the put as failed and may retry. Whatever
        // survived, it is either absent or a complete valid object.
        let reopened = Cas::open(&root).expect("reopens after crash");
        let expected = {
            let mut observer = RecordingObserver::default();
            reopened
                .put_bytes_observed(payload, &mut observer)
                .expect("retry put succeeds")
        };
        if crash_phase == PutPhase::Renamed {
            // The rename may or may not have become visible; both are
            // legal intermediate states. After the retry, the object
            // is present and verifies.
            assert!(reopened.contains(&expected));
        }
        let back = reopened
            .read_to_vec(&expected)
            .expect("object verifies after retry");
        assert_eq!(back, payload);
        remove_store(&root);
    }
}

#[test]
fn fault_stored_object_write_attempt() {
    let root = fresh_root("readonly");
    let cas = Cas::open(&root).expect("opens");
    let payload = b"immutable content";
    let digest = cas.put_bytes(payload).expect("put succeeds");
    let path = cas.object_path(&digest);
    let metadata = std::fs::metadata(&path).expect("object exists");
    assert!(
        metadata.permissions().readonly(),
        "object file is read-only"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            metadata.permissions().mode() & 0o222,
            0,
            "no write bits on a stored object"
        );
    }
    // Attempting to open the object for writing must not change its
    // content through the store's own view of the world. Under an
    // unprivileged user the open itself fails; either way the stored
    // bytes verify afterwards.
    let _ = std::fs::OpenOptions::new().write(true).open(&path);
    let back = cas.read_to_vec(&digest).expect("still verifies");
    assert_eq!(back, payload);
    remove_store(&root);
}

#[test]
fn fault_cross_device_tmp_refused() {
    let shm = Path::new("/dev/shm");
    assert!(shm.is_dir(), "this test needs /dev/shm as a second device");
    let root = fresh_root("xdev");
    let target = shm.join(format!("verirun-fault-xdev-target-{}", std::process::id()));
    std::fs::create_dir_all(root.join("objects")).expect("objects dir");
    std::fs::create_dir_all(&target).expect("shm target dir");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, root.join("tmp")).expect("tmp symlink");
    let result = Cas::open(&root);
    match result {
        Err(StoreError::CrossDevice { .. }) => {}
        Err(other) => panic!("expected CrossDevice refusal, got {other}"),
        Ok(_) => panic!("expected CrossDevice refusal, store opened"),
    }
    let _ = std::fs::remove_dir_all(&target);
    remove_store(&root);
}

#[test]
fn fault_source_mutation_after_put() {
    let root = fresh_root("mutate");
    let source = root.join("source.bin");
    std::fs::create_dir_all(&root).expect("root dir");
    let original: Vec<u8> = (0_u32..10_000).map(|i| (i % 253) as u8).collect();
    std::fs::write(&source, &original).expect("source written");

    let cas_root = root.join("cas");
    let cas = Cas::open(&cas_root).expect("opens");
    let digest = cas.put_file(&source).expect("put_file succeeds");

    // Mutate the source in place and grow it. The stored blob must
    // not move: puts copy, they never hardlink.
    std::fs::write(&source, vec![0xEE; 20_000]).expect("source mutated");
    let back = cas.read_to_vec(&digest).expect("stored blob verifies");
    assert_eq!(back, original);

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let source_ino = std::fs::metadata(&source).expect("source meta").ino();
        let object_ino = std::fs::metadata(cas.object_path(&digest))
            .expect("object meta")
            .ino();
        assert_ne!(source_ino, object_ino, "copy, not hardlink");
    }
    remove_store(&root);
}

#[test]
fn fault_counting_sink_chunk_bound() {
    let root = fresh_root("chunks");
    let cas = Cas::open(&root).expect("opens");
    let payload: Vec<u8> = (0_u32..200_000).map(|i| (i % 241) as u8).collect();
    let mut observer = RecordingObserver::default();
    let digest = cas
        .put_stream_observed(Cursor::new(&payload), &mut observer)
        .expect("streamed put succeeds");
    assert!(observer.chunks.iter().all(|len| *len <= 65_536));
    assert_eq!(observer.chunks.iter().sum::<usize>(), payload.len());
    assert_eq!(observer.chunks.len(), 4, "ceil(200000 / 65536)");
    let back = cas.read_to_vec(&digest).expect("verifies");
    assert_eq!(back, payload);
    remove_store(&root);
}
