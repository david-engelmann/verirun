# Tutorial: a first run, verified

This walkthrough runs a two-task build with the verirun command line program, verifies the result, runs it again against the cache, and inspects what collection would do. Every output below is real, captured from the program as shipped. Digests in your own run will differ only where file contents differ.

## The run file

A run file is JSON. Each task names an executable and its arguments, the environment it may see, the input files it needs, the output files it promises, and the tasks it depends on. Save this as build.json.

```json
{"tasks":[
  {"task_id":"greet","executable":"sh","args":["-c","printf 'hello from verirun\\n' > outputs/greeting.txt"],"env":{},"inputs":[],"outputs":["greeting.txt"],"depends_on":[]},
  {"task_id":"count","executable":"sh","args":["-c","wc -c < inputs/greeting.txt > outputs/size.txt"],"env":{},"inputs":["greeting.txt"],"outputs":["size.txt"],"depends_on":["greet"]}
]}
```

Two facts about the sandbox matter when writing tasks. A task runs in its own directory under the work root, with its inputs staged under inputs/ and its promised outputs collected from outputs/. The count task therefore reads inputs/greeting.txt, which the runner materializes from the greet task's output because count depends on greet and names the file as an input.

## Run it

```
verirun run --cas cas --work work build.json
```

The program prints the active seed to standard error on every invocation, and the manifest digest to standard output.

```
seed: 0
e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a
```

The exit code is 0. The digest is the address of the run's manifest in the store, and the manifest is the complete record of the run: both tasks, their statuses, their exit codes, and the digests of everything they read and wrote.

## Verify it

```
verirun verify --cas cas e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a
verified e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a
```

Verification re-reads the manifest and every object it names and recomputes each digest. A single changed byte anywhere in the chain fails it, with the exit code for an integrity failure. With --json the same check reports the manifest's Merkle root.

```
verirun verify --cas cas --json e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a
{"manifest":"e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a","merkle_root":"2165b51cb1c8ef886d7868919368f8962187ee15c28d53a8b6aa882d5c9cba7d","verified":true}
```

## Look at it

Verify answers whether a manifest is sound. Show answers what it says. It runs the same verification first, so a tampered manifest prints nothing and fails with the verification error, and then it prints the record in readable form.

```
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

Each task prints with its status and exit code, the digest of the execution that produced it, the digests of its captured streams, and one line per artifact with its size. With --json the same record prints as one JSON value.

## Run it again, then diff

Running the same file again executes nothing. Both tasks come back from the by-exec cache, and the run produces a second manifest whose records say cached where the first said succeeded.

```
verirun run --cas cas --work work build.json
a9479465a06fb13fd1593b3abaa1d08ab01a987b13babb29338a03aba743edfb
```

Diff verifies both manifests and then compares their records field by field. It finds exactly the cache's fingerprint and nothing else, and exits 1 because the runs differ.

```
verirun diff --cas cas e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a a9479465a06fb13fd1593b3abaa1d08ab01a987b13babb29338a03aba743edfb
task count: changed (status)
task greet: changed (status)
```

Had any output byte differed, diff would name that task and field instead. Two runs of an unchanged build on an empty cache produce identical manifests, which is the determinism the store exists to prove.

## Look at collection

Nothing in this walkthrough pinned a manifest, so every object in the store is unreachable and old enough to sweep once the grace period passes. A dry run reports the plan without deleting anything.

```
verirun gc --cas cas --grace-secs 0 --dry-run
dry run: 5 objects (907 bytes) and 0 temporary files (0 bytes) would be swept; 0 objects are reachable from pins
```

Pinning a manifest with the library, or keeping one through the store's pin files, is what makes its whole object graph survive collection. The store example in crates/verirun-store/examples shows the same lifecycle from Rust.

## Where to go next

The examples under each crate's examples/ directory show the libraries behind this walkthrough. docs/ARCHITECTURE.md is the full design, and docs/INVARIANTS.md lists every guarantee with the test that enforces it.
