//! Run file loading (docs/ARCHITECTURE.md section 2.4).
//!
//! A transport concern only: the file is read as bytes and the
//! bytes go to core parsing and validation, so the strict
//! parser's duplicate key rejection and byte offsets apply to
//! the file exactly as written. Nothing in this module
//! interprets the spec beyond handing it to core.

use std::path::Path;

use verirun_core::dag::ValidatedDag;

use crate::CliError;

/// Reads the run file at `path` and returns the validated
/// graph.
///
/// Parse and validation complete here, before any caller opens
/// the store, so an invalid run file leaves the store and the
/// work directory untouched (INV-LAT-4).
///
/// # Errors
///
/// Returns [`CliError::SpecIo`] when the file cannot be read,
/// [`CliError::Parse`] when its bytes fail strict parsing, and
/// [`CliError::Dag`] when the parsed spec fails validation.
///
/// # Panics
///
/// Never panics in the implementation.
pub fn load_run_spec(path: &Path) -> Result<ValidatedDag, CliError> {
    let bytes = std::fs::read(path).map_err(|source| CliError::SpecIo {
        path: path.to_path_buf(),
        source,
    })?;
    let value = verirun_core::parse::parse(&bytes)?;
    let dag = verirun_core::dag::validate_run_spec(&value)?;
    Ok(dag)
}
