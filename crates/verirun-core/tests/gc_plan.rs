//! Pure planning tests for garbage collection (INV-GC-1,
//! INV-GC-2) and manifest comparison (INV-CLI-4).
//!
//! The matrix rows for these invariants name the store and CLI
//! fault suites as their enforcement; these tests pin the pure
//! core rules those suites are built on: the reachable set's
//! composition, the strict grace comparison, and the diff's
//! compared fields. Every test here must be able to fail: each
//! asserts a set or a list computed from distinct inputs, never
//! a tautology over one value.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use verirun_core::cas::{reachable_objects, sweep_candidates, temp_is_stale};
use verirun_core::digest::{Digest, TaskStatus};
use verirun_core::manifest::{
    ArtifactRecord, Manifest, TaskDifference, TaskRecord, diff_manifests,
};

const fn digest(byte: u8) -> Digest {
    Digest::from_bytes([byte; 32])
}

fn artifact(path: &str, content: u8) -> ArtifactRecord {
    ArtifactRecord {
        path: path.to_string(),
        digest: digest(content),
        bytes: 3,
        executable: false,
    }
}

fn record(
    task_id: &str,
    exec: u8,
    status: TaskStatus,
    exit_code: Option<i32>,
    stdout: u8,
    stderr: u8,
    artifacts: Vec<ArtifactRecord>,
) -> TaskRecord {
    TaskRecord::new(
        task_id.to_string(),
        digest(exec),
        status,
        exit_code,
        digest(stdout),
        digest(stderr),
        artifacts,
    )
}

const fn manifest(tasks: Vec<TaskRecord>) -> Manifest {
    Manifest {
        tasks,
        merkle_root: digest(250),
    }
}

#[test]
fn reachable_set_composition() {
    // Manifest A names artifacts 10 and 11, logs 12 and 13, and
    // its record's exec digest has a by-exec entry naming the
    // record blob 40. Manifest B names artifact 20 and logs 21
    // and 22; its record has no entry.
    let a = manifest(vec![record(
        "a",
        30,
        TaskStatus::Succeeded,
        Some(0),
        12,
        13,
        vec![artifact("out/a.txt", 10), artifact("out/b.txt", 11)],
    )]);
    let b = manifest(vec![record(
        "b",
        31,
        TaskStatus::Succeeded,
        Some(0),
        21,
        22,
        vec![artifact("out/c.txt", 20)],
    )]);
    let mut by_exec = BTreeMap::new();
    by_exec.insert(digest(30), digest(40));
    // An entry for an execution digest no pinned record uses
    // contributes nothing.
    by_exec.insert(digest(99), digest(41));

    let reachable = reachable_objects(&[(digest(1), a), (digest(2), b)], &by_exec);
    let expected: BTreeSet<Digest> = [
        digest(1),
        digest(2),
        digest(10),
        digest(11),
        digest(12),
        digest(13),
        digest(20),
        digest(21),
        digest(22),
        digest(40),
    ]
    .into_iter()
    .collect();
    assert_eq!(reachable, expected);
}

#[test]
fn sweep_candidates_respect_reachability_and_strict_grace() {
    let grace = 3600;
    let mut objects = BTreeMap::new();
    objects.insert(digest(1), grace - 1); // unreachable but young
    objects.insert(digest(2), grace); // unreachable, exactly at the threshold
    objects.insert(digest(3), grace + 1); // unreachable and old: candidate
    objects.insert(digest(4), grace + 10_000); // old but reachable: kept
    let reachable: BTreeSet<Digest> = BTreeSet::from([digest(4)]);
    assert_eq!(
        sweep_candidates(&objects, &reachable, grace),
        vec![digest(3)]
    );
    // Grace zero makes every unreachable object a candidate,
    // however young, and still never touches reachable ones.
    assert_eq!(
        sweep_candidates(&objects, &reachable, 0),
        vec![digest(1), digest(2), digest(3)]
    );
    // Nothing stored, nothing swept.
    assert_eq!(
        sweep_candidates(&BTreeMap::new(), &reachable, grace),
        Vec::<Digest>::new()
    );
}

#[test]
fn temp_staleness_uses_the_same_strict_comparison() {
    assert!(!temp_is_stale(3599, 3600));
    assert!(!temp_is_stale(3600, 3600));
    assert!(temp_is_stale(3601, 3600));
    assert!(temp_is_stale(1, 0));
    assert!(!temp_is_stale(0, 0));
}

#[test]
fn diff_reports_presence_status_and_digest_changes_in_id_order() {
    let base = record(
        "same",
        30,
        TaskStatus::Succeeded,
        Some(0),
        12,
        13,
        vec![artifact("out/a.txt", 10)],
    );
    let first = manifest(vec![
        record("gone", 31, TaskStatus::Succeeded, Some(0), 21, 22, vec![]),
        base.clone(),
        record("status", 32, TaskStatus::Succeeded, Some(0), 23, 24, vec![]),
        record(
            "logs",
            33,
            TaskStatus::Succeeded,
            Some(0),
            25,
            26,
            vec![artifact("out/x", 27)],
        ),
    ]);
    let second = manifest(vec![
        record("new", 34, TaskStatus::Succeeded, Some(0), 41, 42, vec![]),
        base,
        record("status", 32, TaskStatus::Failed, Some(1), 23, 24, vec![]),
        record(
            "logs",
            33,
            TaskStatus::Succeeded,
            Some(0),
            25,
            99,
            vec![artifact("out/x", 27)],
        ),
    ]);
    let diffs = diff_manifests(&first, &second);
    let ids: Vec<&str> = diffs.iter().map(|diff| diff.task_id.as_str()).collect();
    assert_eq!(ids, ["gone", "logs", "new", "status"]);
    assert_eq!(diffs[0].difference, TaskDifference::OnlyInFirst);
    assert_eq!(diffs[2].difference, TaskDifference::OnlyInSecond);
    match diffs[3].difference {
        TaskDifference::Changed(fields) => {
            assert!(fields.status, "status changed");
            assert!(fields.exit_code, "0 to 1 is an exit code change");
            assert!(!fields.artifacts);
            assert!(!fields.stdout);
            assert!(!fields.stderr);
        }
        other => panic!("expected a change, got {other:?}"),
    }
    match diffs[1].difference {
        TaskDifference::Changed(fields) => {
            assert!(fields.stderr, "the stderr digest changed");
            assert!(!fields.status);
            assert!(!fields.artifacts);
            assert!(!fields.stdout);
        }
        other => panic!("expected a change, got {other:?}"),
    }
}

#[test]
fn diff_of_identical_manifests_is_empty_and_exec_only_changes_do_not_count() {
    let one = manifest(vec![record(
        "a",
        30,
        TaskStatus::Succeeded,
        Some(0),
        12,
        13,
        vec![artifact("out/a.txt", 10)],
    )]);
    assert_eq!(diff_manifests(&one, &one), Vec::new());
    // Same compared fields, different execution digest: the
    // runs are equivalent under section 9.4, whose compared
    // fields are effects, not causes.
    let two = manifest(vec![record(
        "a",
        31,
        TaskStatus::Succeeded,
        Some(0),
        12,
        13,
        vec![artifact("out/a.txt", 10)],
    )]);
    assert_eq!(diff_manifests(&one, &two), Vec::new());
}
