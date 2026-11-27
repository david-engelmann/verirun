# Fuzzing verirun-core

This directory is an out-of-workspace `cargo-fuzz` package targeting the pure parser, canonical encoder, DAG validator, and manifest parser in `verirun-core`. Keeping `fuzz/` in its own `[workspace]` isolates `libfuzzer-sys` from the main workspace's `Cargo.lock` and `cargo-deny` policy while compiling every target on stable Rust in `.github/workflows/ci.yml`.

## Targets

| Target | Entry point | Invariants exercised |
| --- | --- | --- |
| `canonical_json` | `fuzz_targets/canonical_json.rs` | Feeds arbitrary byte slices to `verirun_core::parse::parse`. Whenever parsing succeeds, encodes via `canonical::to_vec`, re-parses the canonical bytes, asserts value equality, and asserts fixed-point byte identity (`INV-CAN-1` to `INV-CAN-7`). |
| `dag_spec` | `fuzz_targets/dag_spec.rs` | Feeds arbitrary byte slices through `parse::parse` and `dag::validate_run_spec`, verifying zero panics on hostile schemas, deterministic validation outcomes (`INV-DAG-5`), and `ValidatedDag` lookups. |
| `manifest_parse` | `fuzz_targets/manifest_parse.rs` | Feeds arbitrary byte slices through `Digest::from_hex`, `ArtifactRecord::from_value`, `TaskRecord::from_value`, and `Manifest::from_value`, plus canonical re-encoding, Merkle root recomputation, and self-diffing on valid manifests (`INV-MAN-4`, `INV-MAN-6`). |

## Compile-checking on stable Rust (CI gate)

```sh
CARGO_TARGET_DIR=./target cargo check --manifest-path fuzz/Cargo.toml --locked --bins
```

## Running fuzzing campaigns locally

Install `cargo-fuzz` and a nightly toolchain (required by SanitizerCoverage instrumentation), then pass the committed seed corpus for the chosen target:

```sh
cargo +nightly fuzz run canonical_json fuzz/seeds/canonical_json -- -max_total_time=60
cargo +nightly fuzz run dag_spec fuzz/seeds/dag_spec -- -max_total_time=60
cargo +nightly fuzz run manifest_parse fuzz/seeds/manifest_parse -- -max_total_time=60
```
