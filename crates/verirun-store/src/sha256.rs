//! SHA-256 behind the core [`HasherFactory`] seam.
//!
//! This module is the only place in the workspace that names a hash
//! implementation. Core framing (the domain tag and its zero separator)
//! is applied by `verirun_core::digest::begin` before any payload byte
//! reaches this hasher.

use sha2::Digest as _;
use verirun_core::digest::{Digest, DomainTag, HasherFactory, StreamingHasher};

/// A streaming SHA-256 computation.
///
/// The inherent methods exist so tests can pin the raw hash against
/// published vectors without any framing. The
/// [`StreamingHasher`] implementation delegates to them.
#[derive(Debug)]
pub struct Sha256Hasher {
    state: sha2::Sha256,
}

impl Sha256Hasher {
    /// Starts a fresh computation.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: sha2::Sha256::new(),
        }
    }

    /// Feeds bytes, in stream order.
    pub fn update(&mut self, bytes: &[u8]) {
        self.state.update(bytes);
    }

    /// Completes the computation and returns the digest.
    #[must_use]
    pub fn finalize(self) -> Digest {
        let bytes: [u8; 32] = self.state.finalize().into();
        Digest::from_bytes(bytes)
    }
}

impl Default for Sha256Hasher {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingHasher for Sha256Hasher {
    fn update(&mut self, bytes: &[u8]) {
        Self::update(self, bytes);
    }

    fn finalize(self: Box<Self>) -> Digest {
        Self::finalize(*self)
    }
}

/// The factory the CLI injects at startup and tests use directly.
/// Stateless: every call returns an independent hasher.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sha256Factory;

impl HasherFactory for Sha256Factory {
    fn new_hasher(&self, _tag: DomainTag) -> Box<dyn StreamingHasher> {
        Box::new(Sha256Hasher::new())
    }
}
