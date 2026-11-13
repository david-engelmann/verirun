//! Task specifications, DAG validation, and Kahn ordering
//! (docs/ARCHITECTURE.md sections 3.2 and 7).
//!
//! A run file is a JSON object with one field, `tasks`, holding an
//! array of task objects in the shape of [`TaskSpec`]. Validation
//! runs before any task executes and before any sandbox is created,
//! and it runs its checks in the fixed order of section 7.1, so the
//! first error reported for a spec is a pure function of the spec
//! bytes (INV-DAG-5):
//!
//! 1. Schema. Every task object matches section 3.2, including the
//!    unknown field rule and the `timeout_ms` lower bound.
//! 2. Duplicate task ids.
//! 3. Unknown dependencies.
//! 4. Colliding outputs, compared on normalized paths.
//! 5. Path safety, on every declared input and output path.
//! 6. Cycles, by Kahn's algorithm over a lexicographically ordered
//!    ready set. The error lists the tasks left with nonzero
//!    indegree, sorted.
//!
//! Within one check, tasks are examined in file order and declared
//! lists in declared order, so ties inside a check are also
//! deterministic. A spec that passes produces a [`ValidatedDag`]:
//! the same ready set discipline the validator used is the one the
//! scheduler reuses at run time, so validation order and execution
//! eligibility cannot drift apart.

use std::collections::BTreeMap;

use crate::error::DagError;
use crate::paths::NormalizedPath;
use crate::value::Value;

/// One task in a run file, in the shape of section 3.2.
///
/// Paths are stored as declared. The normalized forms the rest of
/// the system binds and records live on [`ValidatedDag`], keyed by
/// task id, so the declared text stays available for diagnostics
/// while every comparison happens on normalized text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSpec {
    /// The task identifier. Non-empty and unique across the run.
    /// Byte order defines sort order everywhere.
    pub task_id: String,
    /// The executable string, recorded verbatim in the execution
    /// digest. Resolution rules are section 8.2.
    pub executable: String,
    /// Command arguments, in declared order. Order is part of the
    /// execution digest.
    pub args: Vec<String>,
    /// Declared environment pairs. The fixed default keys of
    /// section 8.1 (`TZ`, `LC_ALL`, `SOURCE_DATE_EPOCH`) must not
    /// appear here; validation rejects a spec that declares one.
    pub env: BTreeMap<String, String>,
    /// Sandbox relative paths of files to materialize from the
    /// store before execution, as declared.
    pub inputs: Vec<String>,
    /// Sandbox relative paths the task promises to produce, as
    /// declared. Unique across the whole run on normalized form.
    pub outputs: Vec<String>,
    /// Task ids that must succeed first, as declared.
    pub depends_on: Vec<String>,
    /// The timeout budget in milliseconds. At least 1 when present.
    pub timeout_ms: Option<u64>,
}

/// A run specification that passed every validation check.
///
/// Produced only by [`validate_run_spec`]. The maps are keyed by
/// task id in byte order. `order` is the Kahn topological order
/// under the lexicographic ready set discipline; the scheduler
/// admits tasks in this same order as their dependencies complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedDag {
    /// Every task in the run, by task id.
    pub tasks: BTreeMap<String, TaskSpec>,
    /// The full topological order of task ids.
    pub order: Vec<String>,
    /// Normalized input paths per task id, in declared order.
    pub inputs: BTreeMap<String, Vec<NormalizedPath>>,
    /// Normalized output paths per task id, in declared order.
    pub outputs: BTreeMap<String, Vec<NormalizedPath>>,
    /// Dependency edges per task id: the sorted, deduplicated set
    /// of task ids that must succeed before it. A duplicated entry
    /// in a declared `depends_on` list is one edge.
    pub dependencies: BTreeMap<String, Vec<String>>,
    /// Reverse edges per task id: the sorted set of tasks that
    /// depend on it, used by failure propagation.
    pub dependents: BTreeMap<String, Vec<String>>,
}

impl ValidatedDag {
    /// Looks up one task by id.
    #[must_use]
    pub fn task(&self, task_id: &str) -> Option<&TaskSpec> {
        self.tasks.get(task_id)
    }

    /// The producer of a normalized output path: the id of the
    /// task that declared it, if any task did. Output paths are
    /// unique across a validated run, so the answer is singular.
    ///
    /// # Panics
    ///
    /// Panics in Milestone 2 Stage A: the interface is staged
    /// ahead of its implementation, which lands in Stage B.
    #[must_use]
    pub fn producer_of(&self, path: &NormalizedPath) -> Option<&str> {
        let _ = path;
        panic!("verirun Milestone 2 Stage A: producer_of is not yet implemented")
    }
}

/// Validates a parsed run file value into a [`ValidatedDag`].
///
/// The checks run in the fixed order of section 7.1 and the module
/// documentation. The first failure stops validation; no partial
/// result is produced.
///
/// # Errors
///
/// Returns [`DagError`] describing the first violation found: a
/// schema violation, a duplicated task id, an unknown dependency, a
/// colliding output, an unsafe path, or the sorted cyclic remnant.
///
/// # Panics
///
/// Panics in Milestone 2 Stage A: the interface is staged ahead
/// of its implementation, which lands in Stage B.
pub fn validate_run_spec(spec: &Value) -> Result<ValidatedDag, DagError> {
    let _ = spec;
    panic!("verirun Milestone 2 Stage A: validate_run_spec is not yet implemented")
}
