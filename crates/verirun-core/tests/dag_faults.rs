//! DAG validation fault oracles (INV-DAG-1 to INV-DAG-6).
//!
//! Every test feeds a parsed run file to core validation and
//! asserts the exact first error, because the fixed check order of
//! section 7.1 makes the first error a pure function of the spec
//! bytes.

use verirun_core::dag::{ValidatedDag, validate_run_spec};
use verirun_core::error::DagError;
use verirun_core::parse::parse;
use verirun_core::value::Value;

fn spec(json: &str) -> Value {
    parse(json.as_bytes()).expect("test spec parses")
}

fn validate(json: &str) -> Result<ValidatedDag, DagError> {
    validate_run_spec(&spec(json))
}

fn task(id: &str) -> String {
    format!(
        r#"{{"task_id":"{id}","executable":"run","args":[],"env":{{}},"inputs":[],"outputs":[],"depends_on":[]}}"#
    )
}

fn task_with(id: &str, extra: &str) -> String {
    format!(
        r#"{{"task_id":"{id}","executable":"run","args":[],"env":{{}},"inputs":[],"outputs":[],"depends_on":[],{extra}}}"#
    )
}

fn run(tasks: &[String]) -> String {
    format!(r#"{{"tasks":[{}]}}"#, tasks.join(","))
}

#[test]
fn fault_cycle_table() {
    // A self dependency.
    let mut selfish = task("a");
    selfish = selfish.replace(r#""depends_on":[]}"#, r#""depends_on":["a"]}"#);
    assert_eq!(
        validate(&run(&[selfish])),
        Err(DagError::Cycle {
            remnants: vec!["a".to_string()]
        })
    );

    // A two cycle.
    let cyc_a = task("a").replace(r#""depends_on":[]}"#, r#""depends_on":["b"]}"#);
    let cyc_b = task("b").replace(r#""depends_on":[]}"#, r#""depends_on":["a"]}"#);
    assert_eq!(
        validate(&run(&[cyc_a, cyc_b])),
        Err(DagError::Cycle {
            remnants: vec!["a".to_string(), "b".to_string()]
        })
    );

    // A cycle inside a large acyclic graph: the remnants are
    // exactly the cyclic tasks, sorted, and nothing else.
    let mut tasks: Vec<String> = vec![task("t0")];
    for index in 1..8 {
        let chained = task(&format!("t{index}")).replace(
            r#""depends_on":[]}"#,
            &format!(r#""depends_on":["t{}"]}}"#, index - 1),
        );
        tasks.push(chained);
    }
    let cyc_x = task("x").replace(r#""depends_on":[]}"#, r#""depends_on":["z"]}"#);
    let cyc_y = task("y").replace(r#""depends_on":[]}"#, r#""depends_on":["x"]}"#);
    let cyc_z = task("z").replace(r#""depends_on":[]}"#, r#""depends_on":["y"]}"#);
    tasks.extend([cyc_x, cyc_y, cyc_z]);
    assert_eq!(
        validate(&run(&tasks)),
        Err(DagError::Cycle {
            remnants: vec!["x".to_string(), "y".to_string(), "z".to_string()]
        })
    );

    // A diamond is not a cycle and validates, with an order that
    // respects every edge.
    let top = task("top");
    let left = task("left").replace(r#""depends_on":[]}"#, r#""depends_on":["top"]}"#);
    let right = task("right").replace(r#""depends_on":[]}"#, r#""depends_on":["top"]}"#);
    let join = task("join").replace(r#""depends_on":[]}"#, r#""depends_on":["left","right"]}"#);
    let dag = validate(&run(&[top, left, right, join])).expect("diamond validates");
    let position = |id: &str| {
        dag.order
            .iter()
            .position(|t| t == id)
            .expect("task ordered")
    };
    assert!(position("top") < position("left"));
    assert!(position("top") < position("right"));
    assert!(position("left") < position("join"));
    assert!(position("right") < position("join"));
    assert_eq!(dag.order.len(), 4);
}

#[test]
fn fault_missing_dependency() {
    let a = task("a").replace(r#""depends_on":[]}"#, r#""depends_on":["ghost"]}"#);
    assert_eq!(
        validate(&run(&[a])),
        Err(DagError::UnknownDependency {
            task_id: "a".to_string(),
            missing: "ghost".to_string()
        })
    );

    // An id that differs only in case is a different id, and
    // missing.
    let b = task("b").replace(r#""depends_on":[]}"#, r#""depends_on":["Task"]}"#);
    let lower = task("task");
    assert_eq!(
        validate(&run(&[b, lower])),
        Err(DagError::UnknownDependency {
            task_id: "b".to_string(),
            missing: "Task".to_string()
        })
    );

    // An id that differs only in trailing whitespace is a
    // different id, and missing.
    let c = task("c").replace(r#""depends_on":[]}"#, r#""depends_on":["a "]}"#);
    let plain = task("a");
    assert_eq!(
        validate(&run(&[c, plain])),
        Err(DagError::UnknownDependency {
            task_id: "c".to_string(),
            missing: "a ".to_string()
        })
    );
}

#[test]
fn fault_duplicate_task_id() {
    // Duplication across non-adjacent positions in the file.
    let tasks = vec![task("a"), task("b"), task("c"), task("a")];
    assert_eq!(
        validate(&run(&tasks)),
        Err(DagError::DuplicateTaskId {
            task_id: "a".to_string()
        })
    );
}

#[test]
fn fault_colliding_outputs_incl_normalized() {
    // The same path claimed by two tasks.
    let a = task("a").replace(r#""outputs":[]"#, r#""outputs":["out.txt"]"#);
    let b = task("b").replace(r#""outputs":[]"#, r#""outputs":["out.txt"]"#);
    assert_eq!(
        validate(&run(&[a, b])),
        Err(DagError::CollidingOutput {
            path: "out.txt".to_string(),
            first_task: "a".to_string(),
            second_task: "b".to_string()
        })
    );

    // The same path claimed twice by one task.
    let solo = task("solo").replace(r#""outputs":[]"#, r#""outputs":["x.txt","x.txt"]"#);
    assert_eq!(
        validate(&run(&[solo])),
        Err(DagError::CollidingOutput {
            path: "x.txt".to_string(),
            first_task: "solo".to_string(),
            second_task: "solo".to_string()
        })
    );

    // A collision that only exists after normalization: the two
    // spellings are one path.
    let dotted = task("d").replace(r#""outputs":[]"#, r#""outputs":["a/./b.txt"]"#);
    let plain = task("p").replace(r#""outputs":[]"#, r#""outputs":["a/b.txt"]"#);
    assert_eq!(
        validate(&run(&[dotted, plain])),
        Err(DagError::CollidingOutput {
            path: "a/b.txt".to_string(),
            first_task: "d".to_string(),
            second_task: "p".to_string()
        })
    );

    // A leading dot component normalizes away as well.
    let dotted = task("d").replace(r#""outputs":[]"#, r#""outputs":["./x.txt"]"#);
    let plain = task("p").replace(r#""outputs":[]"#, r#""outputs":["x.txt"]"#);
    assert_eq!(
        validate(&run(&[dotted, plain])),
        Err(DagError::CollidingOutput {
            path: "x.txt".to_string(),
            first_task: "d".to_string(),
            second_task: "p".to_string()
        })
    );
}

#[test]
fn fault_first_error_stable() {
    // Simultaneously cyclic and duplicated: the duplicate is
    // reported, because check 2 precedes check 6. The same spec
    // reports the same error on every run.
    let a = task("a").replace(r#""depends_on":[]}"#, r#""depends_on":["a"]}"#);
    let dup = run(&[a.clone(), a]);
    let first = validate(&dup);
    let second = validate(&dup);
    assert_eq!(first, second);
    assert_eq!(
        first,
        Err(DagError::DuplicateTaskId {
            task_id: "a".to_string()
        })
    );

    // Unknown dependency (check 3) beats colliding outputs
    // (check 4).
    let a = task("a")
        .replace(r#""depends_on":[]}"#, r#""depends_on":["ghost"]}"#)
        .replace(r#""outputs":[]"#, r#""outputs":["o.txt"]"#);
    let b = task("b").replace(r#""outputs":[]"#, r#""outputs":["o.txt"]"#);
    assert_eq!(
        validate(&run(&[a, b])),
        Err(DagError::UnknownDependency {
            task_id: "a".to_string(),
            missing: "ghost".to_string()
        })
    );

    // A schema violation (check 1) beats a duplicate id
    // (check 2).
    let bad = task_with("a", r#""surprise":true"#);
    let tasks = run(&[bad, task("a")]);
    assert!(matches!(validate(&tasks), Err(DagError::Schema { .. })));
}

#[test]
fn fault_schema_rejection_table() {
    let rejected: Vec<String> = vec![
        // An unknown task field.
        run(&[task_with("a", r#""surprise":true"#)]),
        // A missing required field.
        run(&[r#"{"task_id":"a","executable":"run","env":{},"inputs":[],"outputs":[],"depends_on":[]}"#.to_string()]),
        // A wrong-typed field.
        run(&[task("a").replace(r#""args":[]"#, r#""args":"none""#)]),
        // An empty task id.
        run(&[task("")]),
        // An empty executable.
        run(&[task("a").replace(r#""executable":"run""#, r#""executable":"""#)]),
        // A declared fixed-default env key, with the default's own
        // value: fixed means fixed, still rejected.
        run(&[task("a").replace(r#""env":{}"#, r#""env":{"TZ":"UTC"}"#)]),
        run(&[task("a").replace(r#""env":{}"#, r#""env":{"LC_ALL":"C"}"#)]),
        run(&[task("a").replace(r#""env":{}"#, r#""env":{"SOURCE_DATE_EPOCH":"0"}"#)]),
        // timeout_ms below 1.
        run(&[task_with("a", r#""timeout_ms":0"#)]),
        run(&[task_with("a", r#""timeout_ms":-5"#)]),
        // A fractional timeout.
        run(&[task_with("a", r#""timeout_ms":1.5"#)]),
        // An unknown top-level field.
        r#"{"tasks":[],"extra":1}"#.to_string(),
        // The tasks field of the wrong type.
        r#"{"tasks":{}}"#.to_string(),
    ];
    for (index, json) in rejected.iter().enumerate() {
        assert!(
            matches!(validate(json), Err(DagError::Schema { .. })),
            "case {index} must be a schema rejection: {json}"
        );
    }

    // Controls: timeout_ms of exactly 1 is accepted, and so is a
    // plain task with no timeout.
    assert!(validate(&run(&[task_with("a", r#""timeout_ms":1"#)])).is_ok());
    assert!(validate(&run(&[task("a")])).is_ok());
    assert!(validate(r#"{"tasks":[]}"#).is_ok());
}
