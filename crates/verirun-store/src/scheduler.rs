//! The worker pool scheduler (docs/ARCHITECTURE.md section 7.3).
//!
//! The scheduler executes a [`ValidatedDag`] over a fixed pool of
//! workers. A task becomes ready when all of its dependencies have
//! status Succeeded or Cached; the ready set is ordered by task id
//! and workers always take the lexicographically smallest ready
//! task, so start order is deterministic even though finish order
//! is not. Outcomes land in a map keyed by task id and the
//! manifest is built from that map in key order, so finish order
//! has no path into the manifest (INV-RUN-5).
//!
//! Input binding: a task's declared input resolves to the artifact
//! a dependency produced under the same normalized path, or to the
//! caller's binding in `inputs` when no task in the run produces
//! the path. An input that resolves nowhere aborts the run with
//! [`RunError::MissingInput`] before any task executes.
//!
//! Failure propagation: when a task ends `Failed` or `TimedOut`,
//! every transitive dependent is recorded `Skipped` without
//! starting. A skipped record still carries its execution digest:
//! inputs that resolved (from a succeeded producer or a caller
//! binding) frame normally, and an input whose producer failed or
//! was skipped has no content, so it frames with the blob digest
//! of the empty byte string, the same digest section 3.3 assigns
//! to a skipped task's own streams. Recording a skipped task puts
//! the empty blob into the store through the normal put protocol
//! first, so every digest a skipped record names exists in the
//! store the run produced (INV-RUN-6).
//!
//! Cache: before executing a ready task the scheduler looks its
//! execution digest up in the store's `by-exec/` index. The entry
//! is reused only when the recorded task record and every blob it
//! names still exist and re-hash correctly; a missing or corrupt
//! target is ignored and the task executes (INV-RUN-7). After a
//! task succeeds, its execution digest to record digest mapping
//! is written with the same atomic discipline as the put
//! protocol.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, mpsc};

use verirun_core::canonical;
use verirun_core::cas::by_exec_relative_path;
use verirun_core::dag::ValidatedDag;
use verirun_core::digest::{self, Digest, HasherFactory, TaskStatus};
use verirun_core::manifest::{Manifest, TaskRecord};
use verirun_core::paths::NormalizedPath;

use crate::cas_fs::Cas;
use crate::error::{RunError, StoreError, StoreStep};
use crate::runner::{FIXED_ENV, Runner};

/// The result of one completed run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    /// The run's manifest: one record per spec task, sorted by
    /// task id.
    pub manifest: Manifest,
    /// The manifest digest: the blob digest of the manifest's
    /// canonical bytes, under which the manifest itself was
    /// stored.
    pub manifest_digest: Digest,
    /// The run's exit code (section 12.2): 0 when every task
    /// succeeded or was cached, 1 when any task failed, timed
    /// out, or was skipped through propagation.
    pub exit_code: i32,
    /// Per-task failure details for tasks whose records show a
    /// failure the exit code cannot explain (spawn failures,
    /// collection failures). Diagnostics only; never digested.
    pub failures: BTreeMap<String, String>,
}

/// Executes one validated run to completion.
///
/// `inputs` binds normalized input paths that no task in the run
/// produces to the blob digests to materialize. `jobs` sizes the
/// worker pool; 0 selects the available parallelism. Worker count
/// is an execution detail only: it appears in no digest and no
/// record, and the manifest is byte-identical for any count
/// (INV-RUN-5).
///
/// The manifest blob is stored in `cas` before this returns, and
/// every blob any record names is present in `cas`.
///
/// # Errors
///
/// Returns [`RunError::MissingInput`] before any task executes
/// when a declared input resolves to no producer and no caller
/// binding. Returns [`RunError::Store`] when the store fails,
/// including the integrity failure a corrupted input blob raises
/// during materialization, which aborts the run before the
/// consuming task starts.
///
/// # Panics
///
/// Never panics in the store implementation.
pub fn run(
    cas: &Cas,
    factory: &dyn HasherFactory,
    work_root: &Path,
    dag: &ValidatedDag,
    inputs: &BTreeMap<String, Digest>,
    jobs: usize,
) -> Result<RunOutcome, RunError> {
    let plans = plan_inputs(dag, inputs)?;

    if dag.tasks.is_empty() {
        let manifest = Manifest::build(factory, Vec::new());
        let manifest_digest = store_manifest(cas, &manifest)?;
        return Ok(RunOutcome {
            manifest,
            manifest_digest,
            exit_code: 0,
            failures: BTreeMap::new(),
        });
    }

    let shared = Shared {
        state: Mutex::new(SharedState {
            ready: BTreeMap::new(),
            shutdown: false,
        }),
        available: Condvar::new(),
    };
    let (report_tx, report_rx) = mpsc::channel::<WorkerReport>();

    // Seed the ready set with the dependency-free tasks. Their
    // inputs are all caller bindings, so no records are needed.
    let mut pending: BTreeMap<String, usize> = dag
        .dependencies
        .iter()
        .map(|(task_id, deps)| (task_id.clone(), deps.len()))
        .collect();
    let mut dispatched: usize = 0;
    {
        let no_records: BTreeMap<String, TaskRecord> = BTreeMap::new();
        let mut state = shared.state.lock().expect("scheduler state lock");
        for (task_id, count) in &pending {
            if *count == 0 {
                let item = build_work_item(factory, dag, &plans, &no_records, task_id);
                state.ready.insert(task_id.clone(), item);
                dispatched += 1;
            }
        }
        drop(state);
    }

    let worker_count = worker_count(jobs, dag.tasks.len());
    let mut coordinator = Coordinator {
        records: BTreeMap::new(),
        failures: BTreeMap::new(),
        doomed: BTreeSet::new(),
        first_error: None,
        dispatched,
        reported: 0,
    };
    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            let worker_tx = report_tx.clone();
            let worker_shared = &shared;
            scope.spawn(move || worker_loop(cas, work_root, dag, worker_shared, &worker_tx));
        }
        drop(report_tx);
        coordinator.run_to_completion(factory, dag, &plans, &shared, &report_rx, &mut pending);
        // No more work is admitted; workers exit after their
        // current task, and the scope joins them.
        let mut state = shared.state.lock().expect("scheduler state lock");
        state.shutdown = true;
        state.ready.clear();
        drop(state);
        shared.available.notify_all();
    });

    if let Some(error) = coordinator.first_error {
        return Err(error);
    }

    // Skipped records are built last, when every producer's final
    // status is known, so their execution digests frame resolved
    // inputs exactly as an executed task's would (INV-RUN-5).
    if !coordinator.doomed.is_empty() {
        let empty = cas.put_bytes(b"")?;
        for task_id in &coordinator.doomed {
            let record = skipped_record(factory, dag, &plans, &coordinator.records, task_id, empty);
            coordinator.records.insert(task_id.clone(), record);
        }
    }

    let records: Vec<TaskRecord> = coordinator.records.into_values().collect();
    let exit_code = i32::from(records.iter().any(|record| {
        matches!(
            record.status,
            TaskStatus::Failed | TaskStatus::TimedOut | TaskStatus::Skipped
        )
    }));
    let manifest = Manifest::build(factory, records);
    let manifest_digest = store_manifest(cas, &manifest)?;
    Ok(RunOutcome {
        manifest,
        manifest_digest,
        exit_code,
        failures: coordinator.failures,
    })
}

/// Stores the manifest's canonical bytes and returns the digest
/// they were stored under.
fn store_manifest(cas: &Cas, manifest: &Manifest) -> Result<Digest, RunError> {
    let bytes = manifest
        .canonical_bytes()
        .map_err(|source| canonical_failure(cas.root(), &source))?;
    Ok(cas.put_bytes(&bytes)?)
}

/// The canonical encoding of a constructed manifest or record
/// cannot fail: every field is in the canonical domain by
/// construction. If that invariant is ever broken, the failure
/// surfaces as a store write failure rather than a panic.
fn canonical_failure(root: &Path, source: &verirun_core::error::CanonicalError) -> RunError {
    RunError::from(StoreError::Io {
        path: root.to_path_buf(),
        step: StoreStep::Write,
        source: io::Error::other(format!("canonical encoding failed: {source}")),
    })
}

/// How one declared input of a task is satisfied.
#[derive(Debug, Clone)]
enum InputSource {
    /// Produced by another task in the run, which is a transitive
    /// dependency of the consumer, so the producer completes
    /// before the consumer is scheduled.
    ProducedBy(String),
    /// Bound by the caller to a fixed blob digest.
    Bound(Digest),
}

/// Resolves every declared input of every task, before anything
/// executes. An input resolves to the artifact of a producing
/// task only when that producer is a transitive dependency;
/// otherwise it resolves to the caller's binding. Anything else
/// aborts the run with [`RunError::MissingInput`].
fn plan_inputs(
    dag: &ValidatedDag,
    inputs: &BTreeMap<String, Digest>,
) -> Result<BTreeMap<String, Vec<(NormalizedPath, InputSource)>>, RunError> {
    let mut plans = BTreeMap::new();
    for (task_id, paths) in &dag.inputs {
        let mut plan = Vec::with_capacity(paths.len());
        for path in paths {
            let source = match dag.producer_of(path) {
                Some(producer) if is_transitive_dependency(dag, task_id, producer) => {
                    InputSource::ProducedBy(producer.to_string())
                }
                _ => match inputs.get(path.as_str()) {
                    Some(digest) => InputSource::Bound(*digest),
                    None => {
                        return Err(RunError::MissingInput {
                            task_id: task_id.clone(),
                            path: path.to_string(),
                        });
                    }
                },
            };
            plan.push((path.clone(), source));
        }
        plans.insert(task_id.clone(), plan);
    }
    Ok(plans)
}

/// Whether `producer` is reachable from `task_id` along dependency
/// edges, at any depth.
fn is_transitive_dependency(dag: &ValidatedDag, task_id: &str, producer: &str) -> bool {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut stack: Vec<&str> = dag.dependencies[task_id]
        .iter()
        .map(String::as_str)
        .collect();
    while let Some(current) = stack.pop() {
        if current == producer {
            return true;
        }
        if seen.insert(current) {
            stack.extend(dag.dependencies[current].iter().map(String::as_str));
        }
    }
    false
}

/// Every task that transitively depends on `task_id`, sorted.
fn transitive_dependents(dag: &ValidatedDag, task_id: &str) -> BTreeSet<String> {
    let mut found: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<&str> = dag.dependents.get(task_id).map_or_else(Vec::new, |direct| {
        direct.iter().map(String::as_str).collect()
    });
    while let Some(current) = stack.pop() {
        if found.insert(current.to_string()) {
            if let Some(next) = dag.dependents.get(current) {
                stack.extend(next.iter().map(String::as_str));
            }
        }
    }
    found
}

/// The execution digest of one task (section 5.3): the
/// environment digest over the declared pairs plus the fixed
/// defaults, and the framed input digests in declared order
/// (framing sorts internally).
fn compute_exec_digest(
    factory: &dyn HasherFactory,
    dag: &ValidatedDag,
    task_id: &str,
    framed_inputs: &[(String, Digest)],
) -> Digest {
    let spec = &dag.tasks[task_id];
    let env_pairs: Vec<(String, String)> = spec
        .env
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .chain(
            FIXED_ENV
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string())),
        )
        .collect();
    let env = digest::env_digest(factory, &env_pairs);
    let outputs: Vec<String> = dag.outputs[task_id]
        .iter()
        .map(ToString::to_string)
        .collect();
    digest::exec_digest(
        factory,
        &spec.executable,
        &spec.args,
        &env,
        framed_inputs,
        &outputs,
    )
}

/// Builds the work item for a task whose dependencies all
/// succeeded: the materialization inputs resolved from producer
/// records or caller bindings, and its execution digest.
fn build_work_item(
    factory: &dyn HasherFactory,
    dag: &ValidatedDag,
    plans: &BTreeMap<String, Vec<(NormalizedPath, InputSource)>>,
    records: &BTreeMap<String, TaskRecord>,
    task_id: &str,
) -> WorkItem {
    let mut materialize: Vec<(NormalizedPath, Digest)> = Vec::new();
    let mut framed: Vec<(String, Digest)> = Vec::new();
    for (path, source) in &plans[task_id] {
        let content = match source {
            InputSource::Bound(digest) => *digest,
            InputSource::ProducedBy(producer) => {
                records[producer]
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.path == path.as_str())
                    .expect("a succeeded producer's record holds its declared outputs")
                    .digest
            }
        };
        materialize.push((path.clone(), content));
        framed.push((
            path.to_string(),
            digest::input_digest(factory, path.as_str(), &content),
        ));
    }
    WorkItem {
        task_id: task_id.to_string(),
        exec_digest: compute_exec_digest(factory, dag, task_id, &framed),
        inputs: materialize,
    }
}

/// Builds the record of a task that never started because a
/// transitive dependency failed. Inputs produced by a task that
/// ultimately succeeded or was cached frame with its artifact
/// digest; inputs whose producer failed, timed out, or was itself
/// skipped frame with the digest of the empty byte string,
/// matching the digest the record assigns to its own streams
/// (section 3.3).
fn skipped_record(
    factory: &dyn HasherFactory,
    dag: &ValidatedDag,
    plans: &BTreeMap<String, Vec<(NormalizedPath, InputSource)>>,
    records: &BTreeMap<String, TaskRecord>,
    task_id: &str,
    empty: Digest,
) -> TaskRecord {
    let mut framed: Vec<(String, Digest)> = Vec::new();
    for (path, source) in &plans[task_id] {
        let content = match source {
            InputSource::Bound(digest) => *digest,
            InputSource::ProducedBy(producer) => {
                let producer_record = &records[producer];
                if matches!(
                    producer_record.status,
                    TaskStatus::Succeeded | TaskStatus::Cached
                ) {
                    producer_record
                        .artifacts
                        .iter()
                        .find(|artifact| artifact.path == path.as_str())
                        .expect("a succeeded producer's record holds its declared outputs")
                        .digest
                } else {
                    empty
                }
            }
        };
        framed.push((
            path.to_string(),
            digest::input_digest(factory, path.as_str(), &content),
        ));
    }
    TaskRecord::new(
        task_id.to_string(),
        compute_exec_digest(factory, dag, task_id, &framed),
        TaskStatus::Skipped,
        None,
        empty,
        empty,
        Vec::new(),
    )
}

/// One task admitted to the worker pool, with everything its
/// execution needs resolved.
struct WorkItem {
    /// The task to execute.
    task_id: String,
    /// Its execution digest, computed by the coordinator.
    exec_digest: Digest,
    /// The materialization inputs: normalized path and content
    /// blob digest.
    inputs: Vec<(NormalizedPath, Digest)>,
}

/// The pool state shared between the coordinator and the workers.
struct Shared {
    /// The locked pool state.
    state: Mutex<SharedState>,
    /// Signalled whenever the ready set grows or shutdown begins.
    available: Condvar,
}

/// The locked pool state.
struct SharedState {
    /// Admitted tasks waiting for a worker, keyed by task id so
    /// the smallest id is always taken first.
    ready: BTreeMap<String, WorkItem>,
    /// Set when no further work is admitted.
    shutdown: bool,
}

/// What a worker sends back for one admitted task.
enum WorkerReport {
    /// The task reached a recorded outcome.
    Finished {
        /// The executed task.
        task_id: String,
        /// Its manifest record.
        record: TaskRecord,
        /// Its failure detail, when the record shows a failure the
        /// exit code cannot explain.
        failure_detail: Option<String>,
    },
    /// The run must abort: the task hit a store failure. The
    /// failure itself names its context (the store path, or the
    /// task for a missing input).
    Aborted {
        /// The failure.
        error: RunError,
    },
}

/// The size of the worker pool. `jobs` of 0 selects the available
/// parallelism; the pool never exceeds the task count.
fn worker_count(jobs: usize, tasks: usize) -> usize {
    let wanted = if jobs == 0 {
        std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
    } else {
        jobs
    };
    wanted.min(tasks).max(1)
}

/// The worker loop: take the lexicographically smallest ready
/// task, execute it, report back. All bookkeeping, including
/// every digest the run produces, stays with the coordinator.
fn worker_loop(
    cas: &Cas,
    work_root: &Path,
    dag: &ValidatedDag,
    shared: &Shared,
    reports: &mpsc::Sender<WorkerReport>,
) {
    let runner = Runner::new(cas);
    loop {
        let item = {
            let mut state = shared.state.lock().expect("scheduler state lock");
            loop {
                if let Some((_, item)) = state.ready.pop_first() {
                    break item;
                }
                if state.shutdown {
                    return;
                }
                state = shared.available.wait(state).expect("scheduler state wait");
            }
        };
        let report = execute_item(cas, &runner, work_root, dag, &item);
        if reports.send(report).is_err() {
            return;
        }
    }
}

/// Executes one admitted task: consult the by-exec cache first,
/// execute on a miss, and publish the cache entry on success.
fn execute_item(
    cas: &Cas,
    runner: &Runner<'_>,
    work_root: &Path,
    dag: &ValidatedDag,
    item: &WorkItem,
) -> WorkerReport {
    let task_id = item.task_id.clone();
    match lookup_cache(cas, item) {
        Ok(Some(record)) => {
            return WorkerReport::Finished {
                task_id,
                record,
                failure_detail: None,
            };
        }
        Ok(None) => {}
        Err(error) => return WorkerReport::Aborted { error },
    }

    let spec = &dag.tasks[&item.task_id];
    let sandbox = work_root.join("tasks").join(&item.task_id);
    match runner.execute(spec, &item.inputs, &sandbox) {
        Ok(execution) => {
            let status = if execution.timed_out {
                TaskStatus::TimedOut
            } else if execution.exit_code == Some(0) && execution.failure_detail.is_none() {
                TaskStatus::Succeeded
            } else {
                TaskStatus::Failed
            };
            let record = TaskRecord::new(
                item.task_id.clone(),
                item.exec_digest,
                status,
                execution.exit_code,
                execution.stdout_digest,
                execution.stderr_digest,
                execution.artifacts,
            );
            if status == TaskStatus::Succeeded
                && let Err(error) = store_cache_entry(cas, &record)
            {
                return WorkerReport::Aborted { error };
            }
            WorkerReport::Finished {
                task_id,
                record,
                failure_detail: execution.failure_detail,
            }
        }
        Err(error) => WorkerReport::Aborted { error },
    }
}

/// The coordinator's run state. The coordinator is the only
/// writer of records, which is what keeps the manifest a pure
/// function of the run's outcomes (INV-RUN-5).
struct Coordinator {
    /// Records of tasks that finished executing, by task id.
    records: BTreeMap<String, TaskRecord>,
    /// Failure details by task id.
    failures: BTreeMap<String, String>,
    /// Tasks marked skipped by failure propagation; their records
    /// are built once every producer's final status is known.
    doomed: BTreeSet<String>,
    /// The first run-aborting failure, once one lands.
    first_error: Option<RunError>,
    /// Tasks admitted to the ready set so far.
    dispatched: usize,
    /// Worker reports received so far.
    reported: usize,
}

impl Coordinator {
    /// Consumes worker reports until every task is settled, or,
    /// once a run-aborting failure lands, until every admitted
    /// task has reported back.
    fn run_to_completion(
        &mut self,
        factory: &dyn HasherFactory,
        dag: &ValidatedDag,
        plans: &BTreeMap<String, Vec<(NormalizedPath, InputSource)>>,
        shared: &Shared,
        reports: &mpsc::Receiver<WorkerReport>,
        pending: &mut BTreeMap<String, usize>,
    ) {
        let total = dag.tasks.len();
        loop {
            if self.first_error.is_some() {
                if self.reported >= self.dispatched {
                    return;
                }
            } else if self.records.len() + self.doomed.len() >= total {
                return;
            }
            let Ok(report) = reports.recv() else { return };
            self.reported += 1;
            match report {
                WorkerReport::Aborted { error } => {
                    if self.first_error.is_none() {
                        self.first_error = Some(error);
                        let mut state = shared.state.lock().expect("scheduler state lock");
                        // Admitted tasks that never started are
                        // accounted for here, so the drain waits
                        // only for tasks actually executing.
                        self.dispatched -= state.ready.len();
                        state.ready.clear();
                        state.shutdown = true;
                        drop(state);
                        shared.available.notify_all();
                    }
                }
                WorkerReport::Finished {
                    task_id,
                    record,
                    failure_detail,
                } => {
                    if self.first_error.is_some() {
                        continue;
                    }
                    if let Some(detail) = failure_detail {
                        self.failures.insert(task_id.clone(), detail);
                    }
                    let status = record.status;
                    self.records.insert(task_id.clone(), record);
                    match status {
                        TaskStatus::Failed | TaskStatus::TimedOut => {
                            for dependent in transitive_dependents(dag, &task_id) {
                                if !self.records.contains_key(&dependent) {
                                    self.doomed.insert(dependent);
                                }
                            }
                        }
                        TaskStatus::Succeeded | TaskStatus::Cached => {
                            self.release_dependents(factory, dag, plans, shared, pending, &task_id);
                        }
                        TaskStatus::Skipped => {
                            unreachable!("workers never report a skipped task")
                        }
                    }
                }
            }
        }
    }

    /// After `task_id` succeeds, admits each direct dependent
    /// whose dependencies are now all satisfied.
    fn release_dependents(
        &mut self,
        factory: &dyn HasherFactory,
        dag: &ValidatedDag,
        plans: &BTreeMap<String, Vec<(NormalizedPath, InputSource)>>,
        shared: &Shared,
        pending: &mut BTreeMap<String, usize>,
        task_id: &str,
    ) {
        let mut admitted: Vec<WorkItem> = Vec::new();
        if let Some(dependents) = dag.dependents.get(task_id) {
            for dependent in dependents {
                if self.doomed.contains(dependent) || self.records.contains_key(dependent) {
                    continue;
                }
                let count = pending
                    .get_mut(dependent)
                    .expect("every task has a pending count");
                *count -= 1;
                if *count == 0 {
                    admitted.push(build_work_item(
                        factory,
                        dag,
                        plans,
                        &self.records,
                        dependent,
                    ));
                }
            }
        }
        if admitted.is_empty() {
            return;
        }
        let mut state = shared.state.lock().expect("scheduler state lock");
        for item in admitted {
            self.dispatched += 1;
            state.ready.insert(item.task_id.clone(), item);
        }
        drop(state);
        shared.available.notify_all();
    }
}

/// Consults the store's `by-exec/` index for `item`'s execution
/// digest (section 6.1). The entry is honored only when the
/// recorded record parses, names this execution digest, encodes
/// canonically to the very bytes stored, and every blob the
/// record names still exists and re-hashes correctly. Anything
/// else is a miss, never an error (INV-RUN-7); a store failure
/// that is neither a missing nor a corrupt target aborts.
fn lookup_cache(cas: &Cas, item: &WorkItem) -> Result<Option<TaskRecord>, RunError> {
    let entry_path = cas.root().join(by_exec_relative_path(&item.exec_digest));
    let Ok(text) = fs::read_to_string(&entry_path) else {
        return Ok(None);
    };
    let Ok(record_digest) = Digest::from_hex(text.trim()) else {
        return Ok(None);
    };
    let bytes = match cas.read_to_vec(&record_digest) {
        Ok(bytes) => bytes,
        Err(StoreError::Missing { .. } | StoreError::Integrity { .. }) => return Ok(None),
        Err(source) => return Err(source.into()),
    };
    let Ok(value) = verirun_core::parse::parse(&bytes) else {
        return Ok(None);
    };
    let Ok(record) = TaskRecord::from_value(&value) else {
        return Ok(None);
    };
    if record.exec_digest != item.exec_digest || record.status != TaskStatus::Succeeded {
        return Ok(None);
    }
    let mut reencoded = Vec::new();
    if canonical::encode(&record.to_value(), &mut reencoded).is_err() || reencoded != bytes {
        return Ok(None);
    }
    for named in record.named_digests() {
        match cas.copy_to(&named, &mut io::sink()) {
            Ok(_) => {}
            Err(StoreError::Missing { .. } | StoreError::Integrity { .. }) => return Ok(None),
            Err(source) => return Err(source.into()),
        }
    }
    let mut cached = record;
    cached.status = TaskStatus::Cached;
    Ok(Some(cached))
}

/// Publishes a succeeded task's cache entry: the record's
/// canonical bytes go into the store as a blob, and the entry
/// file in `by-exec/` is written holding that blob's digest as
/// hex text, through the same temp, sync, rename, directory sync
/// discipline as the put protocol (section 6.2).
fn store_cache_entry(cas: &Cas, record: &TaskRecord) -> Result<(), RunError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut bytes = Vec::new();
    canonical::encode(&record.to_value(), &mut bytes)
        .map_err(|source| canonical_failure(cas.root(), &source))?;
    let record_digest = cas.put_bytes(&bytes)?;

    let entry_path = cas.root().join(by_exec_relative_path(&record.exec_digest));
    let shard = entry_path
        .parent()
        .expect("a by-exec entry path has a shard parent")
        .to_path_buf();
    fs::create_dir_all(&shard).map_err(|source| {
        RunError::from(StoreError::Io {
            path: shard.clone(),
            step: StoreStep::CreateShard,
            source,
        })
    })?;

    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_path = cas.root().join("tmp").join(format!(
        "by-exec-{}-{}-{unique}",
        std::process::id(),
        record.exec_digest
    ));
    let write_temp = |temp_path: &PathBuf| -> Result<(), RunError> {
        let mut file = fs::File::create(temp_path).map_err(|source| {
            RunError::from(StoreError::Io {
                path: temp_path.clone(),
                step: StoreStep::CreateTemp,
                source,
            })
        })?;
        file.write_all(record_digest.to_hex().as_bytes())
            .map_err(|source| {
                RunError::from(StoreError::Io {
                    path: temp_path.clone(),
                    step: StoreStep::Write,
                    source,
                })
            })?;
        file.sync_all().map_err(|source| {
            RunError::from(StoreError::Io {
                path: temp_path.clone(),
                step: StoreStep::SyncFile,
                source,
            })
        })?;
        Ok(())
    };
    if let Err(error) = write_temp(&temp_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    fs::rename(&temp_path, &entry_path).map_err(|source| {
        RunError::from(StoreError::Io {
            path: entry_path.clone(),
            step: StoreStep::Rename,
            source,
        })
    })?;
    let shard_dir = fs::File::open(&shard).map_err(|source| {
        RunError::from(StoreError::Io {
            path: shard.clone(),
            step: StoreStep::SyncShard,
            source,
        })
    })?;
    shard_dir.sync_all().map_err(|source| {
        RunError::from(StoreError::Io {
            path: shard,
            step: StoreStep::SyncShard,
            source,
        })
    })?;
    Ok(())
}
