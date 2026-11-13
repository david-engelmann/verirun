//! Sandbox path fault oracles (INV-SBX-1, INV-SBX-2).
//!
//! Any `..` component is rejected, not only escaping ones, and
//! absolute spellings are rejected in every platform's spelling,
//! so the rules have no judgement calls.

use verirun_core::error::PathErrorKind;
use verirun_core::paths::validate_path;

#[test]
fn fault_parent_traversal_table() {
    for path in [
        "../escape",
        "a/../../escape",
        "a/../b",
        "..",
        "a/..",
        "a/b/../../../c",
    ] {
        let err = validate_path(path).expect_err("traversal must be rejected");
        assert_eq!(err.kind(), PathErrorKind::ParentTraversal, "path {path}");
        assert_eq!(err.declared(), path);
    }
}

#[test]
fn fault_absolute_path_table() {
    for path in ["/etc/passwd", "/", "/a/b", "C:\\Windows\\file", "c:/data"] {
        let err = validate_path(path).expect_err("absolute path must be rejected");
        assert_eq!(err.kind(), PathErrorKind::Absolute, "path {path}");
        assert_eq!(err.declared(), path);
    }

    // A backslash is a separator spelling of another platform and
    // is its own rejection class: a verirun path has exactly one
    // separator spelling.
    for path in ["a\\b", "dir\\file.txt"] {
        let err = validate_path(path).expect_err("backslash must be rejected");
        assert_eq!(err.kind(), PathErrorKind::BackslashSeparator, "path {path}");
    }

    // Accepted spellings normalize: dot and empty components are
    // removed and the rest join with single slashes.
    assert_eq!(validate_path("a/./b").expect("accepted").as_str(), "a/b");
    assert_eq!(validate_path("./x").expect("accepted").as_str(), "x");
    assert_eq!(validate_path("a//b").expect("accepted").as_str(), "a/b");
    assert_eq!(
        validate_path("plain.txt").expect("accepted").as_str(),
        "plain.txt"
    );

    // Empty and dot-only paths name no file.
    assert_eq!(
        validate_path("").expect_err("empty").kind(),
        PathErrorKind::Empty
    );
    assert_eq!(
        validate_path(".").expect_err("dot only").kind(),
        PathErrorKind::Empty
    );
    assert_eq!(
        validate_path("./.").expect_err("dots only").kind(),
        PathErrorKind::Empty
    );
}
