//! The filesystem content-addressed store
//! (docs/ARCHITECTURE.md section 6).
//!
//! Layout under the store root: `objects/<xx>/<digest>` holding raw
//! bytes in read-only files, `tmp/` for in-flight puts on the same
//! filesystem, `pins/` for pinned manifests, and `by-exec/` for the
//! execution cache index.
//!
//! Every put runs the 4-step protocol of section 6.2: stream to a
//! unique temporary file while hashing, `sync_all` the file, set it
//! read-only and rename it into its sharded object path, then
//! `sync_all` the destination directory (and the `objects/` directory
//! when this put created the shard). Nothing crosses the store
//! boundary by hardlink, in either direction.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use verirun_core::cas::PutPhase;
use verirun_core::digest::Digest;

use crate::error::StoreError;
use crate::sha256::Sha256Factory;

/// Observes a put in progress. The fault suites use this seam to
/// record step order, chunk sizes, and to simulate a crash at a chosen
/// phase by panicking. Production callers use [`NoopObserver`] through
/// the plain put methods.
pub trait PutObserver {
    /// Called for each buffer of bytes streamed into the temp file,
    /// with the buffer length.
    fn on_chunk(&mut self, _byte_len: usize) {}

    /// Called as the put crosses each phase boundary, after the work
    /// of the phase has completed.
    fn on_phase(&mut self, _phase: PutPhase) {}
}

/// A [`PutObserver`] that records nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopObserver;

impl PutObserver for NoopObserver {}

/// An open content-addressed store. Cheap to share by reference:
/// concurrent puts through one `&Cas` are safe by the protocol design
/// (unique temp names, atomic renames, identical bytes per digest).
#[derive(Debug)]
pub struct Cas {
    root: PathBuf,
    factory: Sha256Factory,
}

impl Cas {
    /// Opens the store at `root`, creating the layout directories when
    /// absent. On Unix, refuses a root whose `tmp/` and `objects/`
    /// directories resolve to different devices.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Io`] when a layout directory cannot be
    /// created, and [`StoreError::CrossDevice`] when the same-device
    /// check fails.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// The store root.
    #[must_use]
    pub fn root(&self) -> &Path {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// The factory this store hashes with, for callers that need to
    /// compute framed digests against the same seam.
    #[must_use]
    pub const fn factory(&self) -> &Sha256Factory {
        &self.factory
    }

    /// The absolute object path for a digest, whether or not the
    /// object exists.
    #[must_use]
    pub fn object_path(&self, digest: &Digest) -> PathBuf {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// Whether an object exists under `digest`. A stat only: this
    /// carries no integrity claim.
    #[must_use]
    pub fn contains(&self, digest: &Digest) -> bool {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// Puts in-memory bytes, whose digest is computed before the temp
    /// file is named, through the full put protocol.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] on any protocol step failure. A failure
    /// leaves at most a temporary file behind.
    pub fn put_bytes(&self, bytes: &[u8]) -> Result<Digest, StoreError> {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// [`Cas::put_bytes`] with a [`PutObserver`] attached.
    ///
    /// # Errors
    ///
    /// Returns the [`StoreError`] of [`Cas::put_bytes`].
    pub fn put_bytes_observed(
        &self,
        bytes: &[u8],
        observer: &mut dyn PutObserver,
    ) -> Result<Digest, StoreError> {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// Streams a file into the store through the put protocol. The
    /// file is copied, never hardlinked, so later mutation of the
    /// source cannot change the stored blob.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the source cannot be read or any
    /// protocol step fails.
    pub fn put_file(&self, path: impl AsRef<Path>) -> Result<Digest, StoreError> {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// Streams a reader into the store. The digest is known only at
    /// end of stream, so the temp file carries a nonce name until the
    /// rename. Used for live pipe drains.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the reader fails or any protocol
    /// step fails.
    pub fn put_stream(&self, reader: impl Read) -> Result<Digest, StoreError> {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// [`Cas::put_stream`] with a [`PutObserver`] attached.
    ///
    /// # Errors
    ///
    /// Returns the [`StoreError`] of [`Cas::put_stream`].
    pub fn put_stream_observed(
        &self,
        reader: impl Read,
        observer: &mut dyn PutObserver,
    ) -> Result<Digest, StoreError> {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// Reads a whole object into memory, verifying its digest during
    /// the read.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Missing`] when no object exists under
    /// `digest`, and [`StoreError::Integrity`] when the content does
    /// not hash to `digest`.
    pub fn read_to_vec(&self, digest: &Digest) -> Result<Vec<u8>, StoreError> {
        unimplemented!("Stage A contract; implemented in Stage B")
    }

    /// Streams an object to `writer`, verifying its digest during the
    /// copy, and returns the number of bytes written.
    ///
    /// # Errors
    ///
    /// Returns the [`StoreError`] of [`Cas::read_to_vec`], plus
    /// [`StoreError::Io`] when the destination write fails.
    pub fn copy_to(&self, digest: &Digest, writer: impl Write) -> Result<u64, StoreError> {
        unimplemented!("Stage A contract; implemented in Stage B")
    }
}
