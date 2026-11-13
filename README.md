# verirun

verirun runs a batch of shell tasks and keeps an exact, checkable record of what the batch did. You describe the work once, as data. verirun checks the description, runs the tasks in the right order, stores every file the tasks read or wrote under a hash of its contents, and finishes by writing one manifest that ties the whole run together. Run the same batch again later, with a different number of workers or in a different folder, and the manifest comes out identical, byte for byte.

## What it does

- Takes a run description as JSON data. Each task names a program, its arguments, the files it reads, the files it writes, the tasks it depends on, and an optional time limit.
- Checks the description before anything runs. Missing dependencies, dependency cycles, two tasks claiming the same output file, and unsafe paths are all reported up front. Nothing runs until the description is clean.
- Runs each task in its own fresh working folder with a cleaned environment. Nothing leaks in from your shell. The environment holds only the values the task declares plus a small fixed set that never changes.
- Stores content by hash. Inputs, outputs, and the captured output and error streams all go into a store folder, named by the SHA-256 hash of their bytes. The same bytes are stored once, however many tasks use them.
- Writes a manifest at the end of the run. The manifest holds one record per task with the hashes of everything the task consumed and produced, plus one root hash that covers the whole run.
- Reuses finished work. When a task comes up whose program, inputs, and environment match a stored success, the stored result is used and the task does not run again.
- Rechecks stored work on demand. A verify step reads a manifest back, hashes everything it names again, and reports the first problem it finds.

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

Inside its working folder, a task reads its input files under `inputs` and writes its output files under `outputs`. An input is either a file an earlier task in the run produced, or a file the caller has placed in the store and bound to that path. A task that needs an input nobody provides stops the run before anything executes, with the missing path named.

## What you can rely on

- The same description produces the same manifest. Worker count and folder layout do not change the result.
- A task that runs past its time limit is stopped together with every process it started.
- Output files that arrive through symbolic links are refused. A stored object whose bytes no longer match its hash is reported rather than used.
- When a task fails, the tasks that depend on it do not run. The manifest records them as skipped, and the run reports a failing exit code.

## Using verirun today

verirun is a Rust workspace with three library crates. The command line program is the next milestone and is not built yet. Today the pieces are used as libraries, and the test suites exercise them end to end.

- `verirun-core` holds the data model. It parses and checks run descriptions, puts JSON into one canonical form, computes the hashes, and builds manifests. It has no dependencies outside the Rust standard library.
- `verirun-store` holds the machinery. It keeps the content store on disk, runs tasks under the sandbox rules, and schedules tasks in parallel up to the worker count you ask for.
- `verirun-cli` holds the library the command line will sit on. It can already run a description from bytes and verify a manifest against a store.

## Building and testing

```sh
cargo build --workspace
cargo test --workspace
```

The Rust version is pinned in `rust-toolchain.toml`, and rustup selects it automatically. The test suites include property tests, and their case count comes from the `PROPTEST_CASES` environment variable. Continuous integration runs 1000 cases for pull requests and 10000 cases on the main branch.

## Project status

- Milestone 0 was the design. It is complete.
- Milestone 1 built the hashing and storage core. It is complete and reviewed.
- Milestone 2 added the task graph checks, the runner, the scheduler, and the result cache. It has landed on the main branch and is in final review.
- Milestone 3 brings the command line program with the full command set and JSON output.
- Milestone 4 brings the tamper and crash recovery suites, larger stress runs, and recorded benchmark numbers. This README arrived ahead of that milestone because the repo needed a front door.

## Read more

- [The architecture document](docs/ARCHITECTURE.md) explains the design in full, including the store layout, the hashing rules, and the timeout behaviour.
- [The invariant list](docs/INVARIANTS.md) states every rule the system keeps. Each rule names the test that enforces it.
- [The decisions file](DECISIONS.md) records the locked project choices, including the licence and the dependency rules.

## Licence

verirun is dual licensed under the MIT licence and the Apache License 2.0. You may use it under either one.
