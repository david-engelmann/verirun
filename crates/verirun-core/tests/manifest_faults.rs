#![forbid(unsafe_code)]
//! Manifest parse fault oracles for the duplicate rejections
//! (audit finding F3).
//!
//! No run can produce two records for one task id, or two
//! artifacts at one path in a record, but the value form could
//! carry both until Round 9. A consumer keyed by task id would
//! silently drop the shadowed record, so the parse rejects the
//! duplicates outright.

use verirun_core::digest::{Digest, TaskStatus};
use verirun_core::error::ManifestError;
use verirun_core::manifest::{ArtifactRecord, Manifest, TaskRecord};
use verirun_core::value::Value;

const fn digest(byte: u8) -> Digest {
    Digest::from_bytes([byte; 32])
}

fn record_with_artifact() -> TaskRecord {
    TaskRecord::new(
        "only".to_string(),
        digest(1),
        TaskStatus::Succeeded,
        Some(0),
        digest(2),
        digest(3),
        vec![ArtifactRecord {
            path: "out.txt".to_string(),
            digest: digest(4),
            bytes: 5,
            executable: false,
        }],
    )
}

fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    match value {
        Value::Object(pairs) => pairs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, item)| item)
            .expect("field present"),
        _ => panic!("not an object"),
    }
}

fn set_field(value: &mut Value, name: &str, replacement: Value) {
    match value {
        Value::Object(pairs) => {
            for (key, item) in pairs.iter_mut() {
                if key == name {
                    *item = replacement;
                    return;
                }
            }
            panic!("field absent");
        }
        _ => panic!("not an object"),
    }
}

#[test]
fn fault_manifest_rejects_duplicate_task_ids() {
    let record = record_with_artifact().to_value();
    let manifest = Value::Object(vec![
        (
            "format".to_string(),
            Value::String("verirun-manifest/1".to_string()),
        ),
        ("merkle_root".to_string(), Value::String(digest(9).to_hex())),
        (
            "tasks".to_string(),
            Value::Array(vec![record.clone(), record]),
        ),
    ]);
    assert_eq!(
        Manifest::from_value(&manifest),
        Err(ManifestError::DuplicateTaskId {
            task_id: "only".to_string()
        })
    );
}

#[test]
fn fault_record_rejects_duplicate_artifact_paths() {
    let mut value = record_with_artifact().to_value();
    let artifact = match field(&value, "artifacts") {
        Value::Array(items) => items[0].clone(),
        _ => panic!("artifacts not an array"),
    };
    set_field(
        &mut value,
        "artifacts",
        Value::Array(vec![artifact.clone(), artifact]),
    );
    assert_eq!(
        TaskRecord::from_value(&value),
        Err(ManifestError::DuplicateArtifactPath {
            task_id: "only".to_string(),
            path: "out.txt".to_string()
        })
    );
}

#[test]
fn manifest_and_record_without_duplicates_parse() {
    let record = record_with_artifact();
    let parsed = TaskRecord::from_value(&record.to_value()).expect("record parses");
    assert_eq!(parsed, record);
    let manifest = Value::Object(vec![
        (
            "format".to_string(),
            Value::String("verirun-manifest/1".to_string()),
        ),
        ("merkle_root".to_string(), Value::String(digest(9).to_hex())),
        ("tasks".to_string(), Value::Array(vec![record.to_value()])),
    ]);
    let parsed = Manifest::from_value(&manifest).expect("manifest parses");
    assert_eq!(parsed.tasks.len(), 1);
}

// The construction paths hold the same invariants by panic
// (audit finding F25): a duplicate handed to a builder is a
// programming error, where the same shape handed to a parser
// is data and earns an error. A minimal factory stands in for
// the real one; the panic fires before any hashing happens.

use verirun_core::digest::{DomainTag, HasherFactory, StreamingHasher};

struct NullHasher;

impl StreamingHasher for NullHasher {
    fn update(&mut self, _bytes: &[u8]) {}

    fn finalize(self: Box<Self>) -> Digest {
        Digest::from_bytes([0u8; 32])
    }
}

#[derive(Clone, Default)]
struct NullFactory;

impl HasherFactory for NullFactory {
    fn new_hasher(&self, _tag: DomainTag) -> Box<dyn StreamingHasher> {
        Box::new(NullHasher)
    }
}

#[test]
#[should_panic(expected = "duplicate artifact path")]
fn built_record_panics_on_duplicate_artifact_paths() {
    let artifact = ArtifactRecord {
        path: "out.txt".to_string(),
        digest: digest(4),
        bytes: 5,
        executable: false,
    };
    let _record = TaskRecord::new(
        "only".to_string(),
        digest(1),
        TaskStatus::Succeeded,
        Some(0),
        digest(2),
        digest(3),
        vec![artifact.clone(), artifact],
    );
}

#[test]
#[should_panic(expected = "duplicate task id")]
fn built_manifest_panics_on_duplicate_task_ids() {
    let record = record_with_artifact();
    let _manifest = Manifest::build(&NullFactory, vec![record.clone(), record]);
}

#[test]
fn built_manifest_round_trips_through_the_value_form() {
    let first = record_with_artifact();
    let mut second = record_with_artifact();
    second.task_id = "other".to_string();
    let built = Manifest::build(&NullFactory, vec![second, first]);
    let parsed = Manifest::from_value(&built.to_value()).expect("a built manifest parses");
    assert_eq!(parsed.tasks.len(), 2);
    assert_eq!(parsed.tasks[0].task_id, "only");
    assert_eq!(parsed.tasks[1].task_id, "other");
}
