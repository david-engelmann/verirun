# Tutorial for a first verified run

This walkthrough runs a two-task build with the `verirun` command line program, verifies and inspects the result, runs it again against the cache, compares the two manifests, and pins a manifest before garbage collection. Every output below is real, captured from the program as shipped. Digests in your own run will differ only where file contents differ.

The examples name a shell called `sh` and write small shell scripts as task commands. On Linux and macOS that is the system shell. On Windows the same examples run under Git Bash, with its `sh` on `PATH`. Under Linux and Windows Git Bash, GNU `wc -c` formats counts without leading spaces and produces the exact manifest digest printed below. On macOS, BSD `/usr/bin/wc -c` pads its output with leading spaces (`"      19\n"` rather than `"19\n"`), so the `greet` task and both execution digests match identically while `count` records the 9-byte BSD output honestly.

## The run file

A run file is JSON. Each task names an executable and its arguments, the environment it may see, the input files it needs, the output files it promises, and the tasks it depends on. Save this as `build.json`.

```json
{"tasks":[
  {"task_id":"greet","executable":"sh","args":["-c","printf 'hello from verirun\\n' > outputs/greeting.txt"],"env":{},"inputs":[],"outputs":["greeting.txt"],"depends_on":[]},
  {"task_id":"count","executable":"sh","args":["-c","wc -c < inputs/greeting.txt > outputs/size.txt"],"env":{},"inputs":["greeting.txt"],"outputs":["size.txt"],"depends_on":["greet"]}
]}
```

Two facts about the sandbox matter when writing tasks. A task runs in its own directory under the work root, with its inputs staged under `inputs/` and its promised outputs collected from `outputs/`. The `count` task therefore reads `inputs/greeting.txt`, which the runner materializes from the `greet` task's output because `count` depends on `greet` and names the file as an input. When a task reads an external file that no earlier task produces, pass `--input <rel>=<host-file>` to `verirun run` to stage and bind it into the store before execution.

## Run it

```text
verirun run --cas cas --work work build.json
```

The program prints the active seed to standard error on every invocation, and the manifest digest to standard output.

```text
seed: 0
e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a
```

The exit code is `0`. The digest is the address of the run's manifest in the store, and the manifest is the complete record of the run covering both tasks, their statuses, their exit codes, and the digests and sizes of everything they read and wrote.

## Verify it

```text
verirun verify --cas cas e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a
verified e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a
```

Verification re-reads the manifest and every object it names, recomputes each digest, and checks every recorded artifact byte length. A single changed byte or tampered size anywhere in the chain fails verification with exit code `2`. With `--json` the same check reports the manifest's Merkle root.

```text
verirun verify --cas cas --json e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a
{"manifest":"e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a","merkle_root":"2165b51cb1c8ef886d7868919368f8962187ee15c28d53a8b6aa882d5c9cba7d","verified":true}
```

## Look at it

Verify answers whether a manifest is sound. Show answers what it says. It runs the same verification first, so a tampered manifest prints nothing and fails with the verification error, and then it prints the record in readable form.

```text
verirun show --cas cas e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a
manifest e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a
merkle root 2165b51cb1c8ef886d7868919368f8962187ee15c28d53a8b6aa882d5c9cba7d
task count: succeeded, exit code 0
  exec 7e41c1b07f168544712d313a7febfd69debdfbf19bdd69bdb197e3a33fc23d0b
  stdout c3dff712aede7ebba7f46332df054028d907425b80e8aff38caf8117e1d58e1f
  stderr c3dff712aede7ebba7f46332df054028d907425b80e8aff38caf8117e1d58e1f
  artifact size.txt d8c053c6ee497a3e316765557619a49c1035cbb12d8df218768b1d48c37f5c96 (3 bytes)
task greet: succeeded, exit code 0
  exec c05faca736d98942358d9bd8fd342f1220ac43af8f4be3701594f4d89945539f
  stdout c3dff712aede7ebba7f46332df054028d907425b80e8aff38caf8117e1d58e1f
  stderr c3dff712aede7ebba7f46332df054028d907425b80e8aff38caf8117e1d58e1f
  artifact greeting.txt 4d475f39e2c5565d866ed0f641d926b9909d4537a075910d8be7d4cdb884273e (19 bytes)
```

Each task prints with its status and exit code, the digest of the execution that produced it, the digests of its captured streams, and one line per artifact with its size. With `--json` the same record prints as one JSON value.

## Run it again, then diff

Running the same file again executes nothing. Both tasks come back from the `by-exec` cache, and the run produces a second manifest whose records say `cached` where the first said `succeeded`.

```text
verirun run --cas cas --work work build.json
a9479465a06fb13fd1593b3abaa1d08ab01a987b13babb29338a03aba743edfb
```

Diff verifies both manifests and then compares their records field by field. It finds the cache status change and nothing else, and exits `1` because the runs differ.

```text
verirun diff --cas cas e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a a9479465a06fb13fd1593b3abaa1d08ab01a987b13babb29338a03aba743edfb
task count: changed (status)
task greet: changed (status)
```

Had any output byte differed, diff would name that task and field instead. Two runs of an unchanged build on an empty cache produce identical manifests, which is the determinism the store exists to prove.

## Pinning and garbage collection

Nothing in the two runs above pinned a manifest, so every object in the store would be a sweep candidate once the grace period passes. Pinning the first manifest preserves it and every artifact, stream, and cache record it reaches, and `verirun run --pin` does the same in one step during a run.

```text
verirun pin --cas cas e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a
pinned e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a
```

A dry run with a zero-second grace period now reports only the unpinned second manifest as a candidate while keeping all six objects reachable from the pinned first run.

```text
verirun gc --cas cas --grace-secs 0 --dry-run
dry run: 1 objects (734 bytes) and 0 temporary files (0 bytes) would be swept; 6 objects are reachable from pins
```

`verirun unpin --cas cas <digest>` removes the pin when you no longer need the run retained. The store example in `crates/verirun-store/examples` shows the same lifecycle from Rust.

## Where to go next

The examples under `crates/verirun-core/examples` and `crates/verirun-store/examples` show the libraries behind this walkthrough. `docs/ARCHITECTURE.md` is the full design, and `docs/INVARIANTS.md` lists every guarantee with the test that enforces it.
