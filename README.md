# verirun

verirun runs a batch of shell tasks and keeps an exact, checkable record of what the batch did. You describe the work once, as data. verirun checks the description, runs the tasks in the right order, stores every file the tasks read or wrote under a hash of its contents, and finishes by writing one manifest that ties the whole run together. Run the same batch again later, with a different number of workers or in a different folder, and the manifest comes out identical, byte for byte.

## What it does

- Takes a run description as JSON data. Each task names a program, its arguments, the files it reads, the files it writes, the tasks it depends on, and an optional time limit.
- Checks the description before anything runs. Missing dependencies, dependency cycles, two tasks claiming the same output file, reserved environment keys, and unsafe paths are all reported up front. Nothing runs until the description is clean.
- Runs each task in its own fresh working folder with a cleaned environment. Nothing leaks in from your shell. The environment holds only the values the task declares plus a small fixed set that never changes.
- Stores content by hash. Inputs, outputs, and the captured output and error streams all go into a store folder, named by the SHA-256 hash of their bytes. The same bytes are stored once, however many tasks use them.
- Writes a manifest at the end of the run. The manifest holds one record per task with the hashes and byte lengths of everything the task consumed and produced, plus one root hash that covers the whole run.
- Reuses finished work. When a task comes up whose program, inputs, and environment match a stored success, the stored result is used after verifying its blobs and recorded sizes, and the task does not run again.
- Rechecks stored work on demand. A verify step reads a manifest back, hashes everything it names again, checks recorded artifact sizes and the Merkle root, and reports the first problem it finds.

## How a run is described

A run is one JSON object holding a list of tasks. This run writes a short file and then converts it to upper case.

```json
{
  "tasks": [
    {
      "task_id": "greet",
      "executable": "sh",
      "args": ["-c", "printf 'hello' > outputs/hello.txt"],
      "env": {},
      "inputs": [],
      "outputs": ["hello.txt"],
      "depends_on": []
    },
    {
      "task_id": "shout",
      "executable": "sh",
      "args": ["-c", "tr a-z A-Z < inputs/hello.txt > outputs/loud.txt"],
      "env": {},
      "inputs": ["hello.txt"],
      "outputs": ["loud.txt"],
      "depends_on": ["greet"]
    }
  ]
}
```

Inside its working folder, a task reads its input files under `inputs` and writes its output files under `outputs`. An input is either a file an earlier task in the run produced, or a host file bound into the store with `--input` or through the library. A task that needs an input nobody provides stops the run before anything executes, with the missing path named.

## What you can rely on

- The same description produces the same manifest across Linux, macOS, and Windows when the tools it invokes write identical bytes. Worker count and folder layout do not change the result.
- A task that runs past its time limit is stopped, together with every process in its Unix process group or Windows job object. Surviving background group members are also terminated when the task ends. A process that leaves its Unix group by starting its own session has its output cut off after a short grace period, and the task is recorded as timed out.
- Output files are opened component by component relative to held directory handles, so symbolic links and reparse points are refused without a check-then-open race. A stored object whose bytes or size no longer match its record is reported rather than used.
- When a task fails, the tasks that depend on it do not run. The manifest records them as skipped, and the run reports a failing exit code.

## Using verirun today

verirun is a Rust workspace at version `0.2.0` with three library crates, an OS boundary crate, and a command line program. Installing the program from source or crates.io is `cargo install verirun-cli`, and prebuilt archives for Linux, macOS, and Windows ship on GitHub Releases with `cargo-binstall` metadata.

- `verirun-core` holds the pure data model. It parses and checks run descriptions, puts JSON into RFC 8785 canonical form, computes framed digests, and builds and diffs manifests. It has no dependencies outside the Rust standard library.
- `verirun-platform` holds the operating system seam. It isolates process-tree control, no-follow handle-relative path traversal, and `rename_replace` atomic replacement across Unix and Windows behind one safe API.
- `verirun-store` holds the storage and execution adapters. It keeps the crash-safe content store on disk, runs tasks under the sandbox rules, schedules tasks in parallel, and executes garbage collection.
- `verirun-cli` holds the `verirun` executable and its pipeline library, providing `run` (with `--input` and `--pin`), `verify`, `show`, `diff`, `pin`, `unpin`, and `gc`, all with `--json` output and the documented exit code contract.

## Building and testing

```sh
cargo build --workspace --locked
cargo test --workspace --locked
```

The Rust version is pinned in `rust-toolchain.toml`, and rustup selects it automatically. Minimum supported Rust version `1.88` is verified in continuous integration alongside Linux, macOS, and Windows full gates. Property test volume comes from `PROPTEST_CASES`, running 1000 cases on pull requests and 10000 on the main branch.

## Benchmarks

The benchmark suite is hand-rolled on the standard library timer and lives in the core, store, and CLI crates. It reports rather than gates, so nothing in continuous integration depends on wall-clock thresholds.

Measured on 2026-11-24 on a hosted Linux runner, from full runs of the bench commands named below.

- Canonical encoding of a 193,835-byte document with 256 task-shaped entries, 200 iterations in 0.112 seconds, 331.4 MiB/s.
- Hashing and storing distinct 4 MiB blobs, 20 iterations in 0.573 seconds, 139.6 MiB/s.
- Storing 500 64 KiB blobs through the full crash-safe put protocol, 0.593 seconds, 843.2 puts per second. The put protocol syncs the file and its directories before an object counts as stored, and that durability work is most of the cost at this size.
- Verified reads of a 4 MiB blob, 50 iterations in 0.138 seconds, 1446.1 MiB/s.

The commands were `cargo bench -p verirun-core --bench canonical_bench`, `cargo bench -p verirun-store --bench cas_bench`, and `cargo bench -p verirun-cli --bench verify_bench`.

## Project status

- Milestones 0 through 4 built the design, canonical encoder, crash-safe store, DAG scheduler, CLI, tamper and crash recovery suites, and benchmarks.
- Roadmap lanes R1 through R7 added crates.io readiness, MSRV verification, examples, tutorial, `verirun show`, portability baselines, and `cargo audit`.
- The 50-round post-completion audit programme hardened sandbox freshness, work-root locking, cache validation, streaming verification, and durability ordering.
- The 14-round perfection programme and Windows workstream delivered full Unix, macOS, and Windows runtime parity, handle-relative output collection, CLI pinning and input bindings, and the `0.2.0` release.

## Read more

- [The architecture document](docs/ARCHITECTURE.md) explains the design in full, including the store layout, the hashing rules, and the timeout behaviour.
- [The invariant list](docs/INVARIANTS.md) states every rule the system keeps. Each rule names the test that enforces it.
- [The tutorial](docs/TUTORIAL.md) walks a first run from the run file to verification, pinning, a cached rerun, a diff, and garbage collection, with real captured output.
- [The security policy](SECURITY.md) states supported versions, the threat model, and how to report vulnerabilities privately.
- [The decisions file](DECISIONS.md) records the locked project choices, including the licence and the dependency rules.
- [The roadmap](docs/ROADMAP.md) records the post-Milestone 4 engineering lanes and owner direction calls.
- [The audit programme](docs/AUDIT_PROGRAMME.md) and [audit register](docs/AUDIT_REGISTER.md) record the 50-round audit of the `0.1.0` build.
- [The perfection programme](docs/PERFECTION_ROADMAP.md) and [perfection register](docs/PERFECTION_REGISTER.md) record the 14-round perfection cycles and Windows runtime workstream.

## Licence

verirun is dual licensed under the [MIT licence](LICENSE-MIT) and the [Apache License 2.0](LICENSE-APACHE). You may use it under either one.
