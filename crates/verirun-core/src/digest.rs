//! Digests, domain separation, and tuple framing
//! (docs/ARCHITECTURE.md section 5).
//!
//! All digests are SHA-256 outputs of 32 bytes. Two framing patterns
//! cover every digest: Pattern S for raw byte streams (tag, one zero
//! byte, then the stream) and Pattern T for structured tuples (tag, one
//! zero byte, then length-prefixed and fixed-width fields in a stated
//! order). All multi-byte integers in framing are little-endian.
//!
//! The concrete hash function is not in this crate. Core defines the
//! [`StreamingHasher`] and [`HasherFactory`] seam (section 5.4) and owns
//! all framing, including writing the tag prefix. Adapters supply the
//! hasher implementation.

use core::fmt;
use core::str::FromStr;

use crate::error::{DigestError, DigestErrorKind};

/// A 32-byte SHA-256 digest. Displayed and parsed as 64 lowercase
/// hexadecimal characters, the only text form.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest([u8; 32]);

const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

impl Digest {
    /// Wraps raw digest bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The lowercase hex text form, 64 characters.
    #[must_use]
    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(64);
        for byte in self.0 {
            out.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
            out.push(char::from(HEX_DIGITS[usize::from(byte & 0x0F)]));
        }
        out
    }

    /// Parses the hex text form. Lowercase hex only, exactly 64
    /// characters.
    ///
    /// # Errors
    ///
    /// Returns [`DigestError`] naming the wrong length or the position
    /// of the first invalid character.
    pub fn from_hex(text: &str) -> Result<Self, DigestError> {
        if text.len() != 64 {
            return Err(DigestError::new(
                DigestErrorKind::InvalidLength,
                format!(
                    "a digest is 64 hex characters, got {} characters",
                    text.len()
                ),
            ));
        }
        let bytes = text.as_bytes();
        let mut out = [0_u8; 32];
        for index in 0..32 {
            let high = hex_nibble(bytes[2 * index], 2 * index)?;
            let low = hex_nibble(bytes[2 * index + 1], 2 * index + 1)?;
            out[index] = (high << 4) | low;
        }
        Ok(Self(out))
    }
}

fn hex_nibble(byte: u8, position: usize) -> Result<u8, DigestError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(DigestError::new(
            DigestErrorKind::InvalidCharacter,
            format!(
                "character {:?} at position {position} is not a lowercase hex digit",
                char::from(byte)
            ),
        )),
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Digest({})", self.to_hex())
    }
}

impl FromStr for Digest {
    type Err = DigestError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::from_hex(text)
    }
}

/// A domain separation tag (docs/ARCHITECTURE.md section 5.2). Every
/// tag is ASCII and shares the `verirun/v1/` prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DomainTag {
    /// Raw blob bytes: files, inputs, outputs, stdout, stderr.
    Blob,
    /// A task input entry: relative path plus content digest.
    Input,
    /// An environment pair set.
    Env,
    /// The execution tuple.
    Exec,
    /// An artifact record.
    Artifact,
    /// A task record.
    Task,
    /// A Merkle interior node.
    Node,
    /// The empty manifest root.
    Empty,
}

impl DomainTag {
    /// The exact tag bytes written at the head of every framed hash.
    #[must_use]
    pub const fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::Blob => b"verirun/v1/blob",
            Self::Input => b"verirun/v1/input",
            Self::Env => b"verirun/v1/env",
            Self::Exec => b"verirun/v1/exec",
            Self::Artifact => b"verirun/v1/artifact",
            Self::Task => b"verirun/v1/task",
            Self::Node => b"verirun/v1/node",
            Self::Empty => b"verirun/v1/empty",
        }
    }
}

/// One streaming hash computation, supplied by adapter crates
/// (docs/ARCHITECTURE.md section 5.4).
///
/// A hasher is created for exactly one digest, framed by core through
/// [`begin`], fed payload bytes through [`StreamingHasher::update`],
/// and consumed by [`StreamingHasher::finalize`].
pub trait StreamingHasher: Send {
    /// Feeds payload bytes, in stream order.
    fn update(&mut self, bytes: &[u8]);

    /// Completes the computation and returns the digest. Consumes the
    /// boxed hasher, so a digest cannot be taken twice.
    fn finalize(self: Box<Self>) -> Digest;
}

/// Creates hashers, one per digest (docs/ARCHITECTURE.md section 5.4).
/// Implemented by `verirun-store` over the `sha2` crate, and by test
/// doubles that record the bytes they are fed.
pub trait HasherFactory: Send + Sync {
    /// Creates a fresh, unframed hasher for one digest under `tag`.
    /// Callers go through [`begin`], which applies the framing.
    fn new_hasher(&self, tag: DomainTag) -> Box<dyn StreamingHasher>;
}

/// Creates a hasher through `factory` and applies the Pattern S/T
/// framing prefix: the tag bytes followed by one zero byte. Core owns
/// this write, so adapters only ever feed payload bytes.
///
/// # Panics
///
/// Never panics in the core implementation. Adapter hashers are
/// expected to accept the prefix through their normal update path.
#[must_use]
pub fn begin(factory: &dyn HasherFactory, tag: DomainTag) -> Box<dyn StreamingHasher> {
    let mut hasher = factory.new_hasher(tag);
    hasher.update(tag.as_bytes());
    hasher.update(&[0]);
    hasher
}

/// Computes a Pattern S digest over an in-memory byte string: the
/// digest of a blob, given its full bytes.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
pub fn hash_bytes(factory: &dyn HasherFactory, tag: DomainTag, bytes: &[u8]) -> Digest {
    let mut hasher = begin(factory, tag);
    hasher.update(bytes);
    hasher.finalize()
}

/// A streaming tuple framer for Pattern T digests.
///
/// Fields are appended in the caller's order, which is the framing
/// order of section 5.2. Variable-length fields carry a u64
/// little-endian length prefix. Digests are appended as their raw 32
/// bytes.
pub struct TupleHasher {
    hasher: Box<dyn StreamingHasher>,
}

impl TupleHasher {
    /// Starts a tuple under `tag`, framed through [`begin`].
    #[must_use]
    pub fn new(factory: &dyn HasherFactory, tag: DomainTag) -> Self {
        Self {
            hasher: begin(factory, tag),
        }
    }

    /// Appends a text field: u64 little-endian byte length, then the
    /// UTF-8 bytes.
    pub fn write_text(&mut self, text: &str) {
        self.write_bytes_field(text.as_bytes());
    }

    /// Appends a byte field with the same length prefix as
    /// [`TupleHasher::write_text`].
    pub fn write_bytes_field(&mut self, bytes: &[u8]) {
        self.write_u64(bytes.len() as u64);
        self.hasher.update(bytes);
    }

    /// Appends a u64 in little-endian, with no length prefix. Counts
    /// in section 5.2 use this exact form.
    pub fn write_u64(&mut self, value: u64) {
        self.hasher.update(&value.to_le_bytes());
    }

    /// Appends an i32 in little-endian, with no length prefix.
    pub fn write_i32(&mut self, value: i32) {
        self.hasher.update(&value.to_le_bytes());
    }

    /// Appends a boolean as one byte, 0x00 or 0x01.
    pub fn write_bool(&mut self, value: bool) {
        self.hasher.update(&[u8::from(value)]);
    }

    /// Appends a raw single byte. Status bytes and presence markers in
    /// section 5.2 use this exact form.
    pub fn write_u8(&mut self, value: u8) {
        self.hasher.update(&[value]);
    }

    /// Appends a digest as its raw 32 bytes, with no length prefix.
    pub fn write_digest(&mut self, digest: &Digest) {
        self.hasher.update(digest.as_bytes());
    }

    /// Completes the tuple and returns its digest.
    #[must_use]
    pub fn finalize(self) -> Digest {
        self.hasher.finalize()
    }
}

/// The terminal status of a task, with its framing byte from
/// docs/ARCHITECTURE.md section 5.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskStatus {
    /// The task ran and exited with code 0.
    Succeeded,
    /// The task ran and failed.
    Failed,
    /// The task exceeded its timeout and was killed.
    TimedOut,
    /// The task was served from the execution cache.
    Cached,
    /// The task never started because an ancestor failed or timed out.
    Skipped,
}

impl TaskStatus {
    /// The status byte used in task record framing.
    #[must_use]
    pub const fn status_byte(self) -> u8 {
        match self {
            Self::Succeeded => 0,
            Self::Failed => 1,
            Self::TimedOut => 2,
            Self::Cached => 3,
            Self::Skipped => 4,
        }
    }

    /// The status string used in manifest JSON.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
            Self::Cached => "cached",
            Self::Skipped => "skipped",
        }
    }
}

/// The digest of one task input entry: the relative path framed with
/// the blob digest of the file content, under the `Input` tag.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
pub fn input_digest(factory: &dyn HasherFactory, relative_path: &str, content: &Digest) -> Digest {
    let mut tuple = TupleHasher::new(factory, DomainTag::Input);
    tuple.write_text(relative_path);
    tuple.write_digest(content);
    tuple.finalize()
}

/// The digest of an environment pair set under the `Env` tag. Pairs
/// are sorted by key bytes, then value bytes, before framing, so the
/// input order of `pairs` does not affect the result.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
pub fn env_digest(factory: &dyn HasherFactory, pairs: &[(String, String)]) -> Digest {
    let mut sorted: Vec<&(String, String)> = pairs.iter().collect();
    sorted.sort_by(|left, right| {
        left.0
            .as_bytes()
            .cmp(right.0.as_bytes())
            .then_with(|| left.1.as_bytes().cmp(right.1.as_bytes()))
    });
    let mut tuple = TupleHasher::new(factory, DomainTag::Env);
    tuple.write_u64(sorted.len() as u64);
    for (key, value) in sorted {
        tuple.write_text(key);
        tuple.write_text(value);
    }
    tuple.finalize()
}

/// The execution digest, under the `Exec` tag.
///
/// The tuple is docs/ARCHITECTURE.md section 5.3. Inputs are framed
/// sorted by relative path (each given as path plus its `Input`
/// digest). Outputs are framed sorted by path bytes. `env` is the
/// [`env_digest`] of the declared pairs plus the fixed defaults.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
pub fn exec_digest(
    factory: &dyn HasherFactory,
    executable: &str,
    args: &[String],
    env: &Digest,
    inputs: &[(String, Digest)],
    outputs: &[String],
) -> Digest {
    let mut sorted_inputs: Vec<&(String, Digest)> = inputs.iter().collect();
    sorted_inputs.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
    let mut sorted_outputs: Vec<&String> = outputs.iter().collect();
    sorted_outputs.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    let mut tuple = TupleHasher::new(factory, DomainTag::Exec);
    tuple.write_text(executable);
    tuple.write_u64(args.len() as u64);
    for arg in args {
        tuple.write_text(arg);
    }
    tuple.write_digest(env);
    tuple.write_u64(sorted_inputs.len() as u64);
    for (_, input) in sorted_inputs {
        tuple.write_digest(input);
    }
    tuple.write_u64(sorted_outputs.len() as u64);
    for output in sorted_outputs {
        tuple.write_text(output);
    }
    tuple.finalize()
}

/// The digest of one artifact record under the `Artifact` tag.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
pub fn artifact_digest(
    factory: &dyn HasherFactory,
    relative_path: &str,
    content: &Digest,
    byte_len: u64,
    is_executable: bool,
) -> Digest {
    let mut tuple = TupleHasher::new(factory, DomainTag::Artifact);
    tuple.write_text(relative_path);
    tuple.write_digest(content);
    tuple.write_u64(byte_len);
    tuple.write_bool(is_executable);
    tuple.finalize()
}

/// The digest of one task record under the `Task` tag.
/// `artifact_digests` must be in the path order of their artifact
/// records, as section 5.2 requires.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
#[allow(
    clippy::too_many_arguments,
    reason = "the parameters are the task record fields of section 5.2, one per framed field; a parameter struct would hide the framing order the signature exists to state"
)]
pub fn task_digest(
    factory: &dyn HasherFactory,
    task_id: &str,
    exec: &Digest,
    status: TaskStatus,
    exit_code: Option<i32>,
    stdout: &Digest,
    stderr: &Digest,
    artifact_digests: &[Digest],
) -> Digest {
    let mut tuple = TupleHasher::new(factory, DomainTag::Task);
    tuple.write_text(task_id);
    tuple.write_digest(exec);
    tuple.write_u8(status.status_byte());
    match exit_code {
        Some(code) => {
            tuple.write_u8(1);
            tuple.write_i32(code);
        }
        None => {
            tuple.write_u8(0);
        }
    }
    tuple.write_digest(stdout);
    tuple.write_digest(stderr);
    tuple.write_u64(artifact_digests.len() as u64);
    for artifact in artifact_digests {
        tuple.write_digest(artifact);
    }
    tuple.finalize()
}

/// The digest of a Merkle interior node under the `Node` tag.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
pub fn node_digest(factory: &dyn HasherFactory, left: &Digest, right: &Digest) -> Digest {
    let mut hasher = begin(factory, DomainTag::Node);
    hasher.update(left.as_bytes());
    hasher.update(right.as_bytes());
    hasher.finalize()
}

/// The Merkle root of an empty leaf set: the Pattern S digest of the
/// empty payload under the `Empty` tag.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
pub fn empty_root(factory: &dyn HasherFactory) -> Digest {
    hash_bytes(factory, DomainTag::Empty, &[])
}

/// The Merkle root of `leaves`.
///
/// The construction is docs/ARCHITECTURE.md section 9.2: adjacent
/// pairing level by level, an odd element promoted unchanged, a
/// single leaf as its own root, and [`empty_root`] for an empty set.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
pub fn merkle_root(factory: &dyn HasherFactory, leaves: &[Digest]) -> Digest {
    if leaves.is_empty() {
        return empty_root(factory);
    }
    let mut level: Vec<Digest> = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            if pair.len() == 2 {
                next.push(node_digest(factory, &pair[0], &pair[1]));
            } else {
                next.push(pair[0]);
            }
        }
        level = next;
    }
    level[0]
}
