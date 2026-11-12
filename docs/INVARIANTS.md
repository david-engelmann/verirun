# verirun Invariants and Traceability Matrix

Status: Milestone 0 design. This document is the formal invariant register for verirun, required by DECISIONS.md Q7. Every invariant has a stable id, a precise statement, its adversarial edge cases, and exactly the enforcement that proves it. The matrix in section 12 maps each invariant 1-to-1 to its property tests, fault oracles, or benchmarks. An invariant with no row in the matrix does not exist, and a test with no invariant is decoration. When a milestone adds behaviour, it adds or amends invariants here in the same pull request.

## How enforcement works

- Property test: a `proptest` property over generated inputs. Volume comes from `PROPTEST_CASES`, 1000 on pull requests and 10000 on main. No test hardcodes its own case count.
- Fault oracle: a test that injects a specific corruption, crash point, or hostile input and asserts the specified failure, including exit code and named digests.
- Unit and vector test: a fixed case with an expected byte string or digest, including the RFC 8785 Appendix B vectors.
- Benchmark: a `criterion` benchmark that must compile and run under `cargo bench --workspace -- --test` in CI. Benchmarks prove the streaming and throughput shape of an implementation. CI never gates on wall-clock numbers.
- Every seeded runner prints its active seed (`--seed`, else `SIM_SEED`, else 0) on every run, and a failure reproduces from that seed alone.

Test locations: core tests live in `crates/verirun-core/tests/`, store tests in `crates/verirun-store/tests/`, CLI tests in `crates/verirun-cli/tests/`. Benchmarks live in the owning crate's `benches/` directory.

## 1. Canonical encoding invariants (CAN)

### INV-CAN-1. UTF-16 key order
For any object, the canonical encoding orders members by the UTF-16 code unit sequence of their names, recursively. The order holds in particular for the discriminating pair: a name starting with a supplementary plane character (high surrogate range U+D800 to U+DBFF) sorts before a name starting with a character in U+E000 to U+FFFF.
Edge cases: equal prefixes with different lengths, names that are empty strings, names containing surrogate pair characters at every position, nested objects.

### INV-CAN-2. Canonical round trip
For any in-domain value v, `parse(encode(v))` equals v, and `encode(parse(encode(v)))` is byte-identical to `encode(v)`. Canonical bytes are a fixed point of parse and encode.
Edge cases: empty object, empty array, empty string, deeply nested arrays up to the depth limit, strings with every control character.

### INV-CAN-3. Number domain
Negative zero encodes as `0`. NaN and both infinities are rejected. Any number whose value is integral with magnitude above 9007199254740991 is rejected with `IntegerOutOfSafeRange`. A literal that overflows f64 is rejected at parse with its byte offset. Rejection through the CLI exits with code 3.
Edge cases: -0.0, 9007199254740991 (accepted, encodes as itself), 9007199254740992 (rejected), 1e21 (integral, rejected), 1.5e21 (non-integral, accepted), 1e999 (parse rejection), 5e-324 (smallest subnormal, accepted).

### INV-CAN-4. ECMAScript number layout
Every accepted non-integral or small integral value encodes byte-identically to the ECMA-262 `Number::toString` layout of ARCHITECTURE.md section 4.3, including the notation thresholds at 1e21 and 1e-6 and the explicit exponent sign.
Edge cases: 0.000001 encodes as `0.000001`, 0.0000001 as `1e-7`, 1.5e21 as `1.5e+21`, integers at the 21 digit boundary, subnormals, values whose shortest digit string ends in zeros after parsing the standard library form.

### INV-CAN-5. String escaping minimality
Canonical output escapes exactly the quotation mark, the backslash, and control characters U+0000 to U+001F (short escapes where RFC 8785 defines them, lowercase `\u00xx` otherwise). All other characters, including `/`, U+007F, and all non-ASCII text, emit as raw UTF-8.
Edge cases: a string holding all 32 control characters, DEL, solidus, combining characters, emoji, and text that looks like an escape sequence.

### INV-CAN-6. Parser strictness
The parser rejects, each with a byte offset and never with a panic: duplicate object keys, trailing bytes after the top level value, lone high or low surrogate escapes, raw control characters in strings, leading zeros and other malformed numbers, empty input, and nesting deeper than 128 levels.
Edge cases: duplicate key at depth, a valid surrogate pair split by whitespace (rejected, the pair must be adjacent escapes), depth exactly 128 (accepted) and 129 (rejected), fuzzed truncations of valid documents at every byte position.

### INV-CAN-7. Duplicate keys never reach the encoder
No code path can construct or encode an object with a duplicate key. Parsing rejects duplicates (INV-CAN-6), and programmatic construction through the value API keeps pairs unique by rejection on insert.
Edge case: a run file that declares the same env key twice inside one task object, which is a duplicate key at the JSON level and exits with code 3.

## 2. Hashing invariants (HASH)

### INV-HASH-1. Domain separation
Two digests computed under different domain tags are computed over different byte strings by construction, because the tag and its 0x00 separator are the first bytes hashed. No payload crafted under one tag can be reinterpreted as a payload under another tag.
Edge case: a blob whose raw bytes begin with a valid tuple framing for another domain. The tag prefix still separates the hash inputs.

### INV-HASH-2. Tuple framing is unambiguous
Pattern T framing (ARCHITECTURE.md section 5.1) is injective. Two different field sequences never frame to the same byte string, because every variable length field is length-prefixed as u64 little-endian and every fixed field has a stated width. All integers in framing are little-endian.
Edge cases: adjacent text fields whose concatenation is ambiguous without lengths (`"ab","c"` versus `"a","bc"`), empty text fields, fields of length 0 and of length above 4 GiB in generated framing tests (length arithmetic only, no allocation of the payload).

### INV-HASH-3. Streaming equals one-shot
For any byte stream, feeding a `StreamingHasher` in any chunking gives the same digest as one update with the whole stream. The hasher is order-sensitive: any permutation of chunks that changes byte order changes the input, and the digest follows the bytes.
Edge cases: chunk sizes of 1 byte, chunk boundaries at every position of a small stream, an empty stream, a stream of exactly one 64 KiB buffer.

### INV-HASH-4. Execution tuple completeness
Any change to the executable string, the ordered arguments, the declared environment pairs, the fixed defaults, the set of input path and content pairs, or the declared output paths changes the execution digest, up to SHA-256 collision resistance. Host workspace path, worker count, and `TMPDIR` value changes never change it.
Edge cases: argument order swap (changes the digest), env pair insertion order in the file (never changes it, pairs sort), the same content at a different input path (changes it), different content at the same input path (changes it).

## 3. Content-addressed store invariants (CAS)

### INV-CAS-1. Atomic visibility
A blob is never observable in a partial state. At the destination path, a reader sees either no file or the complete file whose bytes hash to the name. This holds during concurrent puts and across a crash at any step of the put protocol.
Edge cases: crash during write, after file sync, and after rename before directory sync, per the ARCHITECTURE.md section 6.2 table.

### INV-CAS-2. Durability ordering
`put` reports success only after the file sync, the rename, and the destination directory sync have all completed, in that order. A success result followed by a power loss never loses the blob on a filesystem that honours sync semantics.
Edge case: fault injection that records the call order of sync and rename operations through a test filesystem seam, asserting the order rather than the timing.

### INV-CAS-3. Immutability and read-only storage
A stored object is never modified after its rename. Objects are stored read-only. No API opens an object for writing, and repair means delete plus re-derive.
Edge case: an attempt to write through a stored object path fails on permissions for a non-root process, and the byte content is verified unchanged after the attempt.

### INV-CAS-4. Content identity
A put of bytes B always yields the blob digest of B, and a get of that digest returns exactly B or an integrity error. Putting the same bytes twice leaves exactly one object, and the second put reports already present.
Edge cases: empty bytes, bytes that are themselves a valid manifest, binary bytes covering all 256 values.

### INV-CAS-5. Concurrent writer safety
Parallel workers putting identical digests concurrently never corrupt a blob, never fail with a temporary file collision, and never produce a torn read. Parallel workers putting distinct digests never interfere. After any such storm, every object in the store re-hashes to its name.
Edge cases: 32 writers on one digest from a barrier start, mixed identical and distinct digests, a reader looping gets during the storm asserting INV-CAS-1 on every read.

### INV-CAS-6. Same-filesystem renames
Temporary files are always created inside `<cas>/tmp/` on the same filesystem as `<cas>/objects/`, so renames never fail with EXDEV. A CAS root whose `tmp/` and `objects/` resolve to different devices is refused at open with exit code 3.
Edge case: a store root assembled with a mounted or symlinked `tmp/` on another device, detected through device id comparison.

### INV-CAS-7. No aliasing across the sandbox boundary
No hardlink ever connects a sandbox file and a stored object, in either direction. All transfers across the boundary are streamed copies. A task modifying its sandbox files after collection can never change a stored blob.
Edge case: put a file, then rewrite the source file in place and append to it. The stored blob still hashes to its digest. Materialize a blob, modify the copy, re-verify the store: unchanged.

### INV-CAS-8. Bounded streaming memory
Putting or getting a blob of any size uses a bounded buffer (64 KiB). Peak memory of the transfer is independent of blob size. No code path reads a whole blob into memory to hash, copy, or verify it.
Edge case: a generated blob far larger than the buffer, transferred through a counting sink that records chunk sizes and asserts the bound on every call.

## 4. DAG invariants (DAG)

### INV-DAG-1. Cycles rejected upfront
A graph with any dependency cycle is rejected before execution with exit code 3, and the error lists exactly the tasks Kahn's algorithm leaves with nonzero indegree, sorted.
Edge cases: a self dependency, a two cycle, a cycle inside a large acyclic graph, a diamond that is not a cycle (accepted).

### INV-DAG-2. Missing dependencies rejected upfront
A `depends_on` entry that names no task in the run is rejected with exit code 3, naming the task and the missing id.
Edge case: a dependency on a task id that differs only in case or trailing whitespace (both are different ids, both missing).

### INV-DAG-3. Duplicate task ids rejected upfront
Two tasks with the same `task_id` are rejected with exit code 3, naming the id.
Edge case: duplication across non-adjacent positions in the file.

### INV-DAG-4. Colliding outputs rejected upfront
A declared output path claimed by two tasks, or twice by one task, is rejected with exit code 3, naming the path and both task ids.
Edge cases: collision after path normalization (two spellings of one path), a path that collides only when `.` components are removed.

### INV-DAG-5. Deterministic first error
For a spec with several violations, validation always reports the same first error, because checks run in the fixed order of ARCHITECTURE.md section 7.1. The same spec bytes produce the same error on every host and every run.
Edge case: a spec that is simultaneously cyclic and has a duplicate id reports the duplicate, in every process and at every seed.

### INV-DAG-6. Schema strictness
Unknown task fields, a missing required field, a wrong-typed field, an empty `task_id` or `executable`, a declared fixed-default env key (`TZ`, `LC_ALL`, `SOURCE_DATE_EPOCH`), and `timeout_ms` below 1 are each rejected with exit code 3 before execution.
Edge case: `timeout_ms` of exactly 1 (accepted) and 0 (rejected), an env object declaring `TZ` with the same value as the default (still rejected: fixed means fixed).

## 5. Execution invariants (RUN)

### INV-RUN-1. Hermetic environment
A child process observes exactly its declared env pairs, the three fixed defaults, and the runner-set `TMPDIR`, and nothing else. No host variable, including `PATH`, `HOME`, and locale variables, is present unless declared and allowed.
Edge case: the test host exports a sentinel variable. A task that dumps its environment produces a blob whose content is checked pair by pair against the expected sorted set.

### INV-RUN-2. Fixed defaults hold
Every child observes `TZ=UTC`, `LC_ALL=C`, and `SOURCE_DATE_EPOCH=0`, whatever the host shell holds. A task printing its timezone and locale observes the fixed values under a host set to a different zone and locale.
Edge case: host `TZ` set to a non-UTC zone and `LC_ALL` set to a non-C locale during the test run.

### INV-RUN-3. Concurrent drain, raw bytes
Stdout and stderr are drained concurrently as raw bytes into separate blobs. Any interleaving and any byte content, including NUL bytes and invalid UTF-8, is preserved exactly. A child can fill both pipe buffers many times over without blocking.
Edge cases: 1 MiB on each stream in alternating writes, all output on stderr with stdout empty and the reverse, a child that writes and then sleeps (drains must not wait for exit to make progress, proven by the timeout case below).

### INV-RUN-4. Timeout enforcement
A task exceeding its `timeout_ms` is killed, reaped, and recorded with status `timed_out`. Its drain threads finish and are joined. The run completes, dependents are skipped, and the CLI exits with code 1. No child or drain thread survives the run.
Edge cases: a task that floods both pipes and then sleeps past its deadline (the kill must not hang the drains), a task that exits just before the deadline (recorded by its real exit), `timeout_ms` absent (no deadline).

### INV-RUN-5. Worker-count independence
The same graph and inputs executed with 1 worker and with N workers produce byte-identical manifests. Scheduling affects wall-clock time only.
Edge case: a wide graph with many independent tasks of uneven duration, where completion order visibly differs between the two runs.

### INV-RUN-6. Failure propagation is exact
When a task fails or times out, exactly its transitive dependents are recorded `skipped`. Independent tasks still run. The manifest holds one record per spec task in all outcomes, and the run exits with code 1.
Edge cases: a failure at the root of a chain, a failure in one branch of a diamond (the other branch and the join's other inputs still resolve, the join itself is skipped), a graph where everything fails.

### INV-RUN-7. Cache soundness
A cached task is reused only when its execution digest matches and every recorded output and log blob still exists and re-hashes correctly at materialization. A cache entry pointing at missing or corrupted blobs is ignored and the task re-executes. Cache state can never change a manifest's meaning: a run served fully from cache and the same run executed cold produce identical manifests apart from the `cached` status markers defined in the record semantics, and identical artifact and log digests.
Edge case: delete one output blob of a cached task, re-run, and observe re-execution of exactly that task.

## 6. Sandbox invariants (SBX)

### INV-SBX-1. No parent traversal
A declared input or output path containing a `..` component is rejected during validation with exit code 3. No task input or output is ever read or written outside its sandbox.
Edge cases: `../escape`, `a/../../escape`, `a/../b` (rejected as well: any `..` component is rejected, not only escaping ones, so the rule has no judgement calls).

### INV-SBX-2. Absolute paths rejected
A declared path that is absolute is rejected during validation with exit code 3.
Edge cases: `/etc/passwd` as an output, a Windows-style rooted path on a Unix host (still a path string with components that fail the relative rule or the component rules).

### INV-SBX-3. Symlink outputs rejected
A declared output path that is a symbolic link at collection time fails the task, whatever the link target. The check uses `symlink_metadata` and fires before any open of the path.
Edge cases: a link to a file inside the sandbox (still rejected), a link to a file outside the sandbox, a dangling link, a link created by the task itself during execution pointing at a host file, whose content must never enter the store.

### INV-SBX-4. Collection is exact and metadata is normalized
Collection gathers exactly the declared outputs, in sorted path order. Each artifact record carries only relative path, content digest, byte length, and the executable bit. No timestamps, owners, or host paths appear in any record.
Edge cases: undeclared files in the sandbox (ignored), a declared output that is a directory (task fails), a declared output that never appears (task fails), an output with the executable bit set and one without, in one task.

### INV-SBX-5. Input materialization is verified
Inputs are copied into the sandbox from the store with hash verification during the copy. A stored input that fails verification aborts the run with exit code 2 before the task starts, naming the digest.
Edge case: corrupt one byte of an input blob in the store, then run a graph that consumes it.

## 7. Manifest invariants (MAN)

### INV-MAN-1. Manifest byte determinism
Executing the same graph with the same inputs produces the same manifest bytes, and therefore the same manifest digest, regardless of worker count, completion order, and host workspace directory. There are no timestamps or host values in a manifest to break this.
Edge case: the two runs happen from different absolute workspace paths with different directory names.

### INV-MAN-2. Portability of the manifest digest
A manifest produced on one host verifies on another host against a store holding the same blobs. Nothing in verification depends on the producing host's paths, locale, timezone, or user.
Edge case: copy the store and the manifest digest to a fresh directory tree with different absolute paths and verify there.

### INV-MAN-3. Blob tamper detection
Flipping any single byte in any stored artifact, stdout, or stderr blob causes `verirun verify` to exit with code 2 and to report the expected digest, the actual digest, and the referencing task and path.
Edge cases: a flip in the first byte, the last byte, and a middle byte of blobs of each role, plus a truncated blob and an extended blob.

### INV-MAN-4. Manifest tamper detection
Flipping any single byte in the manifest blob itself causes verification to fail with exit code 2, either at the manifest re-hash step or at the canonical re-encode step. There is no byte position in a manifest whose change goes unnoticed.
Edge case: exhaustive single-byte flips across a small manifest, every position, in the fault suite.

### INV-MAN-5. Pointer swap detection
Swapping two digest pointers inside a manifest (for example exchanging the stdout digests of two tasks, or the artifact digests of two paths) causes verification to fail with exit code 2, at the Merkle root recomputation or at a blob re-hash, naming the inconsistent record.
Edge case: swapping digests of two blobs with identical lengths, so length checks alone cannot catch it.

### INV-MAN-6. Merkle root correctness
The stored `merkle_root` equals the root recomputed from the record fields under the construction of ARCHITECTURE.md section 9.2, including the odd-leaf promotion rule and the empty manifest rule. A manifest whose root field alone is edited fails verification with both roots reported.
Edge cases: manifests with 0, 1, 2, 3, and 5 tasks (promotion at one and two levels), a root edited to the root of a different valid manifest.

## 8. Garbage collection invariants (GC)

### INV-GC-1. Pinned reachability is preserved
`verirun gc` never deletes a blob reachable from a pinned manifest: not the manifest blob, not any artifact or log blob it names, not the by-exec entries its records use.
Edge case: a store where two manifests share most blobs and only one is pinned. The shared blobs survive. The unshared unpinned blobs past the grace threshold do not.

### INV-GC-2. Grace protects in-flight work
An unreachable blob or temporary file younger than the grace threshold is never swept. Only age past the threshold makes an unreachable object a candidate.
Edge cases: an object one second below the threshold and one second above it, a fresh temporary file from a running put, `--grace-secs 0` (every unreachable object is a candidate).

### INV-GC-3. Stale temporary files are reclaimed
Temporary files under `tmp/` older than the grace threshold are deleted by gc, whatever put left them. Temporary files are never treated as reachable content.
Edge case: a temp file whose name embeds a digest that is also a pinned blob. The temp file is still reclaimed and the blob is untouched.

### INV-GC-4. Dry run deletes nothing
`verirun gc --dry-run` prints the exact plan a real run would execute and deletes no file. The store tree is byte-identical before and after a dry run.
Edge case: dry run followed by a real run on an unchanged store executes exactly the printed plan.

## 9. CLI invariants (CLI)

### INV-CLI-1. Exit code totality
Every CLI outcome maps to exactly one documented code: 0 success or verified, 1 task execution or timeout failure or a differing `diff`, 2 integrity or tamper failure, 3 invalid schema, DAG, canonicalization, path, argument, or configuration error. No outcome exits with any other code, and argument parsing failures exit 3, never 2.
Edge cases: a usage error (exits 3), a tampered store under `verify` (exits 2), a timed out task under `run` (exits 1), a cyclic graph under `run` (exits 3).

### INV-CLI-2. JSON output parity
For every command, `--json` output carries the same information content as the text output, including digests, statuses, and error context. The JSON form is never the only place a failure is explained.
Edge case: a verification failure rendered both ways names the same expected and actual digests in both.

### INV-CLI-3. Seed reporting and reproduction
Every CLI run prints its active seed. Seeded property and simulation runners produce byte-identical traces in separate processes at the same seed, and different traces at a different seed for generators with enough entropy to differ.
Edge case: the same seed passed once through `--seed` and once through `SIM_SEED` produces identical traces.

### INV-CLI-4. Diff semantics
`verirun diff` exits 0 for equivalent runs, 1 for runs that differ in any compared field, and 2 when either side fails verification. Its report names the differing tasks in task id order.
Edge cases: a task present on one side only, a status change with identical digests, identical runs (exit 0), a corrupted side (exit 2, never reported as a mere difference).

## 10. Latency and resource invariants (LAT)

### INV-LAT-1. Streaming throughput shape
Canonical encoding, hashing, and CAS transfer all stream in bounded chunks, so throughput benchmarks scale linearly with bytes and peak memory stays flat. The `criterion` benchmarks for encoding, hashing, and put throughput run with `Throughput::Bytes` and `black_box` on inputs and outputs.
Edge case: benchmark inputs at two sizes an order of magnitude apart show the flat memory shape through the counting sink assertions of INV-CAS-8.

### INV-LAT-2. Pipe drain deadlock bound
A task writing unbounded output to both pipes is bounded by its timeout or by completion, never by a drain deadlock. The drain stress suite runs under a hard test deadline: exceeding the deadline is a failure, not a hang.
Edge case: 16 MiB split across both streams from a child that never closes either stream until the end.

### INV-LAT-3. Verification is single pass
Verifying a manifest re-hashes each named blob exactly once and recomputes each record digest exactly once. Verification cost is linear in total bytes plus record count. No blob is read twice in one verify run.
Edge case: a manifest with many tasks sharing one large blob still reads that blob once per referencing check as specified, and the counting reader in the test double proves no record triggers a rescan of the whole store.

### INV-LAT-4. Validation precedes work
All validation errors (DAG family, SBX-1, SBX-2, CAN family) are reported before any sandbox is created, any blob is written, or any process is spawned. An invalid run leaves the store and the work directory untouched.
Edge case: a graph whose last task is invalid, run against a fresh store. The store stays empty.

## 11. Adversarial case catalogue

The cases below cut across families and are named here so the fault suites cannot quietly drop them:

- A run file that is valid JSON but not valid UTF-8 in a string (rejected by the parser with an offset).
- A task id that sorts differently in byte order than in locale collation (byte order wins everywhere).
- Two tasks whose outputs differ only by a `.` component (collision after normalization, INV-DAG-4).
- A child that replaces a declared output file with a symlink between the existence check and collection (the `symlink_metadata` check at collection time is the one that counts, INV-SBX-3).
- A child that keeps a pipe open through a grandchild process after the child itself exits (drains see the pipe held open; the task is not complete until the pipes close or the deadline fires, INV-RUN-4).
- A store with a blob file whose name is a valid digest but whose permissions deny reading (verify reports an integrity class failure naming the digest, never a panic).
- A manifest digest given to `verify` that names no object in the store (exit code 2, reported as a missing manifest, never a panic or a code 3).
- Concurrent `run` invocations sharing one CAS root (INV-CAS-5 covers the writer side; pins and by-exec writes follow the same atomic rules).
- A GC sweep racing a put: the grace rule (INV-GC-2) is the specified protection, and the race test sets a long grace and asserts survival.

## 12. Traceability matrix

Enforcement key: P property test, F fault oracle, V unit or vector test, B benchmark (compile and smoke run in CI).

| Invariant | Enforcement | Test or benchmark | Milestone |
| --- | --- | --- | --- |
| INV-CAN-1 | V, P | `canonical_vectors::vec_utf16_discriminating_pair`, `canonical_prop::prop_key_order_matches_utf16_reference` in `crates/verirun-core/tests/` | M1 |
| INV-CAN-2 | P | `canonical_prop::prop_parse_encode_roundtrip` | M1 |
| INV-CAN-3 | V, F | `canonical_vectors::vec_number_domain_boundaries`, `cli_exit_codes::fault_cli_integer_out_of_range_exits_3` in `crates/verirun-cli/tests/` | M1, M3 |
| INV-CAN-4 | V, P | `canonical_vectors::vec_rfc8785_appendix_b_in_domain`, `canonical_prop::prop_number_layout_matches_oracle` | M1 |
| INV-CAN-5 | V, P | `canonical_vectors::vec_escaping_table`, `canonical_prop::prop_no_unlisted_escapes` | M1 |
| INV-CAN-6 | F, P | `canonical_faults::fault_parser_rejection_table`, `canonical_prop::prop_parser_never_panics_on_truncation` | M1 |
| INV-CAN-7 | F | `canonical_faults::fault_duplicate_env_key_in_run_file` (CLI level, exit 3) | M1, M3 |
| INV-HASH-1 | V | `digest_vectors::vec_domain_separation_prefixes` | M1 |
| INV-HASH-2 | P | `digest_prop::prop_tuple_framing_injective` | M1 |
| INV-HASH-3 | P | `digest_prop::prop_streaming_chunking_invariant` | M1 |
| INV-HASH-4 | P, V | `digest_prop::prop_exec_tuple_sensitivity`, `digest_vectors::vec_exec_tuple_fixed` | M1 |
| INV-CAS-1 | F | `cas_faults::fault_crash_at_each_put_step` in `crates/verirun-store/tests/` | M1 |
| INV-CAS-2 | F | `cas_faults::fault_put_call_order_recorded` | M1 |
| INV-CAS-3 | F | `cas_faults::fault_stored_object_write_attempt` | M1 |
| INV-CAS-4 | P | `cas_prop::prop_put_get_identity` | M1 |
| INV-CAS-5 | F, P | `cas_concurrent::stress_parallel_identical_and_distinct_puts` | M1 |
| INV-CAS-6 | F | `cas_faults::fault_cross_device_tmp_refused` | M1 |
| INV-CAS-7 | F | `cas_faults::fault_source_mutation_after_put` | M1 |
| INV-CAS-8 | F, B | `cas_faults::fault_counting_sink_chunk_bound`, benchmark `cas_bench::bench_put_throughput` | M1, M4 |
| INV-DAG-1 | F | `dag_faults::fault_cycle_table` in `crates/verirun-core/tests/` | M2 |
| INV-DAG-2 | F | `dag_faults::fault_missing_dependency` | M2 |
| INV-DAG-3 | F | `dag_faults::fault_duplicate_task_id` | M2 |
| INV-DAG-4 | F | `dag_faults::fault_colliding_outputs_incl_normalized` | M2 |
| INV-DAG-5 | F | `dag_faults::fault_first_error_stable` | M2 |
| INV-DAG-6 | F | `dag_faults::fault_schema_rejection_table` | M2 |
| INV-RUN-1 | F | `runner_env::fault_sentinel_variable_absent` in `crates/verirun-store/tests/` | M2 |
| INV-RUN-2 | F | `runner_env::fault_fixed_defaults_under_hostile_host_env` | M2 |
| INV-RUN-3 | F | `runner_pipes::stress_both_pipes_past_capacity` | M2, M4 |
| INV-RUN-4 | F | `runner_timeout::fault_timeout_kills_and_reaps`, `runner_timeout::fault_timeout_under_pipe_flood` | M2, M4 |
| INV-RUN-5 | P | `scheduler_prop::prop_worker_count_independence` | M2 |
| INV-RUN-6 | F | `scheduler_faults::fault_failure_propagation_sets` | M2 |
| INV-RUN-7 | F | `scheduler_faults::fault_cache_missing_blob_reexecutes` | M2 |
| INV-SBX-1 | F | `paths_faults::fault_parent_traversal_table` in `crates/verirun-core/tests/` | M2 |
| INV-SBX-2 | F | `paths_faults::fault_absolute_path_table` | M2 |
| INV-SBX-3 | F | `runner_collect::fault_symlink_output_rejected` in `crates/verirun-store/tests/` | M2 |
| INV-SBX-4 | F | `runner_collect::fault_collection_exactness_and_metadata` | M2 |
| INV-SBX-5 | F | `runner_collect::fault_corrupt_input_aborts_before_spawn` | M2 |
| INV-MAN-1 | P | `manifest_prop::prop_manifest_determinism_across_workers_and_workspaces` in `crates/verirun-cli/tests/` | M2 |
| INV-MAN-2 | F | `manifest_faults::fault_verify_from_relocated_store` | M3 |
| INV-MAN-3 | F | `tamper_suite::fault_bitflip_leaf_blobs` in `crates/verirun-cli/tests/` | M4 |
| INV-MAN-4 | F | `tamper_suite::fault_bitflip_manifest_exhaustive_small` | M4 |
| INV-MAN-5 | F | `tamper_suite::fault_digest_pointer_swaps` | M4 |
| INV-MAN-6 | V, F | `manifest_vectors::vec_merkle_shapes_0_1_2_3_5`, `tamper_suite::fault_root_field_edited` | M2, M4 |
| INV-GC-1 | F | `gc_faults::fault_gc_preserves_pinned_reachability` in `crates/verirun-store/tests/` | M3 |
| INV-GC-2 | F | `gc_faults::fault_gc_grace_boundary` | M3 |
| INV-GC-3 | F | `gc_faults::fault_gc_reclaims_stale_tmp_only` | M3 |
| INV-GC-4 | F | `gc_faults::fault_gc_dry_run_deletes_nothing` | M3 |
| INV-CLI-1 | F | `cli_exit_codes::fault_exit_code_table` in `crates/verirun-cli/tests/` | M3 |
| INV-CLI-2 | F | `cli_exit_codes::fault_json_parity_on_failure` | M3 |
| INV-CLI-3 | P | `cli_seed::prop_seed_trace_reproducibility` | M3 |
| INV-CLI-4 | F | `cli_diff::fault_diff_semantics_table` | M3 |
| INV-LAT-1 | B | `canonical_bench::bench_encode_throughput`, `cas_bench::bench_hash_throughput` in the owning crates' `benches/` | M4 |
| INV-LAT-2 | F | `runner_pipes::stress_unbounded_output_under_deadline` | M4 |
| INV-LAT-3 | F, B | `manifest_faults::fault_verify_single_pass_counting_reader`, benchmark `cas_bench::bench_verify_throughput` | M4 |
| INV-LAT-4 | F | `dag_faults::fault_invalid_run_leaves_store_untouched` | M2 |
