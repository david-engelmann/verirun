//! The `run` command (docs/ARCHITECTURE.md section 12.1).
//!
//! Loads and validates the run file, validates and stages any
//! caller `--input` host files into the store, schedules the
//! graph, and optionally pins the resulting manifest. Validation
//! of the spec and of every caller input binding completes before
//! the store or work directory is opened (INV-LAT-4).

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path::{Path, PathBuf};

use verirun_core::digest::Digest;
use verirun_core::paths::{NormalizedPath, validate_path};
use verirun_store::cas_fs::Cas;
use verirun_store::error::RunError;
use verirun_store::scheduler::RunOutcome;

use crate::CliError;

/// Parses one `<REL_PATH>=<HOST_FILE>` binding from `--input`,
/// validating `<REL_PATH>` under the sandbox path rules of
/// section 8.4.
///
/// # Errors
///
/// Returns [`CliError::Argument`] when the binding lacks an `=`,
/// names an empty host path, or fails sandbox relative path
/// validation.
pub fn parse_input_binding(raw: &str) -> Result<(NormalizedPath, PathBuf), CliError> {
    let Some((rel, host)) = raw.split_once('=') else {
        return Err(CliError::Argument {
            detail: format!("invalid --input binding {raw:?}: expected <REL_PATH>=<HOST_FILE>"),
        });
    };
    if host.is_empty() {
        return Err(CliError::Argument {
            detail: format!("invalid --input binding {raw:?}: host file path is empty"),
        });
    }
    let normalized = validate_path(rel).map_err(|source| CliError::Argument {
        detail: format!("invalid --input binding {raw:?}: {source}"),
    })?;
    Ok((normalized, PathBuf::from(host)))
}

/// Runs the run file at `spec_path` against `cas_root` and
/// `work_root`.
///
/// Binds no caller inputs and does not pin the produced
/// manifest; `jobs` sizes the worker pool (`0` selects the
/// available parallelism).
///
/// # Errors
///
/// Returns [`CliError`] for an unreadable, unparsable, or
/// invalid run file and for a run that aborts. A run whose
/// tasks merely fail is not an error here: its outcome carries
/// the exit code 1 and the per-task failure details.
///
/// # Panics
///
/// Never panics in the implementation.
pub fn run(
    spec_path: &Path,
    cas_root: &Path,
    work_root: &Path,
    jobs: usize,
) -> Result<RunOutcome, CliError> {
    run_with_options(spec_path, cas_root, work_root, &[], jobs, false)
}

/// Runs the run file at `spec_path` with optional caller input
/// bindings and optional manifest pinning.
///
/// Every binding in `input_bindings` maps a validated sandbox
/// relative path to a host file whose contents are streamed into
/// the store before scheduling begins. Spec validation, duplicate
/// binding rejection, and opening each host file all complete
/// before `cas_root` or `work_root` is created (INV-LAT-4).
///
/// # Errors
///
/// Returns [`CliError`] for an unreadable, unparsable, or
/// invalid run file, duplicate or unreadable input bindings,
/// store failures, and runs that abort.
///
/// # Panics
///
/// Never panics in the implementation.
pub fn run_with_options(
    spec_path: &Path,
    cas_root: &Path,
    work_root: &Path,
    input_bindings: &[(NormalizedPath, PathBuf)],
    jobs: usize,
    pin: bool,
) -> Result<RunOutcome, CliError> {
    let dag = crate::spec_io::load_run_spec(spec_path)?;

    let mut seen_paths: BTreeSet<String> = BTreeSet::new();
    let mut staged_files: Vec<(String, File)> = Vec::with_capacity(input_bindings.len());
    for (rel_path, host_path) in input_bindings {
        if !seen_paths.insert(rel_path.as_str().to_string()) {
            return Err(CliError::Argument {
                detail: format!("duplicate --input binding for path {rel_path:?}"),
            });
        }
        let file = File::open(host_path).map_err(|source| CliError::InputIo {
            path: host_path.clone(),
            source,
        })?;
        let metadata = file.metadata().map_err(|source| CliError::InputIo {
            path: host_path.clone(),
            source,
        })?;
        if !metadata.is_file() {
            return Err(CliError::InputIo {
                path: host_path.clone(),
                source: std::io::Error::other("bound input is not a regular file"),
            });
        }
        staged_files.push((rel_path.as_str().to_string(), file));
    }

    let cas = Cas::open(cas_root).map_err(RunError::from)?;
    let mut bound_inputs: BTreeMap<String, Digest> = BTreeMap::new();
    for (rel_path, file) in staged_files {
        let digest = cas.put_stream(file).map_err(RunError::from)?;
        bound_inputs.insert(rel_path, digest);
    }

    let outcome = verirun_store::scheduler::run(
        &cas,
        &verirun_store::sha256::Sha256Factory,
        work_root,
        &dag,
        &bound_inputs,
        jobs,
    )?;
    if pin {
        cas.pin_manifest(&outcome.manifest_digest)
            .map_err(RunError::from)?;
    }
    Ok(outcome)
}
