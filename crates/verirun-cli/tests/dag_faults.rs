//! Validation precedence oracle (INV-LAT-4).
//!
//! An invalid run is rejected before the store is opened: run
//! against paths that do not exist, an invalid spec leaves both
//! the store path and the work path uncreated.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use verirun_cli::{CliError, run_spec_bytes};
use verirun_core::error::DagError;

static COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn fault_invalid_run_leaves_store_untouched() {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    let base = std::env::temp_dir().join(format!("verirun-lat4-{}-{unique}", std::process::id()));
    let cas_root: PathBuf = base.join("cas");
    let work_root: PathBuf = base.join("work");
    // A cyclic spec: invalid at check 6, after every earlier
    // check has passed.
    let spec = r#"{"tasks":[
        {"task_id":"a","executable":"sh","args":[],"env":{},"inputs":[],"outputs":[],"depends_on":["b"]},
        {"task_id":"b","executable":"sh","args":[],"env":{},"inputs":[],"outputs":[],"depends_on":["a"]}
    ]}"#;
    let outcome = run_spec_bytes(spec.as_bytes(), &cas_root, &work_root, &BTreeMap::new(), 1);
    match outcome {
        Err(CliError::Dag {
            source: DagError::Cycle { remnants },
        }) => {
            assert_eq!(remnants, vec!["a".to_string(), "b".to_string()]);
        }
        other => panic!("expected a cycle rejection, got {other:?}"),
    }
    assert!(
        !cas_root.exists(),
        "the store path must not be created for an invalid run"
    );
    assert!(
        !work_root.exists(),
        "the work path must not be created for an invalid run"
    );
    let _ = std::fs::remove_dir_all(&base);
}
