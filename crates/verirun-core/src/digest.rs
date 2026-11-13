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
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// Parses the hex text form. Lowercase hex only, exactly 64
    /// characters.
    ///
    /// # Errors
    ///
    /// Returns [`DigestError`] naming the wrong length or the position
    /// of the first invalid character.
    pub fn from_hex(text: &str) -> Result<Self, DigestError> {
        unimplemented!("Stage A contract; implemented in Stage B")
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!("Stage A contract; implemented in Stage B")
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!("Stage A contract; implemented in Stage B")
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
        panic!("Stage A contract; implemented in Stage B")
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
    unimplemented!("Stage A contract; implemented in Stage B")
}

/// Computes a Pattern S digest over an in-memory byte string: the
/// digest of a blob, given its full bytes.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
pub fn hash_bytes(factory: &dyn HasherFactory, tag: DomainTag, bytes: &[u8]) -> Digest {
    unimplemented!("Stage A contract; implemented in Stage B")
}

/// A streaming tuple framer for Pattern T digests. Fields are appended
/// in the caller's order, which is the framing order of section 5.2.
/// Variable-length fields carry a u64 little-endian length prefix.
/// Digests are appended as their raw 32 bytes.
pub struct TupleHasher {
    hasher: Box<dyn StreamingHasher>,
}

impl TupleHasher {
    /// Starts a tuple under `tag`, framed through [`begin`].
    #[must_use]
    pub fn new(factory: &dyn HasherFactory, tag: DomainTag) -> Self {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// Appends a text field: u64 little-endian byte length, then the
    /// UTF-8 bytes.
    pub fn write_text(&mut self, text: &str) {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// Appends a byte field with the same length prefix as
    /// [`TupleHasher::write_text`].
    pub fn write_bytes_field(&mut self, bytes: &[u8]) {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// Appends a u64 in little-endian, with no length prefix. Counts
    /// in section 5.2 use this exact form.
    pub fn write_u64(&mut self, value: u64) {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// Appends an i32 in little-endian, with no length prefix.
    pub fn write_i32(&mut self, value: i32) {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// Appends a boolean as one byte, 0x00 or 0x01.
    pub fn write_bool(&mut self, value: bool) {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// Appends a digest as its raw 32 bytes, with no length prefix.
    pub fn write_digest(&mut self, digest: &Digest) {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// Completes the tuple and returns its digest.
    #[must_use]
    pub fn finalize(self) -> Digest {
        unimplemented!("Stage A contract; implemented in Stage B")
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
        panic!("Stage A contract; implemented in Stage B")
    }

    /// The status string used in manifest JSON.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        panic!("Stage A contract; implemented in Stage B")
    }
}

/// The digest of one task input entry: the relative path framed with
/// the blob digest of the file content, under the `Input` tag.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
pub fn input_digest(
    factory: &dyn HasherFactory,
    relative_path: &str,
    content: &Digest,
) -> Digest {
    unimplemented!("Stage A contract; implemented in Stage B")
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
    unimplemented!("Stage A contract; implemented in Stage B")
}

/// The execution digest of docs/ARCHITECTURE.md section 5.3, under the
/// `Exec` tag. Inputs are framed sorted by relative path (each given
/// as path plus its `Input` digest). Outputs are framed sorted by
/// path bytes. `env` is the [`env_digest`] of the declared pairs plus
/// the fixed defaults.
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
    unimplemented!("Stage A contract; implemented in Stage B")
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
    unimplemented!("Stage A contract; implemented in Stage B")
}

/// The digest of one task record under the `Task` tag.
/// `artifact_digests` must be in the path order of their artifact
/// records, as section 5.2 requires.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
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
    unimplemented!("Stage A contract; implemented in Stage B")
}

/// The digest of a Merkle interior node under the `Node` tag.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
pub fn node_digest(factory: &dyn HasherFactory, left: &Digest, right: &Digest) -> Digest {
    unimplemented!("Stage A contract; implemented in Stage B")
}

/// The Merkle root of an empty leaf set: the Pattern S digest of the
/// empty payload under the `Empty` tag.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
pub fn empty_root(factory: &dyn HasherFactory) -> Digest {
    unimplemented!("Stage A contract; implemented in Stage B")
}

/// The Merkle root of `leaves`, under the construction of
/// docs/ARCHITECTURE.md section 9.2: adjacent pairing level by level,
/// an odd element promoted unchanged, a single leaf as its own root,
/// and [`empty_root`] for an empty set.
///
/// # Panics
///
/// Never panics in the core implementation.
#[must_use]
pub fn merkle_root(factory: &dyn HasherFactory, leaves: &[Digest]) -> Digest {
    unimplemented!("Stage A contract; implemented in Stage B")
}
