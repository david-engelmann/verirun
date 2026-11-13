//! Hermetic subprocess execution (docs/ARCHITECTURE.md section 8).
//!
//! The runner owns everything about one task's process: building
//! its environment from nothing, materializing its inputs with
//! verification during the copy, resolving its executable,
//! spawning it with two concurrent drain threads that stream its
//! output into the store as raw bytes, enforcing its timeout, and
//! collecting exactly its declared outputs under the component
//! walk of section 8.4.
//!
//! Outcomes that belong to the task (a nonzero exit, death by
//! signal, a spawn failure, a missing or linked or non-regular
//! output) are reported in [`TaskExecution`], never as errors.
//! [`RunError`] is reserved for what aborts a run: store failures,
//! including the corrupted input blob that materialization finds
//! before the task starts (INV-SBX-5).

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use verirun_core::cas::STREAM_BUFFER_BYTES;
use verirun_core::dag::TaskSpec;
use verirun_core::digest::Digest;
use verirun_core::manifest::ArtifactRecord;
use verirun_core::paths::{NormalizedPath, validate_path};

use crate::cas_fs::Cas;
use crate::error::{RunError, StoreError, StoreStep};

/// How long output collection may run past a task's deadline
/// while the drain threads finish. After a group kill the pipes
/// close at once and the drains report in milliseconds; the
/// grace exists only for a kill that somehow missed a
/// pipe-holding grandchild. When it expires, the affected
/// stream is cut: the drain finalizes the store object from the
/// bytes delivered by then, which are an exact prefix of the
/// stream, and the deadline stays a hard bound on the task's
/// wall time.
const DRAIN_GRACE: Duration = Duration::from_secs(10);

/// The fixed default environment pairs of section 8.1, sorted by
/// key.
///
/// A spec that declares any of these keys is rejected at
/// validation (INV-DAG-6); the runner sets them unconditionally,
/// because fixed means fixed (INV-RUN-2).
pub const FIXED_ENV: [(&str, &str); 3] =
    [("LC_ALL", "C"), ("SOURCE_DATE_EPOCH", "0"), ("TZ", "UTC")];

/// The raw outcome of one task's execution, before the scheduler
/// assigns a status.
///
/// `exit_code` is absent when the child died by signal, when it
/// was killed at its deadline, and when it never spawned.
/// `artifacts` is empty unless the child exited with code 0 and
/// collection succeeded; a task that fails keeps none of its
/// outputs. `failure_detail` explains a task-level failure that
/// the exit code alone cannot: an unresolvable executable, a
/// spawn failure, a missing or linked or non-regular output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskExecution {
    /// The child exit code, when the child exited with one.
    pub exit_code: Option<i32>,
    /// Whether the deadline fired and the child was killed.
    pub timed_out: bool,
    /// The blob digest of the captured stdout stream. The empty
    /// blob digest when the child never spawned.
    pub stdout_digest: Digest,
    /// The blob digest of the captured stderr stream.
    pub stderr_digest: Digest,
    /// The collected artifacts, sorted by path.
    pub artifacts: Vec<ArtifactRecord>,
    /// A human-readable account of a task-level failure.
    pub failure_detail: Option<String>,
}

/// The executor for one store. Holds the `PATH` snapshot taken at
/// construction, which bare executable names resolve against
/// (section 8.2). The snapshot is a host detail and enters no
/// digest.
pub struct Runner<'a> {
    cas: &'a Cas,
    path_dirs: Vec<PathBuf>,
}

impl<'a> Runner<'a> {
    /// Creates a runner over `cas`, snapshotting the current
    /// process `PATH` for bare name resolution.
    #[must_use]
    pub fn new(cas: &'a Cas) -> Self {
        let path_dirs = std::env::var_os("PATH")
            .map(|value| std::env::split_paths(&value).collect())
            .unwrap_or_default();
        Self { cas, path_dirs }
    }

    /// Builds the child's full environment, sorted by key: the
    /// declared pairs, the fixed defaults of [`FIXED_ENV`], and
    /// `TMPDIR` naming the task's private temporary directory
    /// inside its sandbox. Nothing else is present (INV-RUN-1).
    /// `TMPDIR` is host-specific, so it is excluded from the
    /// environment digest (section 5.3); this list is the spawn
    /// environment, not the digest input.
    ///
    /// # Panics
    ///
    /// Never panics in the store implementation.
    #[must_use]
    pub fn build_environment(
        declared: &BTreeMap<String, String>,
        tmpdir: &Path,
    ) -> Vec<(String, String)> {
        let mut env: BTreeMap<String, String> = BTreeMap::new();
        for (key, value) in declared {
            env.insert(key.clone(), value.clone());
        }
        for (key, value) in FIXED_ENV {
            env.insert((*key).to_string(), (*value).to_string());
        }
        env.insert("TMPDIR".to_string(), tmpdir.to_string_lossy().into_owned());
        env.into_iter().collect()
    }

    /// Resolves the executable string of section 8.2. A string
    /// containing a path separator is interpreted relative to
    /// `sandbox` and must pass the path rules of section 8.4. A
    /// bare name is resolved against the `PATH` snapshot.
    ///
    /// # Errors
    ///
    /// Returns a failure detail string when the string names no
    /// executable file: resolution failure is a task outcome, so
    /// it is reported as text for [`TaskExecution::failure_detail`],
    /// not as a [`RunError`].
    ///
    /// # Panics
    ///
    /// Never panics in the store implementation.
    pub fn resolve_executable(&self, executable: &str, sandbox: &Path) -> Result<PathBuf, String> {
        if executable.contains('/') || executable.contains('\\') {
            let normalized = validate_path(executable).map_err(|source| {
                format!("executable {executable:?} is not a valid sandbox-relative path: {source}")
            })?;
            let candidate = sandbox.join(normalized.as_str());
            if is_executable_file(&candidate) {
                Ok(candidate)
            } else {
                Err(format!(
                    "executable {executable:?} names no executable file inside the sandbox"
                ))
            }
        } else {
            for dir in &self.path_dirs {
                let candidate = dir.join(executable);
                if is_executable_file(&candidate) {
                    return Ok(candidate);
                }
            }
            Err(format!(
                "executable {executable:?} was not found on the runner's PATH snapshot"
            ))
        }
    }

    /// Executes one task in `sandbox`, creating the sandbox and
    /// its `inputs`, `outputs`, and `tmp` areas.
    ///
    /// `inputs` pairs each normalized declared input path with the
    /// blob digest to materialize. Materialization copies through
    /// the store's verified read, walking every destination
    /// component for symlinks before writing any bytes
    /// (section 8.4 rule 5).
    ///
    /// # Errors
    ///
    /// Returns [`RunError`] when the store fails, including the
    /// integrity failure a corrupted input blob raises; the task
    /// is then never spawned and the whole run aborts
    /// (INV-SBX-5). Task-level failures are returned inside
    /// [`TaskExecution`], not as errors.
    ///
    /// # Panics
    ///
    /// Never panics in the store implementation.
    pub fn execute(
        &self,
        task: &TaskSpec,
        inputs: &[(NormalizedPath, Digest)],
        sandbox: &Path,
    ) -> Result<TaskExecution, RunError> {
        let inputs_dir = sandbox.join("inputs");
        let outputs_dir = sandbox.join("outputs");
        let tmp_dir = sandbox.join("tmp");
        for dir in [sandbox, &inputs_dir, &outputs_dir, &tmp_dir] {
            if let Err(source) = fs::create_dir_all(dir) {
                return self.unspawned(format!(
                    "creating sandbox directory {} failed: {source}",
                    dir.display()
                ));
            }
        }

        // Materialize inputs through the store's verified read
        // before anything spawns. A corrupted blob aborts the run
        // from here (INV-SBX-5); a sandbox-side path problem fails
        // only this task.
        for (path, digest) in inputs {
            match self.materialize(&inputs_dir, path, digest) {
                Ok(()) => {}
                Err(PrepFailure::Run(error)) => return Err(error),
                Err(PrepFailure::Task(detail)) => return self.unspawned(detail),
            }
        }

        let resolved = match self.resolve_executable(&task.executable, sandbox) {
            Ok(path) => path,
            Err(detail) => return self.unspawned(detail),
        };

        let environment = Self::build_environment(&task.env, &tmp_dir);
        let mut command = Command::new(&resolved);
        // The child leads its own process group, so the deadline
        // can kill the whole tree: a child that forks (the shell
        // running a sleeper, a build spawning a daemon) would
        // otherwise leave grandchildren holding the output pipes
        // open long past the kill, and the drains could not
        // finish (INV-RUN-4).
        command.process_group(0);
        command
            .args(&task.args)
            .env_clear()
            .envs(environment)
            .current_dir(sandbox)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(source) => {
                return self.unspawned(format!("spawning {} failed: {source}", resolved.display()));
            }
        };
        let stdout_pipe = child.stdout.take().expect("stdout was piped");
        let stderr_pipe = child.stderr.take().expect("stderr was piped");

        // Each pipe is owned by a detached reader thread that
        // forwards chunks over a channel; a scoped drain thread
        // streams each channel into the store through
        // `put_stream` while this thread supervises the child.
        // Neither stream is ever drained sequentially, so a child
        // cannot deadlock on a full pipe (INV-RUN-3). The channel
        // seam is also what bounds a timed-out task whose pipes
        // a grandchild still holds: the drain's reader can be
        // told to report end of stream at the grace bound, which
        // finalizes the store object from the bytes delivered by
        // then (see `collect_stream`).
        let (stdout_chunks, stdout_cutoff) = spawn_pipe_reader(stdout_pipe);
        let (stderr_chunks, stderr_cutoff) = spawn_pipe_reader(stderr_pipe);
        let (stdout_report_tx, stdout_report_rx) = mpsc::channel();
        let (stderr_report_tx, stderr_report_rx) = mpsc::channel();
        let cas = self.cas;
        let stdout_reader_cutoff = stdout_cutoff.clone();
        let stderr_reader_cutoff = stderr_cutoff.clone();
        let (supervision, stdout_collected, stderr_collected) = std::thread::scope(|scope| {
            scope.spawn(move || {
                let reader = ChannelReader::new(stdout_chunks, stdout_reader_cutoff);
                let _ = stdout_report_tx.send(cas.put_stream(reader));
            });
            scope.spawn(move || {
                let reader = ChannelReader::new(stderr_chunks, stderr_reader_cutoff);
                let _ = stderr_report_tx.send(cas.put_stream(reader));
            });
            let supervision = supervise(&mut child, task.timeout_ms);
            let stdout_collected = collect_stream(
                &stdout_report_rx,
                supervision.timed_out,
                &stdout_cutoff,
                sandbox,
            );
            let stderr_collected = collect_stream(
                &stderr_report_rx,
                supervision.timed_out,
                &stderr_cutoff,
                sandbox,
            );
            (supervision, stdout_collected, stderr_collected)
        });
        let (stdout_digest, stdout_cut) = stdout_collected?;
        let (stderr_digest, stderr_cut) = stderr_collected?;
        let mut detail = supervision.detail;
        if stdout_cut || stderr_cut {
            let note = "a stream was cut at the drain grace after the deadline; \
                        its digest covers the bytes delivered by then";
            detail = Some(detail.map_or_else(
                || note.to_string(),
                |existing| format!("{existing}; {note}"),
            ));
        }

        let mut execution = TaskExecution {
            exit_code: supervision.exit_code,
            timed_out: supervision.timed_out,
            stdout_digest,
            stderr_digest,
            artifacts: Vec::new(),
            failure_detail: detail,
        };
        if supervision.exit_code == Some(0) && !supervision.timed_out {
            match self.collect(task, &outputs_dir) {
                Ok(artifacts) => execution.artifacts = artifacts,
                Err(PrepFailure::Run(error)) => return Err(error),
                Err(PrepFailure::Task(detail)) => execution.failure_detail = Some(detail),
            }
        }
        Ok(execution)
    }

    /// The execution record of a task that never spawned: no exit
    /// code, both streams the empty blob, no artifacts, and the
    /// reason in `failure_detail`.
    fn unspawned(&self, detail: String) -> Result<TaskExecution, RunError> {
        let empty = self.cas.put_bytes(b"")?;
        Ok(TaskExecution {
            exit_code: None,
            timed_out: false,
            stdout_digest: empty,
            stderr_digest: empty,
            artifacts: Vec::new(),
            failure_detail: Some(detail),
        })
    }

    /// Copies one input blob into the inputs area through the
    /// store's verified read, after walking the destination
    /// components under section 8.4 rule 5.
    fn materialize(
        &self,
        inputs_dir: &Path,
        path: &NormalizedPath,
        digest: &Digest,
    ) -> Result<(), PrepFailure> {
        create_parent_dirs(inputs_dir, path)?;
        let destination = inputs_dir.join(path.as_str());
        if let Ok(metadata) = fs::symlink_metadata(&destination)
            && metadata.file_type().is_symlink()
        {
            return Err(PrepFailure::Task(format!(
                "input destination {} is a symbolic link",
                destination.display()
            )));
        }
        let mut file = fs::File::create(&destination).map_err(|source| {
            PrepFailure::Task(format!(
                "creating input file {} failed: {source}",
                destination.display()
            ))
        })?;
        self.cas
            .copy_to(digest, &mut file)
            .map_err(|source| PrepFailure::Run(RunError::from(source)))?;
        Ok(())
    }

    /// Collects exactly the declared outputs of a task that exited
    /// with code 0, in normalized path order, under the component
    /// walk of section 8.4 rules 1 to 4.
    fn collect(
        &self,
        task: &TaskSpec,
        outputs_dir: &Path,
    ) -> Result<Vec<ArtifactRecord>, PrepFailure> {
        let mut declared: Vec<NormalizedPath> = Vec::with_capacity(task.outputs.len());
        for output in &task.outputs {
            let normalized = validate_path(output).map_err(|source| {
                PrepFailure::Task(format!(
                    "declared output {output:?} fails the path rules: {source}"
                ))
            })?;
            declared.push(normalized);
        }
        declared.sort();
        declared.dedup();

        let mut artifacts = Vec::with_capacity(declared.len());
        for output in &declared {
            let candidate = outputs_dir.join(output.as_str());
            let metadata = walk_output(outputs_dir, output)?;
            let digest = self
                .cas
                .put_file(&candidate)
                .map_err(|source| PrepFailure::Run(RunError::from(source)))?;
            artifacts.push(ArtifactRecord {
                path: output.to_string(),
                digest,
                bytes: metadata.len(),
                executable: metadata.permissions().mode() & 0o111 != 0,
            });
        }
        Ok(artifacts)
    }
}

/// Spawns the detached reader thread for one pipe. The thread
/// forwards the pipe's bytes over a channel in read-sized chunks
/// and exits at end of stream, on a read error, or when the
/// receiving side is gone (which is how a cut stream releases
/// it). The returned flag is the stream's cutoff: setting it
/// makes the drain side report end of stream at the next read,
/// finalizing the store object from the bytes delivered by
/// then.
fn spawn_pipe_reader(
    mut pipe: impl Read + Send + 'static,
) -> (mpsc::Receiver<Vec<u8>>, std::sync::Arc<AtomicBool>) {
    let (sender, receiver) = mpsc::channel();
    let cutoff = std::sync::Arc::new(AtomicBool::new(false));
    std::thread::spawn(move || {
        let mut buffer = vec![0_u8; STREAM_BUFFER_BYTES];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    if sender.send(buffer[..read].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    (receiver, cutoff)
}

/// A [`Read`] over a pipe reader's channel, for `put_stream`.
///
/// Reads block while the stream is live. End of stream is
/// reported when the reader thread ends (the pipe closed) or
/// when the cutoff flag is set, whichever comes first; bytes
/// already delivered are always delivered first, so a cut
/// stream is an exact prefix of the full stream.
struct ChannelReader {
    /// The chunk channel from the pipe reader thread.
    receiver: mpsc::Receiver<Vec<u8>>,
    /// The chunk currently being served.
    pending: Vec<u8>,
    /// How much of `pending` has been served.
    offset: usize,
    /// The cutoff flag shared with `collect_stream`.
    cutoff: std::sync::Arc<AtomicBool>,
}

impl ChannelReader {
    /// Creates a reader over `receiver`, cut short when
    /// `cutoff` is set.
    const fn new(receiver: mpsc::Receiver<Vec<u8>>, cutoff: std::sync::Arc<AtomicBool>) -> Self {
        Self {
            receiver,
            pending: Vec::new(),
            offset: 0,
            cutoff,
        }
    }
}

impl Read for ChannelReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.offset < self.pending.len() {
                let served = (self.pending.len() - self.offset).min(out.len());
                out[..served].copy_from_slice(&self.pending[self.offset..self.offset + served]);
                self.offset += served;
                if self.offset == self.pending.len() {
                    self.pending = Vec::new();
                    self.offset = 0;
                }
                return Ok(served);
            }
            if self.cutoff.load(Ordering::SeqCst) {
                return Ok(0);
            }
            match self.receiver.recv_timeout(Duration::from_millis(5)) {
                Ok(chunk) => {
                    self.pending = chunk;
                    self.offset = 0;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(0),
            }
        }
    }
}

/// Collects one drain's report: the digest of the stream it
/// stored, and whether the stream was cut.
///
/// After a natural exit the drain runs to end of stream, however
/// long a pipe-holding grandchild keeps it open (section 8.3).
/// After a timeout the deadline is a hard bound, so collection
/// waits only [`DRAIN_GRACE`] for the report; on expiry it sets
/// the stream's cutoff, the drain finalizes the object from the
/// bytes delivered by then, and that digest is collected as a
/// cut stream.
///
/// # Errors
///
/// Returns [`RunError`] when the drain's store operation failed,
/// or when the drain thread died without reporting.
fn collect_stream(
    reports: &mpsc::Receiver<Result<Digest, StoreError>>,
    timed_out: bool,
    cutoff: &AtomicBool,
    sandbox: &Path,
) -> Result<(Digest, bool), RunError> {
    let (received, cut) = if timed_out {
        match reports.recv_timeout(DRAIN_GRACE) {
            Ok(result) => (result, false),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                cutoff.store(true, Ordering::SeqCst);
                match reports.recv() {
                    Ok(result) => (result, true),
                    Err(_) => return Err(drain_died(sandbox)),
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(drain_died(sandbox)),
        }
    } else {
        match reports.recv() {
            Ok(result) => (result, false),
            Err(_) => return Err(drain_died(sandbox)),
        }
    };
    match received {
        Ok(digest) => Ok((digest, cut)),
        Err(source) => Err(RunError::from(source)),
    }
}

/// The run failure for a drain thread that died without
/// reporting its stream's outcome.
fn drain_died(sandbox: &Path) -> RunError {
    RunError::from(StoreError::Io {
        path: sandbox.to_path_buf(),
        step: StoreStep::ReadObject,
        source: io::Error::other("an output drain thread failed"),
    })
}

/// Why preparing or collecting for a task stopped. `Task` fails
/// the one task and is reported in its `failure_detail`; `Run`
/// aborts the whole run.
enum PrepFailure {
    /// A task-level failure, with its detail text.
    Task(String),
    /// A run-level failure.
    Run(RunError),
}

/// Whether `path` names a regular file with at least one
/// executable permission bit, following links the way the host's
/// own command resolution does.
fn is_executable_file(path: &Path) -> bool {
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// Creates the parent directories of `path` below `area`, walking
/// each existing component: a symbolic link or a non-directory
/// component is a task-level failure (section 8.4 rule 5).
fn create_parent_dirs(area: &Path, path: &NormalizedPath) -> Result<(), PrepFailure> {
    let components: Vec<&str> = path.as_str().split('/').collect();
    let mut current = area.to_path_buf();
    for component in &components[..components.len() - 1] {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(PrepFailure::Task(format!(
                        "input path component {} is a symbolic link",
                        current.display()
                    )));
                }
                if !metadata.is_dir() {
                    return Err(PrepFailure::Task(format!(
                        "input path component {} is not a directory",
                        current.display()
                    )));
                }
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|source| {
                    PrepFailure::Task(format!(
                        "creating input directory {} failed: {source}",
                        current.display()
                    ))
                })?;
            }
            Err(source) => {
                return Err(PrepFailure::Task(format!(
                    "inspecting input path component {} failed: {source}",
                    current.display()
                )));
            }
        }
    }
    Ok(())
}

/// Walks the components of `declared` below the outputs area
/// under section 8.4: every component is inspected with
/// `symlink_metadata`, any symbolic link component fails
/// regardless of its target, and the final component must be a
/// regular file. Returns the final component's metadata. The
/// area directory itself is runner-created and trusted.
fn walk_output(area: &Path, declared: &NormalizedPath) -> Result<fs::Metadata, PrepFailure> {
    let mut current = area.to_path_buf();
    let mut metadata = None;
    for component in declared.as_str().split('/') {
        current.push(component);
        let step = fs::symlink_metadata(&current).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                PrepFailure::Task(format!(
                    "declared output {declared} is missing from the sandbox"
                ))
            } else {
                PrepFailure::Task(format!(
                    "inspecting declared output {declared} failed: {source}"
                ))
            }
        })?;
        if step.file_type().is_symlink() {
            return Err(PrepFailure::Task(format!(
                "declared output {declared} passes through a symbolic link at {}",
                current.display()
            )));
        }
        metadata = Some(step);
    }
    match metadata {
        Some(metadata) if metadata.is_file() => Ok(metadata),
        _ => Err(PrepFailure::Task(format!(
            "declared output {declared} is not a regular file"
        ))),
    }
}

/// The supervision result for one spawned child.
struct Supervision {
    /// The exit code, when the child exited with one.
    exit_code: Option<i32>,
    /// Whether the deadline fired and the child was killed.
    timed_out: bool,
    /// A failure the exit code cannot express: death by signal,
    /// or a failed wait.
    detail: Option<String>,
}

/// Watches `child` until it exits or its deadline passes.
///
/// On the deadline the child's whole process group is killed
/// (the child was spawned as its group leader) and the child is
/// reaped through `wait`; the caller then joins the drain
/// threads, which finish when the pipes close (section 8.3).
fn supervise(child: &mut Child, timeout_ms: Option<u64>) -> Supervision {
    let deadline = timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms));
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Supervision {
                    exit_code: status.code(),
                    timed_out: false,
                    detail: status
                        .signal()
                        .map(|signal| format!("the child was terminated by signal {signal}")),
                };
            }
            Ok(None) => {
                if let Some(deadline) = deadline
                    && Instant::now() >= deadline
                {
                    kill_process_group(child.id());
                    let _ = child.kill();
                    return match child.wait() {
                        Ok(_) => Supervision {
                            exit_code: None,
                            timed_out: true,
                            detail: None,
                        },
                        Err(source) => Supervision {
                            exit_code: None,
                            timed_out: true,
                            detail: Some(format!("reaping the killed child failed: {source}")),
                        },
                    };
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(source) => {
                return Supervision {
                    exit_code: None,
                    timed_out: false,
                    detail: Some(format!("waiting on the child failed: {source}")),
                };
            }
        }
    }
}

/// Kills every process in the group led by `leader`, through
/// the shell's builtin `kill`: this crate forbids unsafe code,
/// so the group signal cannot be sent through a syscall binding.
/// (The standalone `kill` utility's negative-pid form was
/// observed to be a silent no-op in the development sandbox;
/// the builtin reaches the syscall directly.) A grandchild that
/// escaped the group by creating its own session is beyond this
/// kill; the drain grace in `collect_stream` bounds what that
/// can cost. Falls back silently to the caller's direct kill
/// when the shell is unavailable.
fn kill_process_group(leader: u32) {
    let script = format!("kill -KILL -{leader}");
    let _ = Command::new("/bin/sh")
        .args(["-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}
