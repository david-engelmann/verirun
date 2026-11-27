#![no_main]

use libfuzzer_sys::fuzz_target;
use verirun_core::{dag, parse};

fuzz_target!(|data: &[u8]| {
    if let Ok(value) = parse::parse(data) {
        let first = dag::validate_run_spec(&value);
        let second = dag::validate_run_spec(&value);
        assert_eq!(
            first, second,
            "validate_run_spec must be deterministic across repeated calls"
        );
        if let Ok(validated) = first {
            assert_eq!(validated.order.len(), validated.tasks.len());
            for task_id in &validated.order {
                assert!(validated.task(task_id).is_some());
            }
            for (owner_id, outputs) in &validated.outputs {
                for out_path in outputs {
                    assert_eq!(validated.producer_of(out_path), Some(owner_id.as_str()));
                }
            }
        }
    }
});
