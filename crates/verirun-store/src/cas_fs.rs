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

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use verirun_core::cas::{
    PutPhase, STREAM_BUFFER_BYTES, object_relative_path, temp_file_name_known,
    temp_file_name_stream,
};
use verirun_core::digest::{Digest, DomainTag, HasherFactory, begin, hash_bytes};

use crate::error::{StoreError, StoreStep};
use crate::sha256::Sha256Factory;

/// This build's worker identity in temp file names. The scheduler of
/// Milestone 2 passes real worker ids through the observed variants'
/// internals; the public put methods all speak for worker 0.
const WORKER_ID: u32 = 0;

/// Process-wide counter making temp file names unique per attempt.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Observes a put in progress.
///
/// The fault suites use this seam to record step order, chunk sizes,
/// and to simulate a crash at a chosen phase by panicking. Production
/// callers use [`NoopObserver`] through the plain put methods.
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

fn io_error(path: &Path, step: StoreStep, source: std::io::Error) -> StoreError {
    StoreError::Io {
        path: path.to_path_buf(),
        step,
        source,
    }
}

/// Opens a directory read-only and syncs it, the POSIX durability
/// primitive for directory entries.
fn sync_directory(path: &Path, step: StoreStep) -> Result<(), StoreError> {
    let dir = File::open(path).map_err(|err| io_error(path, step, err))?;
    dir.sync_all().map_err(|err| io_error(path, step, err))
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
        let root = root.as_ref().to_path_buf();
        for layout in ["objects", "tmp", "pins", "by-exec"] {
            let dir = root.join(layout);
            std::fs::create_dir_all(&dir)
                .map_err(|err| io_error(&dir, StoreStep::CreateLayout, err))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let tmp_dir = root.join("tmp");
            let objects_dir = root.join("objects");
            let tmp_meta = std::fs::metadata(&tmp_dir)
                .map_err(|err| io_error(&tmp_dir, StoreStep::CreateLayout, err))?;
            let objects_meta = std::fs::metadata(&objects_dir)
                .map_err(|err| io_error(&objects_dir, StoreStep::CreateLayout, err))?;
            if tmp_meta.dev() != objects_meta.dev() {
                return Err(StoreError::CrossDevice {
                    root,
                    tmp_device: tmp_meta.dev(),
                    objects_device: objects_meta.dev(),
                });
            }
        }
        Ok(Self {
            root,
            factory: Sha256Factory,
        })
    }

    /// The store root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
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
        self.root.join(object_relative_path(digest))
    }

    /// Whether an object exists under `digest`. A stat only: this
    /// carries no integrity claim.
    #[must_use]
    pub fn contains(&self, digest: &Digest) -> bool {
        self.object_path(digest).is_file()
    }

    /// Puts in-memory bytes, whose digest is computed before the temp
    /// file is named, through the full put protocol.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] on any protocol step failure. A failure
    /// leaves at most a temporary file behind.
    pub fn put_bytes(&self, bytes: &[u8]) -> Result<Digest, StoreError> {
        self.put_bytes_observed(bytes, &mut NoopObserver)
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
        let digest = hash_bytes(&self.factory, DomainTag::Blob, bytes);
        let temp_path = self.fresh_temp_path(&temp_file_name_known(
            &digest,
            std::process::id(),
            WORKER_ID,
            TEMP_COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        let mut file = File::create_new(&temp_path)
            .map_err(|err| io_error(&temp_path, StoreStep::CreateTemp, err))?;
        for chunk in bytes.chunks(STREAM_BUFFER_BYTES) {
            file.write_all(chunk)
                .map_err(|err| io_error(&temp_path, StoreStep::Write, err))?;
            observer.on_chunk(chunk.len());
        }
        observer.on_phase(PutPhase::Writing);
        self.finish_put(file, &temp_path, &digest, observer)
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
        let path = path.as_ref();
        let file = File::open(path).map_err(|err| io_error(path, StoreStep::OpenSource, err))?;
        self.put_stream_observed(file, &mut NoopObserver)
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
        self.put_stream_observed(reader, &mut NoopObserver)
    }

    /// [`Cas::put_stream`] with a [`PutObserver`] attached.
    ///
    /// # Errors
    ///
    /// Returns the [`StoreError`] of [`Cas::put_stream`].
    pub fn put_stream_observed(
        &self,
        mut reader: impl Read,
        observer: &mut dyn PutObserver,
    ) -> Result<Digest, StoreError> {
        let temp_path = self.fresh_temp_path(&temp_file_name_stream(
            std::process::id(),
            WORKER_ID,
            TEMP_COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        let mut file = File::create_new(&temp_path)
            .map_err(|err| io_error(&temp_path, StoreStep::CreateTemp, err))?;
        let mut hasher = begin(&self.factory, DomainTag::Blob);
        let mut buffer = vec![0_u8; STREAM_BUFFER_BYTES];
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|err| io_error(&temp_path, StoreStep::Write, err))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            file.write_all(&buffer[..read])
                .map_err(|err| io_error(&temp_path, StoreStep::Write, err))?;
            observer.on_chunk(read);
        }
        observer.on_phase(PutPhase::Writing);
        let digest = hasher.finalize();
        self.finish_put(file, &temp_path, &digest, observer)
    }

    /// Steps 2 to 4 of the put protocol, shared by every put shape:
    /// sync the file, set it read-only, rename it into place (or
    /// discard the temp when the object is already present), then sync
    /// the destination directories.
    fn finish_put(
        &self,
        file: File,
        temp_path: &Path,
        digest: &Digest,
        observer: &mut dyn PutObserver,
    ) -> Result<Digest, StoreError> {
        file.sync_all()
            .map_err(|err| io_error(temp_path, StoreStep::SyncFile, err))?;
        observer.on_phase(PutPhase::FileSynced);
        let mut permissions = file
            .metadata()
            .map_err(|err| io_error(temp_path, StoreStep::SetReadOnly, err))?
            .permissions();
        permissions.set_readonly(true);
        file.set_permissions(permissions)
            .map_err(|err| io_error(temp_path, StoreStep::SetReadOnly, err))?;
        drop(file);

        let destination = self.object_path(digest);
        if destination.is_file() {
            // Already present from an earlier or concurrent put of the
            // same bytes. Discard the temp; the object and its
            // durability were established by the put that landed it.
            std::fs::remove_file(temp_path)
                .map_err(|err| io_error(temp_path, StoreStep::RemoveTemp, err))?;
            observer.on_phase(PutPhase::Renamed);
            observer.on_phase(PutPhase::Durable);
            return Ok(*digest);
        }
        let shard_dir = destination
            .parent()
            .expect("an object path always has a shard parent")
            .to_path_buf();
        let shard_created = !shard_dir.is_dir();
        if shard_created {
            std::fs::create_dir_all(&shard_dir)
                .map_err(|err| io_error(&shard_dir, StoreStep::CreateShard, err))?;
        }
        std::fs::rename(temp_path, &destination)
            .map_err(|err| io_error(temp_path, StoreStep::Rename, err))?;
        observer.on_phase(PutPhase::Renamed);
        sync_directory(&shard_dir, StoreStep::SyncShard)?;
        if shard_created {
            sync_directory(&self.root.join("objects"), StoreStep::SyncObjects)?;
        }
        observer.on_phase(PutPhase::Durable);
        Ok(*digest)
    }

    fn fresh_temp_path(&self, name: &str) -> PathBuf {
        self.root.join("tmp").join(name)
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
        let mut out = Vec::new();
        self.copy_to(digest, &mut out)?;
        Ok(out)
    }

    /// Streams an object to `writer`, verifying its digest during the
    /// copy, and returns the number of bytes written.
    ///
    /// # Errors
    ///
    /// Returns the [`StoreError`] of [`Cas::read_to_vec`], plus
    /// [`StoreError::Io`] when the destination write fails.
    pub fn copy_to(&self, digest: &Digest, mut writer: impl Write) -> Result<u64, StoreError> {
        let path = self.object_path(digest);
        let mut file = File::open(&path).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                StoreError::Missing {
                    digest: *digest,
                    path: path.clone(),
                }
            } else {
                io_error(&path, StoreStep::OpenObject, err)
            }
        })?;
        let mut hasher = begin(&self.factory, DomainTag::Blob);
        let mut buffer = vec![0_u8; STREAM_BUFFER_BYTES];
        let mut total: u64 = 0;
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|err| io_error(&path, StoreStep::ReadObject, err))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            writer
                .write_all(&buffer[..read])
                .map_err(|err| io_error(&path, StoreStep::WriteDestination, err))?;
            total += read as u64;
        }
        let actual = hasher.finalize();
        if actual != *digest {
            return Err(StoreError::Integrity {
                expected: *digest,
                actual,
                path,
            });
        }
        Ok(total)
    }
}
