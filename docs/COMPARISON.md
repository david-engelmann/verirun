# How verirun compares

`verirun` occupies a deliberate point in the build and evaluation tool space: a single-binary, content-addressed task DAG runner with canonical RFC 8785 manifests, verified `by-exec` caching, and end-to-end SHA-256 tamper conviction, without requiring a daemon, container runtime, or bespoke DSL.

## Capability matrix

| Dimension | `make` / `just` | `Bazel` / `Buck2` / `Nix` | Container CI runners (`Argo`, Docker steps) | Ad-hoc Python eval scripts | `verirun` |
| --- | --- | --- | --- | --- | --- |
| Graph input format | `Makefile` / `justfile` DSL | `Starlark` / Nix expression language | YAML workflow CRDs or pipeline files | Imperative Python loops | Strict JSON spec or typed `verirun.PipelineBuilder` |
| Upfront graph and path validation | Partial (runtime shell errors common) | Yes | Partial (container failures surface at runtime) | Manual | Total upfront check before any sandbox or process starts (exit `3`) |
| Environment scrubbing | Inherits ambient shell by default | Hermetic sandbox / store derivation | Per-container image environment | Inherits ambient `os.environ` | Cleared environment; only declared keys plus `LC_ALL=C`, `TZ=UTC`, `SOURCE_DATE_EPOCH=0`, and sandbox `TMPDIR` |
| Cache invalidation key | File mtimes (`make`) or none (`just`) | Action digest / derivation hash | Layer cache or manual cache keys | Ad-hoc pickle or filename conventions | Domain-separated SHA-256 `exec_digest` over command, args, env, input digests, and output paths |
| Cache read verification | None | Store digest / daemon metadata | Image digest at pull time | Rare | Re-verifies every referenced blob and recorded byte size before serving a `cached` hit |
| Cryptographic run record | None | Build event stream / store paths | Logs and container exit statuses | Custom JSON or CSV summaries | Canonical RFC 8785 manifest blob + Merkle root verifiable offline via `verirun verify` |
| Post-hoc tamper conviction | None | Store repair / hash check | External artifact signing tools | Manual | Built-in `verirun verify` (`exit 2` on 1-byte flip, missing blob, or size mismatch) |
| Runtime footprint | System `make` or small binary | JVM server (`Bazel`), daemon, or Nix store | Container daemon / Kubernetes cluster | Python interpreter + packages | Standalone Rust binary (`std`, `sha2`, `clap`, `serde_json`, OS FFI seam) + zero-dep Python 3.10+ SDK |

## Detailed comparisons

### Command runners (`make`, `just`, shell scripts)

`make` and `just` excel at ergonomic developer shortcuts (`make test`, `make lint`). In fact, this repository uses `Makefile` as a thin developer entry point over Cargo and Python test suites.

Where `make` and shell scripts fall short is reproducible batch execution and auditability:

- `make` decides whether work is stale from filesystem modification timestamps, which break across `git checkout`, clock skew, and shared caches.
- Child commands inherit `PATH`, `HOME`, locale variables, credentials, and stray environment flags from the caller's terminal.
- Mutating an output directory in place leaves partial debris when a command fails or times out partway through.
- After a run finishes, no self-contained cryptographic record proves which exact input bytes produced an output file.

`verirun` complements `make`: keep `make` or `just` for interactive project shortcuts, and hand data preparation, benchmark suites, artifact generation, or evaluation DAGs to `verirun` when you need content-addressed caching and verifiable manifests.

### Hermetic monorepo build systems (`Bazel`, `Buck2`, `Nix`)

`Bazel`, `Buck2`, and `Nix` solve large-scale polyglot software builds with deep language rules, header/module dependency discovery, filesystem sandboxes, and distributed remote execution clusters.

Choose `Bazel`, `Buck2`, or `Nix` when you are building a multi-language monorepo with thousands of C++, Rust, Go, or Java targets and a remote build farm.

Choose `verirun` when:

- Your task graph is generated dynamically by an application, benchmark harness, data pipeline, or AI agent as plain JSON or via `verirun.PipelineBuilder`.
- You want a zero-daemon, relocatable content-addressed store on local disk (Linux, macOS, and Windows) that you can archive, inspect, diff (`verirun diff`), and verify (`verirun verify`) with one static-style CLI binary.
- You need strict RFC 8785 canonical JSON manifests and domain-separated Merkle roots whose byte layout is small enough to audit end to end in [`docs/ARCHITECTURE.md`](ARCHITECTURE.md).

### Container workflow engines (Docker steps, `Argo Workflows`, CI YAML)

Container runners package OS userlands so a workflow brings its own `/usr/bin` tools across machines. However, containers alone do not give you content-addressed task caching, deterministic environment variables inside the container, symlink-safe output collection, or cryptographic manifest diffs across runs.

`verirun` works cleanly *inside* a container or CI job: the container pins the OS toolchain image, while `verirun` schedules the task graph across CPU cores, caches unchanged steps in `--cas`, enforces per-task timeouts with process-group cleanup, and emits a verifiable Merkle manifest for the run.

### Ad-hoc Python evaluation and data pipelines

Evaluation harnesses, ML dataset builders, and reproducible research scripts often start as Python `subprocess.run` loops or `joblib` caches writing into `results/run_01/`. Over time, subtle bugs creep in:

- Changing a CLI flag or upstream input file silently mixes fresh outputs with stale files left in `results/` from an earlier run.
- Worker pool size or dictionary iteration order changes summary file bytes.
- Corrupted or truncated cached files are loaded without re-hashing.

`verirun`'s Python SDK (`pip install verirun`) replaces ad-hoc subprocess glue with `PipelineBuilder`, `run`, `verify`, `show`, `diff`, and `gc`, while enforcing total exit-code separation (`0` ok, `1` task failure or diff, `2` integrity failure, `3` invalid spec). See [`examples/reproducible_eval_pipeline.py`](../examples/reproducible_eval_pipeline.py) for a complete walkthrough.

## Explicit non-goals

`verirun` stays small and auditable by refusing scope that belongs in adjacent layers:

- **Not a distributed remote execution cluster.** `verirun` executes tasks on the local host across `--jobs` workers against a local filesystem `--cas`. You can copy or sync a store directory across machines and run `verirun verify` anywhere (`INV-MAN-2`), without running a network server.
- **Not a hostile-code containment VM.** `verirun` scrubs ambient environment variables (`INV-RUN-1`), enforces handle-relative no-follow traversal so symlinks and Windows reparse points cannot escape `outputs/` (`INV-SBX-3`), and terminates full process groups or Windows Job Objects on timeout (`INV-RUN-4`). It does not create Linux user/mount/network namespaces or hypervisor sandboxes; run untrusted third-party binaries inside a VM or container boundary.
- **Not a source-import scanner.** `verirun` executes the exact `inputs`, `outputs`, and `depends_on` edges declared in the JSON spec rather than parsing C headers or Python imports automatically.
- **Not a long-lived service supervisor.** Every `verirun run` validates a finite acyclic task graph, runs it to completion, and seals one immutable manifest.
