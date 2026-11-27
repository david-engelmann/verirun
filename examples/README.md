# Runnable verirun examples

Self-contained examples demonstrating `verirun` from both the typed Python 3.10+ SDK and the POSIX command line interface. Both scripts run inside temporary directories, clean up after themselves on exit, and execute on every pull request in `.github/workflows/ci.yml`.

## Prerequisites

Build the `verirun` binary from the workspace root (or install a release binary onto `PATH` / set `VERIRUN_BIN`):

```sh
cargo build --locked -p verirun-cli --bin verirun
```

## Examples in this directory

| Script | Surface | What it demonstrates |
| --- | --- | --- |
| `reproducible_eval_pipeline.py` | Python SDK (`verirun.PipelineBuilder`) | Builds a 3-task evaluation DAG (`prepare_dataset` -> `score_candidates` -> `summarize_report`), proves cold worker-count determinism (`jobs=1` vs `jobs=4`), exercises verified warm `by-exec` caching, runs a threshold-modified graph with `verirun.diff(...)`, and convicts a 1-byte blob corruption on an unpinned store copy with `VerirunError` (`exit_code == 2`). |
| `run_and_verify.sh` | CLI (`verirun --json`) | Stages a host input file and a 2-task JSON specification in a `mktemp -d` sandbox, then runs `verirun run --pin --json`, `verirun show --json`, `verirun verify --json`, and `verirun gc --dry-run --json`. |

## Running locally

```sh
python3 examples/reproducible_eval_pipeline.py
sh examples/run_and_verify.sh
```

Or run both together via `Makefile`:

```sh
make examples
```

## Crate-level Rust examples

For Rust library consumers embedding `verirun-core` or `verirun-store` directly:

```sh
cargo run --locked -p verirun-core --example canonical_and_digests
cargo run --locked -p verirun-store --example store_roundtrip
```
