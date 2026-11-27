# Frequently asked questions

## Determinism and caching

### Why does a warm cached run produce a different manifest digest from a cold run?

By design (`INV-RUN-7` and `INV-MAN-1`). Each task record in a manifest records its execution disposition in the `status` field: `"succeeded"` when the subprocess ran during that invocation, or `"cached"` when `verirun` satisfied the task from a verified `by-exec` cache hit without spawning the subprocess.

Everything else in the task record stays byte-identical across cold and warm runs:

- `exec_digest` (the domain-separated hash of command, args, env, input digests, and declared output paths)
- `exit_code`
- `stdout_sha256` and `stderr_sha256`
- Every entry in `artifacts` (`path`, `sha256`, `bytes`, `executable`)

Running `verirun diff <cold-manifest> <warm-manifest> --cas <dir>` confirms this directly: it reports `changed (status)` and `artifacts: false`. Meanwhile, two cold runs (or two warm runs) with different `--jobs` worker counts or different host workspace paths produce the exact same manifest digest.

### Does `--jobs` affect the manifest digest or execution order determinism?

No (`INV-RUN-5`, `INV-MAN-1`). Ready tasks enter a lexicographically ordered set keyed by `task_id`, and manifest task records are always sorted by `task_id` byte order before Merkle root construction. Running with `--jobs 1` and `--jobs 8` against equivalent cache states produces byte-identical manifests.

### Why are `PATH`, `HOME`, `TMPDIR`, `TZ`, `LC_ALL`, and `SOURCE_DATE_EPOCH` rejected in `env`?

`verirun` constructs every task's environment from scratch (`INV-RUN-1`, `INV-RUN-2`):

- `LC_ALL=C`, `TZ=UTC`, and `SOURCE_DATE_EPOCH=0` are fixed deterministic defaults set unconditionally by the runner. Rejecting them in task `env` prevents a spec from hashing one value into `exec_digest` only to have the runner overwrite it at spawn time.
- `TMPDIR` points to a private per-task temporary directory inside the sandbox and is intentionally excluded from `exec_digest` because its host path varies per run.
- Bare executable names (`"sh"`, `"awk"`, `"python3"`) resolve against the `PATH` snapshot captured when `verirun` starts, without leaking host `PATH` or `HOME` strings into the child environment or into any digest.

## JSON specifications and numbers

### Why are integers above `9007199254740991` (`2^53 - 1`) rejected?

`verirun` canonicalizes JSON under RFC 8785 (JCS), which formats numbers according to IEEE-754 double-precision rules (`INV-CAN-3`, `INV-CAN-4`). Integers with magnitude above `2^53 - 1` cannot round-trip losslessly through `f64` across compliant JSON implementations. Rather than silently rounding large integers, `verirun-core` rejects out-of-range integers at parse and encode time with a byte offset and exits with code `3`.

### Why does `verirun` reject duplicate JSON keys instead of taking the last key?

Lenient parsers disagree on whether the first or last duplicate key wins, opening a classic canonicalization mismatch vector where two tools see different task specifications in the same byte stream. `verirun-core` rejects duplicate object keys at every depth with a byte offset (`INV-CAN-6`, `INV-CAN-7`).

## Exit codes and verification

### Why do CLI usage and flag errors exit with code `3` instead of `2`?

Argument parsing libraries such as `clap` default to exit code `2` on flag mistakes. In `verirun`, exit code `2` is reserved exclusively for cryptographic or store integrity failures (`INV-CLI-1`): a corrupted blob, a tampered manifest, a Merkle root mismatch, an artifact byte-length mismatch, or a missing pinned object. Mapping usage, schema, DAG, and path errors to `3` guarantees that automation and AI agents never confuse a typo on the command line with disk corruption or tampering.

| Exit code | Meaning |
| --- | --- |
| `0` | All tasks succeeded or hit verified cache (`run`), manifest verified (`verify`, `show`, `pin`), manifests equivalent (`diff`), or `gc` / `unpin` / `completions` succeeded. |
| `1` | At least one task failed or timed out (`run`), or verified manifests differ (`diff`). |
| `2` | Store, manifest, Merkle root, or blob integrity check failed. |
| `3` | Invalid CLI arguments, malformed JSON, invalid DAG, unsafe relative path, missing host `--input`, or store lock/setup error. |

### Does `verirun show` or `verirun diff` trust stored manifests without checking blobs?

Never (`INV-CLI-4`, `INV-CLI-5`). Both `verirun show` and `verirun diff` execute the full four-step verification pass of [`docs/ARCHITECTURE.md`](ARCHITECTURE.md) section 9.3 before printing or comparing anything:

1. Re-hash the manifest blob against its requested digest.
2. Parse and re-encode the manifest to RFC 8785 canonical bytes and require byte-for-byte equality.
3. Recompute the Merkle root from task record digests and compare against `merkle_root`.
4. Stream every distinct referenced artifact and `stdout`/`stderr` blob through SHA-256 and check every recorded `bytes` count.

If any check fails, `show` and `diff` emit nothing on standard output (unless `--json` is requested, which emits structured `error_json`) and exit with code `2`.

## Storage, concurrency, and platforms

### Can multiple `verirun` processes share one `--cas` directory at the same time?

Yes (`INV-CAS-5`). Every blob, `by-exec` cache entry, and pin lands via a per-process/per-worker temporary file inside `<cas>/tmp/`, `fsync`, atomic rename, and directory sync. Give each concurrent `verirun run` its own `--work` sandbox root; `verirun` places an exclusive lock on `--work` (`INV-RUN-8`) so two runs cannot trample each other's active task sandboxes.

### How do pins and `verirun gc` interact?

Unpinned objects are eligible for deletion by `verirun gc` once their modification age exceeds `--grace-secs` (default `3600` seconds, so concurrent runs in progress are protected by `INV-GC-2`). Pinning a manifest with `verirun run --pin` or `verirun pin <digest> --cas <dir>` retains:

- The manifest blob itself
- Every artifact, `stdout`, and `stderr` blob referenced by its tasks
- The `by-exec` task record blobs corresponding to its tasks (`INV-GC-1`)

If a pinned manifest or any object reachable from a pin is missing or corrupt, `verirun gc` aborts with exit code `2` before deleting a single file. Use `verirun gc --cas <dir> --dry-run` to preview sweep counts safely (`INV-GC-4`).

### Does `verirun` run natively on Windows as well as Linux and macOS?

Yes (`INV-PLAT-1`). `verirun-platform` provides native Windows implementations alongside Unix:

- Process-tree containment uses Windows Job Objects with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (matching Unix process-group `SIGKILL`).
- Output collection walks components relative to open directory handles via `NtCreateFile` and rejects symlinks, junctions, and reparse points (matching Unix `openat` with `O_NOFOLLOW`).
- Atomic store replacement uses `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`.
