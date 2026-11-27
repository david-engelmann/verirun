# verirun

<p>
  <img class="theme-light-only" src="docs/media/verirun-lockup-light.svg" alt="verirun" height="50">
  <img class="theme-dark-only" src="docs/media/verirun-lockup-dark.svg" alt="verirun" height="50">
</p>

`verirun` is a deterministic task-DAG runner, crash-safe SHA-256 content-addressed store, and RFC 8785 (JCS) Merkle manifest verifier written in Rust, paired with a zero-dependency Python 3.10+ SDK (`verirun.PipelineBuilder`).

<p>
  <img src="docs/media/money-shot.gif" alt="verirun hermetic task execution, verified by-exec cache hit, manifest diff, and exit-2 single-byte tamper conviction" width="860">
</p>

## Execution & Crash-Safe Store Pipeline

<p>
  <img class="theme-light-only" src="docs/media/execution-cas-pipeline-light.svg" alt="How verirun validates JSON task graphs upfront, resolves verified by-exec cache hits, runs hermetic sandboxes on misses, and commits blobs via the 4-step atomic CAS protocol" width="880">
  <img class="theme-dark-only" src="docs/media/execution-cas-pipeline-dark.svg" alt="How verirun validates JSON task graphs upfront, resolves verified by-exec cache hits, runs hermetic sandboxes on misses, and commits blobs via the 4-step atomic CAS protocol" width="880">
</p>

- **Upfront validation before side effects (`exit 3`).** Strict JSON parsing, IEEE-754 safe-integer checks (`<= 2^53 - 1`), Kahn cycle detection, case-folded path collision checks, and reserved `env` key checks complete before touching `--cas` or `--work` (`INV-LAT-4`).
- **Verified `by-exec` cache vs hermetic sandbox.** Cache hits verify every referenced blob and recorded byte length (`status: "cached"`, `INV-RUN-7`); misses execute in a scrubbed environment (`LC_ALL=C`, `TZ=UTC`, `SOURCE_DATE_EPOCH=0`) inside a Unix `ProcessGroup` or Windows `JobObject` (`INV-RUN-4`) with handle-relative no-follow output collection (`INV-SBX-3`).
- **4-step atomic CAS commit.** Blobs stream in `64 KiB` chunks into `<cas>/tmp/`, `sync_all`, become read-only (`0o444`), atomically rename to `objects/<xx>/<digest>`, and sync the shard directory (`INV-CAS-1..8`).

## Domain-Separated Merkle Tamper Conviction

<p>
  <img class="theme-light-only" src="docs/media/merkle-domain-tree-light.svg" alt="Exact domain-separated SHA-256 framing tree in verirun-core from raw blob streams up to the canonical RFC 8785 manifest and exit-2 tamper conviction" width="880">
  <img class="theme-dark-only" src="docs/media/merkle-domain-tree-dark.svg" alt="Exact domain-separated SHA-256 framing tree in verirun-core from raw blob streams up to the canonical RFC 8785 manifest and exit-2 tamper conviction" width="880">
</p>

Every hash prefixes its exact `verirun/v1/{blob,input,env,exec,artifact,task,node,empty}` tag plus `0x00`. `verirun verify` checks the manifest digest, canonical RFC 8785 encoding, Merkle root, and every referenced blob and byte count, convicting single-byte corruption with exit code `2` (`INV-MAN-3..5`).

## Chapters in This Book

- **[Quickstart](./quickstart.md):** Install `verirun` and run your first Python `PipelineBuilder` or CLI graph.
- **[Step-by-Step Tutorial](docs/TUTORIAL.md):** Cold execution, warm `by-exec` cache rerun, `verify`, `show`, `diff`, `pin`, and `gc`.
- **[CLI and Python SDK](./cli-and-python.md):** Command flags, `--json` output schemas, four-code exit contract (`0`, `1`, `2`, `3`), and Python API types.
- **[Architecture](docs/ARCHITECTURE.md) & [Invariants](docs/INVARIANTS.md):** Normative byte framing, store state machine, and 1-to-1 invariant test matrix.
- **[Comparison](docs/COMPARISON.md) & [FAQ](docs/FAQ.md):** Choosing between `verirun`, `make`, `Bazel`/`Nix`, containers, and custom eval scripts.
