//! Framing properties: injectivity, chunking invariance, and
//! execution tuple sensitivity, over the recording hasher seam.

use std::sync::{Arc, Mutex};

use proptest::prelude::*;
use verirun_core::digest::{
    Digest, DomainTag, HasherFactory, StreamingHasher, TupleHasher, begin, exec_digest,
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
        Digest::from_bytes([0xCD; 32])
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

fn frame_fields(fields: &[String]) -> Vec<u8> {
    let factory = RecordingFactory::default();
    let mut tuple = TupleHasher::new(&factory, DomainTag::Exec);
    for field in fields {
        tuple.write_text(field);
    }
    tuple.finalize();
    factory.take()
}

proptest! {
    #[test]
    fn prop_tuple_framing_injective(
        left in prop::collection::vec("[a-c]{0,3}", 0..4),
        right in prop::collection::vec("[a-c]{0,3}", 0..4),
    ) {
        prop_assume!(left != right);
        prop_assert_ne!(frame_fields(&left), frame_fields(&right));
    }

    #[test]
    fn prop_streaming_chunking_invariant(
        payload in prop::collection::vec(any::<u8>(), 0..512),
        chunk in 1_usize..64,
    ) {
        let whole_factory = RecordingFactory::default();
        let mut hasher = begin(&whole_factory, DomainTag::Blob);
        hasher.update(&payload);
        hasher.finalize();
        let whole = whole_factory.take();

        let chunked_factory = RecordingFactory::default();
        let mut hasher = begin(&chunked_factory, DomainTag::Blob);
        for piece in payload.chunks(chunk) {
            hasher.update(piece);
        }
        hasher.finalize();
        prop_assert_eq!(whole, chunked_factory.take());
    }
}

#[test]
fn prop_exec_tuple_sensitivity() {
    let env = Digest::from_bytes([0x11; 32]);
    let input_entry = Digest::from_bytes([0x33; 32]);
    let frame = |executable: &str,
                 args: &[String],
                 env: &Digest,
                 inputs: &[(String, Digest)],
                 outputs: &[String]| {
        let factory = RecordingFactory::default();
        exec_digest(&factory, executable, args, env, inputs, outputs);
        factory.take()
    };
    let base_args = vec!["a".to_string(), "b".to_string()];
    let base_inputs = vec![("in.txt".to_string(), input_entry)];
    let base_outputs = vec!["out.txt".to_string()];
    let base = frame("/bin/echo", &base_args, &env, &base_inputs, &base_outputs);

    let mut other_env = *env.as_bytes();
    other_env[0] ^= 1;
    let mut other_input = *input_entry.as_bytes();
    other_input[31] ^= 1;
    let variants: Vec<Vec<u8>> = vec![
        frame("/bin/cat", &base_args, &env, &base_inputs, &base_outputs),
        frame(
            "/bin/echo",
            &["b".to_string(), "a".to_string()],
            &env,
            &base_inputs,
            &base_outputs,
        ),
        frame(
            "/bin/echo",
            &base_args,
            &Digest::from_bytes(other_env),
            &base_inputs,
            &base_outputs,
        ),
        frame(
            "/bin/echo",
            &base_args,
            &env,
            &[("in.txt".to_string(), Digest::from_bytes(other_input))],
            &base_outputs,
        ),
        frame("/bin/echo", &base_args, &env, &base_inputs, &[]),
        frame("/bin/echo", &[], &env, &base_inputs, &base_outputs),
    ];
    for variant in &variants {
        assert_ne!(&base, variant, "every field change moves the framing");
    }
    // Paths bind through the pairing: with two inputs, swapping which
    // digest belongs to which path moves the framing, while listing
    // the same pairing in a different order does not.
    let other_entry = Digest::from_bytes([0x99; 32]);
    let paired = frame(
        "/bin/echo",
        &base_args,
        &env,
        &[
            ("a.txt".to_string(), input_entry),
            ("b.txt".to_string(), other_entry),
        ],
        &base_outputs,
    );
    let swapped = frame(
        "/bin/echo",
        &base_args,
        &env,
        &[
            ("a.txt".to_string(), other_entry),
            ("b.txt".to_string(), input_entry),
        ],
        &base_outputs,
    );
    assert_ne!(paired, swapped);
    let reordered = frame(
        "/bin/echo",
        &base_args,
        &env,
        &[
            ("b.txt".to_string(), other_entry),
            ("a.txt".to_string(), input_entry),
        ],
        &base_outputs,
    );
    assert_eq!(paired, reordered);
}
