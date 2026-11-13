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
    /// Never panics in the core implementation.
    #[must_use]
    pub fn producer_of(&self, path: &NormalizedPath) -> Option<&str> {
        self.outputs
            .iter()
            .find_map(|(task_id, outputs)| outputs.contains(path).then_some(task_id.as_str()))
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
/// Never panics in the core implementation: every failure is a
/// [`DagError`].
pub fn validate_run_spec(spec: &Value) -> Result<ValidatedDag, DagError> {
    let root = as_object(spec).ok_or_else(|| schema(None, "the run file must be a JSON object"))?;
    for (key, _) in root {
        if key != "tasks" {
            return Err(schema(None, &format!("unknown top-level field {key:?}")));
        }
    }
    let tasks_value =
        field(root, "tasks").ok_or_else(|| schema(None, "the run file has no \"tasks\" field"))?;
    let Value::Array(task_values) = tasks_value else {
        return Err(schema(None, "\"tasks\" must be an array"));
    };
    let mut specs: Vec<TaskSpec> = Vec::with_capacity(task_values.len());
    for item in task_values {
        specs.push(parse_task(item)?);
    }

    // Check 2: duplicate task ids, in file order.
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for task in &specs {
        if !seen.insert(task.task_id.as_str()) {
            return Err(DagError::DuplicateTaskId {
                task_id: task.task_id.clone(),
            });
        }
    }
    let ids: std::collections::BTreeSet<&str> =
        specs.iter().map(|task| task.task_id.as_str()).collect();

    // Check 3: unknown dependencies, in file and declared order.
    for task in &specs {
        for dependency in &task.depends_on {
            if !ids.contains(dependency.as_str()) {
                return Err(DagError::UnknownDependency {
                    task_id: task.task_id.clone(),
                    missing: dependency.clone(),
                });
            }
        }
    }

    // Output paths are validated now because check 4 compares
    // normalized forms. A path that fails validation takes no
    // part in the collision check; check 5 reports it.
    let output_results: Vec<Vec<Result<NormalizedPath, crate::error::PathError>>> = specs
        .iter()
        .map(|task| {
            task.outputs
                .iter()
                .map(|declared| crate::paths::validate_path(declared))
                .collect()
        })
        .collect();

    // Check 4: colliding outputs, in file and declared order.
    let mut owners: BTreeMap<&str, &str> = BTreeMap::new();
    for (task, results) in specs.iter().zip(output_results.iter()) {
        for normalized in results.iter().filter_map(|result| result.as_ref().ok()) {
            if let Some(first) = owners.get(normalized.as_str()) {
                return Err(DagError::CollidingOutput {
                    path: normalized.as_str().to_string(),
                    first_task: (*first).to_string(),
                    second_task: task.task_id.clone(),
                });
            }
            owners.insert(normalized.as_str(), task.task_id.as_str());
        }
    }

    // Check 5: path safety, inputs before outputs within a task,
    // in file and declared order.
    let mut normalized_inputs: BTreeMap<String, Vec<NormalizedPath>> = BTreeMap::new();
    let mut normalized_outputs: BTreeMap<String, Vec<NormalizedPath>> = BTreeMap::new();
    for (task, results) in specs.iter().zip(output_results.iter()) {
        let mut inputs: Vec<NormalizedPath> = Vec::with_capacity(task.inputs.len());
        for declared in &task.inputs {
            match crate::paths::validate_path(declared) {
                Ok(normalized) => inputs.push(normalized),
                Err(source) => {
                    return Err(DagError::InvalidPath {
                        task_id: task.task_id.clone(),
                        source,
                    });
                }
            }
        }
        let mut outputs: Vec<NormalizedPath> = Vec::with_capacity(results.len());
        for result in results {
            match result {
                Ok(normalized) => outputs.push(normalized.clone()),
                Err(source) => {
                    return Err(DagError::InvalidPath {
                        task_id: task.task_id.clone(),
                        source: source.clone(),
                    });
                }
            }
        }
        normalized_inputs.insert(task.task_id.clone(), inputs);
        normalized_outputs.insert(task.task_id.clone(), outputs);
    }

    assemble_and_order(specs, normalized_inputs, normalized_outputs)
}

/// Assembly and check 6: Kahn's algorithm over a
/// lexicographically ordered ready set.
fn assemble_and_order(
    specs: Vec<TaskSpec>,
    normalized_inputs: BTreeMap<String, Vec<NormalizedPath>>,
    normalized_outputs: BTreeMap<String, Vec<NormalizedPath>>,
) -> Result<ValidatedDag, DagError> {
    let mut tasks: BTreeMap<String, TaskSpec> = BTreeMap::new();
    let mut dependencies: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for task in specs {
        let mut deps = task.depends_on.clone();
        deps.sort();
        deps.dedup();
        dependencies.insert(task.task_id.clone(), deps);
        tasks.insert(task.task_id.clone(), task);
    }
    let mut dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut indegree: BTreeMap<String, usize> = BTreeMap::new();
    for (task_id, deps) in &dependencies {
        indegree.insert(task_id.clone(), deps.len());
        for dependency in deps {
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(task_id.clone());
        }
    }
    let mut ready: std::collections::BTreeSet<String> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(task_id, _)| task_id.clone())
        .collect();
    let mut order: Vec<String> = Vec::with_capacity(tasks.len());
    while let Some(task_id) = ready.iter().next().cloned() {
        ready.remove(&task_id);
        if let Some(direct) = dependents.get(&task_id) {
            for dependent in direct {
                let degree = indegree
                    .get_mut(dependent)
                    .expect("every dependent has an indegree entry");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(dependent.clone());
                }
            }
        }
        order.push(task_id);
    }
    if order.len() != tasks.len() {
        let remnants: Vec<String> = indegree
            .iter()
            .filter(|(_, degree)| **degree > 0)
            .map(|(task_id, _)| task_id.clone())
            .collect();
        return Err(DagError::Cycle { remnants });
    }

    Ok(ValidatedDag {
        tasks,
        order,
        inputs: normalized_inputs,
        outputs: normalized_outputs,
        dependencies,
        dependents,
    })
}

/// The field names a task object may hold (section 3.2).
const TASK_FIELDS: [&str; 8] = [
    "task_id",
    "executable",
    "args",
    "env",
    "inputs",
    "outputs",
    "depends_on",
    "timeout_ms",
];

/// The environment keys the runner sets unconditionally
/// (section 8.1). A spec must not declare them.
const FIXED_ENV_KEYS: [&str; 3] = ["LC_ALL", "SOURCE_DATE_EPOCH", "TZ"];

const fn as_object(value: &Value) -> Option<&Vec<(String, Value)>> {
    match value {
        Value::Object(pairs) => Some(pairs),
        _ => None,
    }
}

fn field<'a>(pairs: &'a [(String, Value)], name: &str) -> Option<&'a Value> {
    pairs
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value)
}

fn schema(task_id: Option<String>, detail: &str) -> DagError {
    DagError::Schema {
        task_id,
        detail: detail.to_string(),
    }
}

fn required<'a>(
    pairs: &'a [(String, Value)],
    name: &'static str,
    task_id: Option<&String>,
) -> Result<&'a Value, DagError> {
    field(pairs, name).ok_or_else(|| DagError::Schema {
        task_id: task_id.cloned(),
        detail: format!("task is missing required field {name:?}"),
    })
}

fn string_list(
    value: &Value,
    name: &'static str,
    task_id: Option<&String>,
) -> Result<Vec<String>, DagError> {
    match value {
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::String(text) => out.push(text.clone()),
                    _ => {
                        return Err(schema(
                            task_id.cloned(),
                            &format!("{name:?} must be an array of strings"),
                        ));
                    }
                }
            }
            Ok(out)
        }
        _ => Err(schema(
            task_id.cloned(),
            &format!("{name:?} must be an array of strings"),
        )),
    }
}

/// Parses and schema-checks one task object (check 1 of
/// section 7.1 for this task).
#[allow(
    clippy::float_cmp,
    reason = "the timeout check is an exactness test on the parsed value: a timeout is valid only when the number is exactly integral"
)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the timeout value is range-checked against 1..=9007199254740991 immediately before the conversion, so the conversion is exact"
)]
fn parse_task(item: &Value) -> Result<TaskSpec, DagError> {
    let pairs = as_object(item).ok_or_else(|| schema(None, "each task must be a JSON object"))?;
    for (key, _) in pairs {
        if !TASK_FIELDS.contains(&key.as_str()) {
            let named = field(pairs, "task_id").and_then(|value| match value {
                Value::String(text) => Some(text.clone()),
                _ => None,
            });
            return Err(schema(named, &format!("unknown field {key:?}")));
        }
    }
    let named = field(pairs, "task_id").and_then(|value| match value {
        Value::String(text) => Some(text.clone()),
        _ => None,
    });
    let task_id = match required(pairs, "task_id", named.as_ref())? {
        Value::String(text) if !text.is_empty() => text.clone(),
        _ => {
            return Err(schema(
                named.clone(),
                "\"task_id\" must be a non-empty string",
            ));
        }
    };
    let named = Some(task_id.clone());
    let executable = match required(pairs, "executable", named.as_ref())? {
        Value::String(text) if !text.is_empty() => text.clone(),
        _ => {
            return Err(schema(
                named.clone(),
                "\"executable\" must be a non-empty string",
            ));
        }
    };
    let args = string_list(
        required(pairs, "args", named.as_ref())?,
        "args",
        named.as_ref(),
    )?;
    let env = parse_env(pairs, named.as_ref())?;
    let inputs = string_list(
        required(pairs, "inputs", named.as_ref())?,
        "inputs",
        named.as_ref(),
    )?;
    let outputs = string_list(
        required(pairs, "outputs", named.as_ref())?,
        "outputs",
        named.as_ref(),
    )?;
    let depends_on = string_list(
        required(pairs, "depends_on", named.as_ref())?,
        "depends_on",
        named.as_ref(),
    )?;
    let timeout_ms = match field(pairs, "timeout_ms") {
        None => None,
        Some(Value::Number(value)) => {
            if *value < 1.0 || *value > 9_007_199_254_740_991.0 || value.fract() != 0.0 {
                return Err(schema(
                    named,
                    "\"timeout_ms\" must be an integer of at least 1",
                ));
            }
            Some(*value as u64)
        }
        Some(_) => {
            return Err(schema(
                named,
                "\"timeout_ms\" must be an integer of at least 1",
            ));
        }
    };
    Ok(TaskSpec {
        task_id,
        executable,
        args,
        env,
        inputs,
        outputs,
        depends_on,
        timeout_ms,
    })
}

/// Parses the `env` object of one task (part of check 1):
/// string values only, and none of the fixed default keys.
fn parse_env(
    pairs: &[(String, Value)],
    named: Option<&String>,
) -> Result<BTreeMap<String, String>, DagError> {
    match required(pairs, "env", named)? {
        Value::Object(entries) => {
            let mut map = BTreeMap::new();
            for (key, value) in entries {
                if FIXED_ENV_KEYS.contains(&key.as_str()) {
                    return Err(schema(
                        named.cloned(),
                        &format!(
                            "env declares the fixed key {key:?}; fixed defaults are set by the runner and must not be declared"
                        ),
                    ));
                }
                match value {
                    Value::String(text) => {
                        map.insert(key.clone(), text.clone());
                    }
                    _ => {
                        return Err(schema(named.cloned(), "\"env\" values must be strings"));
                    }
                }
            }
            Ok(map)
        }
        _ => Err(schema(named.cloned(), "\"env\" must be an object")),
    }
}
