//! Seed reporting property (INV-CLI-3;
//! docs/ARCHITECTURE.md section 12.1).
//!
//! Every command prints its active seed to stderr, where the
//! seed comes from `--seed`, else the `SIM_SEED` environment
//! variable, else 0. The same seed in separate processes
//! produces byte-identical traces; the flag and the environment
//! variable are interchangeable sources; a different seed is
//! visible in the trace. The property drives the cheapest
//! command (`gc --dry-run` on an empty store) so the volume
//! goes to seed variation, not to work.

#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use proptest::prelude::*;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "verirun-seed-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Runs `gc --dry-run` against a fresh store in `dir`, with
/// the seed supplied by flag, by environment, or not at all.
fn trace(dir: &Path, seed_flag: Option<u64>, seed_env: Option<u64>) -> Output {
    let cas = dir.join("cas");
    let mut command = Command::new(env!("CARGO_BIN_EXE_verirun"));
    command
        .args(["gc", "--cas", cas.to_str().expect("utf8 path"), "--dry-run"])
        .env_remove("SIM_SEED");
    if let Some(seed) = seed_flag {
        command.args(["--seed", &seed.to_string()]);
    }
    if let Some(seed) = seed_env {
        command.env("SIM_SEED", seed.to_string());
    }
    command.output().expect("spawn verirun")
}

proptest! {
    #[test]
    fn prop_seed_trace_reproducibility(seed in any::<u64>()) {
        let dir = fresh_dir();
        let by_flag = trace(&dir, Some(seed), None);
        prop_assert!(by_flag.status.success());
        // A second process at the same seed: byte-identical
        // stdout and stderr.
        let again = trace(&dir, Some(seed), None);
        prop_assert_eq!(&by_flag.stdout, &again.stdout);
        prop_assert_eq!(&by_flag.stderr, &again.stderr);
        // The environment source produces the identical trace.
        let by_env = trace(&dir, None, Some(seed));
        prop_assert_eq!(&by_flag.stdout, &by_env.stdout);
        prop_assert_eq!(&by_flag.stderr, &by_env.stderr);
        // The seed is actually printed, and a different seed
        // changes the trace.
        let stderr = String::from_utf8(by_flag.stderr.clone()).expect("utf8 stderr");
        prop_assert!(
            stderr.contains(&format!("seed: {seed}")),
            "stderr names the active seed: {stderr}"
        );
        let other = trace(&dir, Some(seed.wrapping_add(1)), None);
        prop_assert_ne!(&by_flag.stderr, &other.stderr);
        fs::remove_dir_all(&dir).ok();
    }
}
