//! Handle relative traversal: the checked object is the
//! opened object (perfection programme workstream W3, audit
//! finding F11).
//!
//! The race case at the bottom is the slice's reason to
//! exist. A swapper thread flips one path component between
//! a real directory and a link to a decoy tree while the
//! traversal runs; every traversal must yield the real bytes
//! or an error, never the decoy's bytes. A walk that checks
//! components by path and opens by path afterwards loses
//! this race whenever the swap lands in its window.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use verirun_platform::{DirHandle, TraversalError};

/// A fresh empty directory for one test.
fn fresh_root(label: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("verirun-platform-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("root creates");
    root
}

/// Creates a link at `link` naming `target`: a symbolic link
/// on Unix, a directory junction on Windows (the unprivileged
/// reparse point).
#[cfg(unix)]
fn make_link(link: &Path, target: &Path) {
    std::os::unix::fs::symlink(target, link).expect("link creates");
}

/// Creates a link at `link` naming `target`: a symbolic link
/// on Unix, a directory junction on Windows (the unprivileged
/// reparse point).
#[cfg(windows)]
fn make_link(link: &Path, target: &Path) {
    let status = std::process::Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            &link.display().to_string(),
            &target.display().to_string(),
        ])
        .status()
        .expect("mklink runs");
    assert!(status.success(), "mklink succeeds");
}

/// Removes a link created by [`make_link`] without touching
/// its target.
#[cfg(unix)]
fn remove_link(link: &Path) {
    fs::remove_file(link).expect("link removes");
}

/// Removes a link created by [`make_link`] without touching
/// its target.
#[cfg(windows)]
fn remove_link(link: &Path) {
    fs::remove_dir(link).expect("link removes");
}

/// The tolerant link creation the race swapper uses: failure
/// is a normal mid-race outcome there.
#[cfg(unix)]
fn try_make_link(link: &Path, target: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

/// The tolerant link creation the race swapper uses: failure
/// is a normal mid-race outcome there.
#[cfg(windows)]
fn try_make_link(link: &Path, target: &Path) -> bool {
    std::process::Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            &link.display().to_string(),
            &target.display().to_string(),
        ])
        .status()
        .is_ok_and(|status| status.success())
}

/// The tolerant link removal the race swapper uses.
#[cfg(unix)]
fn try_remove_link(link: &Path) {
    let _ = fs::remove_file(link);
}

/// The tolerant link removal the race swapper uses.
#[cfg(windows)]
fn try_remove_link(link: &Path) {
    let _ = fs::remove_dir(link);
}

/// Reads the file at `components` below `root` through held
/// handles only.
fn read_through(root: &Path, components: &[&str]) -> Result<Vec<u8>, TraversalError> {
    let mut dir = DirHandle::open_root(root)?;
    for (index, component) in components.iter().enumerate() {
        if index + 1 == components.len() {
            let mut file = dir.open_file(component)?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            return Ok(bytes);
        }
        dir = dir.open_dir(component)?;
    }
    Err(TraversalError::NotRegularFile)
}

#[test]
fn nested_file_reads_back_through_handles() {
    let root = fresh_root("nested");
    fs::create_dir_all(root.join("a/b")).expect("dirs create");
    fs::write(root.join("a/b/out.txt"), b"nested bytes").expect("file writes");
    let bytes = read_through(&root, &["a", "b", "out.txt"]).expect("traversal reads");
    assert_eq!(bytes, b"nested bytes");
}

#[test]
fn missing_component_is_not_found() {
    let root = fresh_root("missing");
    let missing = read_through(&root, &["nope", "out.txt"]);
    assert!(matches!(missing, Err(TraversalError::NotFound)));
    fs::create_dir_all(root.join("a")).expect("dir creates");
    let missing_file = read_through(&root, &["a", "nope.txt"]);
    assert!(matches!(missing_file, Err(TraversalError::NotFound)));
}

#[test]
fn directory_as_the_final_component_is_not_a_regular_file() {
    let root = fresh_root("dirfinal");
    fs::create_dir_all(root.join("a/dir")).expect("dirs create");
    let result = read_through(&root, &["a", "dir"]);
    assert!(matches!(result, Err(TraversalError::NotRegularFile)));
}

#[test]
fn linked_component_is_rejected() {
    let root = fresh_root("linked");
    fs::create_dir_all(root.join("decoy")).expect("dir creates");
    fs::write(root.join("decoy/out.txt"), b"decoy bytes").expect("file writes");
    make_link(&root.join("slot"), &root.join("decoy"));
    let result = read_through(&root, &["slot", "out.txt"]);
    assert!(
        matches!(result, Err(TraversalError::Link)),
        "a linked component is a link error, got {result:?}"
    );
}

#[test]
fn traversal_never_collects_the_decoy_during_a_swap() {
    let root = fresh_root("race");
    let slot = root.join("slot");
    let decoy = root.join("decoy");
    fs::create_dir_all(&slot).expect("slot creates");
    fs::write(slot.join("out.txt"), b"real bytes").expect("real file writes");
    fs::create_dir_all(&decoy).expect("decoy creates");
    fs::write(decoy.join("out.txt"), b"decoy bytes").expect("decoy file writes");

    let stop = Arc::new(AtomicBool::new(false));
    let swapper_stop = Arc::clone(&stop);
    let swapper_root = root.clone();
    let swapper = std::thread::spawn(move || {
        let slot = swapper_root.join("slot");
        let parked = swapper_root.join("slot-parked");
        let decoy = swapper_root.join("decoy");
        while !swapper_stop.load(Ordering::Relaxed) {
            // Flip the component to the link and back. Every
            // step may race the traversal; a failed step just
            // means the flip restarts from wherever it stopped,
            // so the swapper uses the tolerant forms.
            if fs::rename(&slot, &parked).is_ok() {
                if try_make_link(&slot, &decoy) {
                    try_remove_link(&slot);
                }
                let _ = fs::rename(&parked, &slot);
            }
        }
    });

    let mut successes = 0_usize;
    for _ in 0..400 {
        // A link error while the component is flipped, or a
        // not found while it is parked, are both the contract
        // working; only a successful read is checked, and it
        // must be the real bytes.
        if let Ok(bytes) = read_through(&root, &["slot", "out.txt"]) {
            successes += 1;
            assert_eq!(
                bytes, b"real bytes",
                "the traversal collected through a swapped component"
            );
        }
    }
    stop.store(true, Ordering::Relaxed);
    swapper.join().expect("swapper joins");
    assert!(successes > 0, "the good path was exercised");
}
