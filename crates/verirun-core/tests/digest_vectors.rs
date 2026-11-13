//! Framing vectors, checked against a recording hasher that captures
//! the exact bytes core feeds the hash function.

use std::sync::{Arc, Mutex};

use verirun_core::digest::{
    Digest, DomainTag, HasherFactory, StreamingHasher, TaskStatus, begin, env_digest, exec_digest,
    task_digest,
};

#[derive(Default)]
struct Recorder {
    bytes: Mutex<Vec<u8>>,
}

struct RecordingHasher {
    recorder: Arc<Recorder>,
}

impl StreamingHasher for RecordingHasher {
    fn update(&mut self, bytes: &[u8]) {
        self.recorder
            .bytes
            .lock()
            .expect("recorder lock")
            .extend_from_slice(bytes);
    }

    fn finalize(self: Box<Self>) -> Digest {
        Digest::from_bytes([0xAB; 32])
    }
}

#[derive(Clone, Default)]
struct RecordingFactory {
    recorder: Arc<Recorder>,
}

impl RecordingFactory {
    fn take(&self) -> Vec<u8> {
        std::mem::take(&mut *self.recorder.bytes.lock().expect("recorder lock"))
    }
}

impl HasherFactory for RecordingFactory {
    fn new_hasher(&self, _tag: DomainTag) -> Box<dyn StreamingHasher> {
        Box::new(RecordingHasher {
            recorder: Arc::clone(&self.recorder),
        })
    }
}

fn push_text(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&(text.len() as u64).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
}

fn framed(tag: DomainTag) -> Vec<u8> {
    let mut out = tag.as_bytes().to_vec();
    out.push(0);
    out
}

#[test]
fn vec_domain_separation_prefixes() {
    let tags = [
        DomainTag::Blob,
        DomainTag::Input,
        DomainTag::Env,
        DomainTag::Exec,
        DomainTag::Artifact,
        DomainTag::Task,
        DomainTag::Node,
        DomainTag::Empty,
    ];
    let mut seen = std::collections::BTreeSet::new();
    for tag in tags {
        assert!(tag.as_bytes().starts_with(b"verirun/v1/"));
        assert!(seen.insert(tag.as_bytes().to_vec()), "tags are distinct");
        let factory = RecordingFactory::default();
        let hasher = begin(&factory, tag);
        hasher.finalize();
        assert_eq!(factory.take(), framed(tag), "prefix for {tag:?}");
    }
}

#[test]
fn vec_exec_tuple_fixed() {
    let factory = RecordingFactory::default();
    let env = Digest::from_bytes([0x11; 32]);
    let input_content = Digest::from_bytes([0x22; 32]);
    let input_entry = Digest::from_bytes([0x33; 32]);
    let _ = input_content;
    let digest = exec_digest(
        &factory,
        "/bin/echo",
        &["hello".to_string()],
        &env,
        &[("in.txt".to_string(), input_entry)],
        &["out.txt".to_string()],
    );
    assert_eq!(digest, Digest::from_bytes([0xAB; 32]));

    let mut expected = framed(DomainTag::Exec);
    push_text(&mut expected, "/bin/echo");
    expected.extend_from_slice(&1_u64.to_le_bytes());
    push_text(&mut expected, "hello");
    expected.extend_from_slice(env.as_bytes());
    expected.extend_from_slice(&1_u64.to_le_bytes());
    expected.extend_from_slice(input_entry.as_bytes());
    expected.extend_from_slice(&1_u64.to_le_bytes());
    push_text(&mut expected, "out.txt");
    assert_eq!(factory.take(), expected);
}

#[test]
fn vec_task_framing_fixed() {
    let factory = RecordingFactory::default();
    let exec = Digest::from_bytes([0x44; 32]);
    let stdout = Digest::from_bytes([0x55; 32]);
    let stderr = Digest::from_bytes([0x66; 32]);
    let artifact = Digest::from_bytes([0x77; 32]);
    let _framed = task_digest(
        &factory,
        "build",
        &exec,
        TaskStatus::TimedOut,
        None,
        &stdout,
        &stderr,
        &[artifact],
    );
    let mut expected = framed(DomainTag::Task);
    push_text(&mut expected, "build");
    expected.extend_from_slice(exec.as_bytes());
    expected.push(2); // TimedOut status byte
    expected.push(0); // exit code absent
    expected.extend_from_slice(stdout.as_bytes());
    expected.extend_from_slice(stderr.as_bytes());
    expected.extend_from_slice(&1_u64.to_le_bytes());
    expected.extend_from_slice(artifact.as_bytes());
    assert_eq!(factory.take(), expected);

    // With an exit code present, the presence byte is 1 and the i32
    // follows in little-endian.
    let factory = RecordingFactory::default();
    let _framed = task_digest(
        &factory,
        "build",
        &exec,
        TaskStatus::Failed,
        Some(-3),
        &stdout,
        &stderr,
        &[],
    );
    let mut expected = framed(DomainTag::Task);
    push_text(&mut expected, "build");
    expected.extend_from_slice(exec.as_bytes());
    expected.push(1); // Failed
    expected.push(1); // exit code present
    expected.extend_from_slice(&(-3_i32).to_le_bytes());
    expected.extend_from_slice(stdout.as_bytes());
    expected.extend_from_slice(stderr.as_bytes());
    expected.extend_from_slice(&0_u64.to_le_bytes());
    assert_eq!(factory.take(), expected);
}

#[test]
fn vec_env_sorting_and_framing() {
    let pairs = vec![
        ("B".to_string(), "2".to_string()),
        ("A".to_string(), "1".to_string()),
    ];
    let factory = RecordingFactory::default();
    let _framed = env_digest(&factory, &pairs);
    let mut expected = framed(DomainTag::Env);
    expected.extend_from_slice(&2_u64.to_le_bytes());
    push_text(&mut expected, "A");
    push_text(&mut expected, "1");
    push_text(&mut expected, "B");
    push_text(&mut expected, "2");
    assert_eq!(factory.take(), expected);
}

#[test]
fn digest_hex_roundtrip_and_rejection() {
    let digest = Digest::from_bytes([0x0A; 32]);
    let hex = digest.to_hex();
    assert_eq!(hex.len(), 64);
    assert!(
        hex.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
    assert_eq!(Digest::from_hex(&hex).expect("roundtrip"), digest);
    assert_eq!(format!("{digest}"), hex);

    let err = Digest::from_hex("AB").expect_err("too short");
    assert_eq!(
        err.kind(),
        verirun_core::error::DigestErrorKind::InvalidLength
    );
    let upper = "AB".repeat(32);
    let err = Digest::from_hex(&upper).expect_err("uppercase rejected");
    assert_eq!(
        err.kind(),
        verirun_core::error::DigestErrorKind::InvalidCharacter
    );
}
