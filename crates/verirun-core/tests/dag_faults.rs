//! DAG validation fault oracles (INV-DAG-1 to INV-DAG-9).
//!
//! Every test feeds a parsed run file to core validation and
//! asserts the exact first error, because the fixed check order of
//! section 7.1 makes the first error a pure function of the spec
//! bytes.

#![forbid(unsafe_code)]

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
        // A task id that is not a single safe path component: the
        // scheduler names the task's sandbox directory with it, so
        // separators and dot components would escape the layout.
        run(&[task("../escape")]),
        run(&[task("a/b")]),
        run(&[task(".")]),
        run(&[task("..")]),
        // A task id whose Win32 stripped form is empty: on
        // Windows it would name the tasks directory itself
        // rather than a sandbox inside it (INV-DAG-10).
        run(&[task("...")]),
        run(&[task(" ")]),
        run(&[task(" .")]),
        run(&[r#"{"task_id":"a\\b","executable":"run","args":[],"env":{},"inputs":[],"outputs":[],"depends_on":[]}"#.to_string()]),
        // An empty executable.
        run(&[task("a").replace(r#""executable":"run""#, r#""executable":"""#)]),
        // A declared fixed-default or runner-managed env key:
        // fixed means fixed, still rejected, and TMPDIR cannot be
        // smuggled into exec_digest before the runner overwrites
        // it at spawn.
        run(&[task("a").replace(r#""env":{}"#, r#""env":{"TZ":"UTC"}"#)]),
        run(&[task("a").replace(r#""env":{}"#, r#""env":{"LC_ALL":"C"}"#)]),
        run(&[task("a").replace(r#""env":{}"#, r#""env":{"SOURCE_DATE_EPOCH":"0"}"#)]),
        run(&[task("a").replace(r#""env":{}"#, r#""env":{"TMPDIR":"/tmp/fake"}"#)]),
        // A declared PATH or HOME: neither is runner-managed,
        // but a declared value would steer the child's own
        // command and file lookups, so both are refused.
        run(&[task("a").replace(r#""env":{}"#, r#""env":{"PATH":"/opt/tools/bin"}"#)]),
        run(&[task("a").replace(r#""env":{}"#, r#""env":{"HOME":"/home/fake"}"#)]),
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

    // Controls: timeout_ms of 1 and MAX_SAFE_INTEGER are accepted,
    // and so is a plain task with no timeout. A task id with dots
    // and dashes inside it is a single component and is accepted.
    assert!(validate(&run(&[task_with("a", r#""timeout_ms":1"#)])).is_ok());
    assert!(validate(&run(&[task_with("a", r#""timeout_ms":9007199254740991"#)])).is_ok());
    assert!(validate(&run(&[task("a")])).is_ok());
    assert!(validate(&run(&[task("a.b-c_d")])).is_ok());
    assert!(validate(r#"{"tasks":[]}"#).is_ok());
}

#[test]
fn fault_duplicate_input() {
    // The same input declared twice in one task (audit finding
    // F5). Outputs reject this shape under check 4; inputs now
    // reject it in check 5, after path validation of the first
    // occurrence.
    let adjacent = task("a").replace(r#""inputs":[]"#, r#""inputs":["in.txt","in.txt"]"#);
    assert_eq!(
        validate(&run(&[adjacent])),
        Err(DagError::DuplicateInput {
            task_id: "a".to_string(),
            path: "in.txt".to_string()
        })
    );

    // The duplicate need not be adjacent in the declared list.
    let separated = task("a").replace(r#""inputs":[]"#, r#""inputs":["a.txt","b.txt","a.txt"]"#);
    assert_eq!(
        validate(&run(&[separated])),
        Err(DagError::DuplicateInput {
            task_id: "a".to_string(),
            path: "a.txt".to_string()
        })
    );

    // Control: distinct inputs in one task validate, and the same
    // input path in two different tasks is ordinary fan-in.
    let distinct = task("a").replace(r#""inputs":[]"#, r#""inputs":["a.txt","b.txt"]"#);
    assert!(validate(&run(&[distinct])).is_ok());
    let fan_in_a = task("a").replace(r#""inputs":[]"#, r#""inputs":["shared.txt"]"#);
    let fan_in_b = task("b").replace(r#""inputs":[]"#, r#""inputs":["shared.txt"]"#);
    assert!(validate(&run(&[fan_in_a, fan_in_b])).is_ok());
}

#[test]
fn fault_case_colliding_paths() {
    // Two outputs in different tasks that differ only in letter
    // case (INV-DAG-7). On a case-insensitive filesystem the pair
    // is one file, so validation refuses it everywhere, naming
    // both paths and both tasks. Before the folded comparison
    // existed this spec validated, so this case fails on the
    // pre-rule behaviour.
    let upper = task("a").replace(r#""outputs":[]"#, r#""outputs":["Report.txt"]"#);
    let lower = task("b").replace(r#""outputs":[]"#, r#""outputs":["report.txt"]"#);
    assert_eq!(
        validate(&run(&[upper, lower])),
        Err(DagError::CaseCollidingOutput {
            first_path: "Report.txt".to_string(),
            path: "report.txt".to_string(),
            first_task: "a".to_string(),
            second_task: "b".to_string()
        })
    );

    // The same refusal inside one task's own outputs.
    let both = task("a").replace(r#""outputs":[]"#, r#""outputs":["A.txt","a.txt"]"#);
    assert_eq!(
        validate(&run(&[both])),
        Err(DagError::CaseCollidingOutput {
            first_path: "A.txt".to_string(),
            path: "a.txt".to_string(),
            first_task: "a".to_string(),
            second_task: "a".to_string()
        })
    );

    // The fold runs over the whole path, so a collision through
    // a directory component alone is refused as well.
    let dir_upper = task("a").replace(r#""outputs":[]"#, r#""outputs":["Data/out.txt"]"#);
    let dir_lower = task("b").replace(r#""outputs":[]"#, r#""outputs":["data/out.txt"]"#);
    assert_eq!(
        validate(&run(&[dir_upper, dir_lower])),
        Err(DagError::CaseCollidingOutput {
            first_path: "Data/out.txt".to_string(),
            path: "data/out.txt".to_string(),
            first_task: "a".to_string(),
            second_task: "b".to_string()
        })
    );

    // Two inputs in one task that differ only in letter case
    // share one materialization directory and are refused the
    // same way, naming the task and both paths.
    let inputs = task("a").replace(r#""inputs":[]"#, r#""inputs":["In.txt","in.txt"]"#);
    assert_eq!(
        validate(&run(&[inputs])),
        Err(DagError::CaseCollidingInput {
            task_id: "a".to_string(),
            first_path: "In.txt".to_string(),
            path: "in.txt".to_string()
        })
    );

    // Control: fold-equal inputs in different tasks validate,
    // because each task materializes its own inputs directory.
    let in_a = task("a").replace(r#""inputs":[]"#, r#""inputs":["Same.txt"]"#);
    let in_b = task("b").replace(r#""inputs":[]"#, r#""inputs":["same.txt"]"#);
    assert!(validate(&run(&[in_a, in_b])).is_ok());

    // Control: outputs that differ in more than letter case
    // validate.
    let one = task("a").replace(r#""outputs":[]"#, r#""outputs":["a.txt"]"#);
    let two = task("b").replace(r#""outputs":[]"#, r#""outputs":["a2.txt"]"#);
    assert!(validate(&run(&[one, two])).is_ok());
}

#[test]
fn fault_trailing_colliding_paths() {
    // Two outputs of different tasks that differ only in a
    // trailing space name one file on Windows, where trailing
    // spaces and dots in a component are stripped (INV-DAG-8).
    let first = task("a").replace(r#""outputs":[]"#, r#""outputs":["out.txt "]"#);
    let second = task("b").replace(r#""outputs":[]"#, r#""outputs":["out.txt"]"#);
    assert_eq!(
        validate(&run(&[first, second])),
        Err(DagError::TrailingCollidingOutput {
            first_path: "out.txt ".to_string(),
            path: "out.txt".to_string(),
            first_task: "a".to_string(),
            second_task: "b".to_string()
        })
    );

    // A trailing dot across one task's own two outputs.
    let pair = task("a").replace(r#""outputs":[]"#, r#""outputs":["report.","report"]"#);
    assert_eq!(
        validate(&run(&[pair])),
        Err(DagError::TrailingCollidingOutput {
            first_path: "report.".to_string(),
            path: "report".to_string(),
            first_task: "a".to_string(),
            second_task: "a".to_string()
        })
    );

    // A collision through a directory component alone.
    let left = task("a").replace(r#""outputs":[]"#, r#""outputs":["dir /f.txt"]"#);
    let right = task("b").replace(r#""outputs":[]"#, r#""outputs":["dir/f.txt"]"#);
    assert_eq!(
        validate(&run(&[left, right])),
        Err(DagError::TrailingCollidingOutput {
            first_path: "dir /f.txt".to_string(),
            path: "dir/f.txt".to_string(),
            first_task: "a".to_string(),
            second_task: "b".to_string()
        })
    );

    // A case difference and a trailing space together still
    // name one file on Windows.
    let upper = task("a").replace(r#""outputs":[]"#, r#""outputs":["OUT.txt "]"#);
    let lower = task("b").replace(r#""outputs":[]"#, r#""outputs":["out.txt"]"#);
    assert_eq!(
        validate(&run(&[upper, lower])),
        Err(DagError::TrailingCollidingOutput {
            first_path: "OUT.txt ".to_string(),
            path: "out.txt".to_string(),
            first_task: "a".to_string(),
            second_task: "b".to_string()
        })
    );

    // Two inputs in one task that differ only in a trailing
    // dot are refused the same way.
    let inputs = task("a").replace(r#""inputs":[]"#, r#""inputs":["in.txt.","in.txt"]"#);
    assert_eq!(
        validate(&run(&[inputs])),
        Err(DagError::TrailingCollidingInput {
            task_id: "a".to_string(),
            first_path: "in.txt.".to_string(),
            path: "in.txt".to_string()
        })
    );

    // Control: a lone output carrying a trailing dot
    // validates, because the refusal needs a pair.
    let lone = task("a").replace(r#""outputs":[]"#, r#""outputs":["solo."]"#);
    assert!(validate(&run(&[lone])).is_ok());

    // Control: outputs that differ in more than the trailing
    // characters validate.
    let apart = task("a").replace(r#""outputs":[]"#, r#""outputs":["a.txt","a.txt.bak"]"#);
    assert!(validate(&run(&[apart])).is_ok());
}

#[test]
fn fault_case_colliding_task_ids() {
    // Two task ids that differ only in letter case share one
    // sandbox directory on a case-insensitive filesystem,
    // because each task's sandbox is named by its id
    // (INV-DAG-9).
    assert_eq!(
        validate(&run(&[task("Build"), task("build")])),
        Err(DagError::CaseCollidingTaskId {
            first_id: "Build".to_string(),
            task_id: "build".to_string()
        })
    );

    // Control: ids that differ in more than letter case
    // validate.
    assert!(validate(&run(&[task("Build"), task("build2")])).is_ok());
}

#[test]
fn fault_trailing_colliding_task_ids() {
    // Two task ids that differ only in trailing spaces or
    // dots name one sandbox directory on Windows, because
    // Win32 strips those characters from a path component
    // and each task's sandbox is named by its id
    // (INV-DAG-10).
    assert_eq!(
        validate(&run(&[task("build"), task("build ")])),
        Err(DagError::TrailingCollidingTaskId {
            first_id: "build".to_string(),
            task_id: "build ".to_string()
        })
    );
    assert_eq!(
        validate(&run(&[task("build"), task("build.")])),
        Err(DagError::TrailingCollidingTaskId {
            first_id: "build".to_string(),
            task_id: "build.".to_string()
        })
    );

    // The comparison folds case as well: ids differing in
    // both letter case and trailing characters are refused
    // under this invariant, because check 2 reaches the
    // folded comparison first only for pure case pairs.
    assert_eq!(
        validate(&run(&[task("Build"), task("build ")])),
        Err(DagError::TrailingCollidingTaskId {
            first_id: "Build".to_string(),
            task_id: "build ".to_string()
        })
    );

    // Control: ids that differ in more than trailing spaces,
    // dots, and letter case validate.
    assert!(validate(&run(&[task("build"), task("build 2")])).is_ok());
}
