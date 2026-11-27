# Changelog

Every notable change to verirun is recorded here. Versions follow the semantic versioning policy in `CONTRIBUTING.md`.

## Unreleased

- Process group cleanup: a task's process group is disarmed once its leader has been reaped, so a finished run can never signal a process group id the system has since handed to another group.
- Python SDK error messages name the resolved binary's version beside the SDK's own when the binary fails without a structured error, so a stale binary found on PATH explains itself.
- Version parity now covers the crate versions, the Python package version, and the SDK's own version string at pull request time as well as at release time, so a version skew fails the gate where it is introduced.

## 0.2.0

Cross-platform runtime parity across Linux, macOS, and Windows, CLI ergonomics for pinning and external inputs, a typed Python SDK and PyPI binary wheels, and correctness, integrity, and durability hardening across the entire workspace.

- Cross-platform execution and `verirun-platform`. The runner and scheduler execute natively on Linux, macOS, and Windows, with full CI gates across all three platforms. Process control, no-follow handle-relative path traversal, and atomic `rename_replace` file replacement live behind safe APIs in the new `verirun-platform` crate. On Unix `ProcessGroup` signals `-pid` directly via `libc::kill` with `SIGKILL` on timeout and on drop, matching Windows job object `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` cleanup parity. Handle traversal maps `ELOOP`, FreeBSD `EMLINK`, and NetBSD `EFTYPE` uniformly to `TraversalError::Link`.
- CLI `pin`, `unpin`, `run --pin`, and `run --input`. `verirun pin` verifies a manifest in full before recording its pin, `verirun unpin` removes a pin unconditionally so damaged pins can always be cleared, `verirun run --pin` pins the produced manifest atomically, and `verirun run --input <rel>=<file>` validates and stages external host files into the store before scheduling (`INV-LAT-4`).
- Artifact byte-length integrity (`VerifyError::ArtifactSizeMismatch`, breaking). Output collection streams through `Cas::put_stream_sized` so `ArtifactRecord.bytes` commits to the exact byte count hashed into the store rather than a separate metadata read. `verirun verify` and `verirun show` check every verified blob's byte length against `ArtifactRecord.bytes` and reject tampered sizes with exit code `2` even when the Merkle root was recomputed. Scheduler cache lookup likewise verifies cached artifact byte lengths before accepting a cache hit.
- Environment and timeout hardening. Run validation rejects `"TMPDIR"` in `TaskSpec.env` alongside `LC_ALL`, `SOURCE_DATE_EPOCH`, and `TZ` so a declared `TMPDIR` cannot enter `exec_digest` and be overwritten at spawn (`INV-DAG-6`, `INV-RUN-1`). Task supervision computes deadlines with `Instant::checked_add` so large schema-valid `timeout_ms` values up to `MAX_SAFE_INTEGER` never panic worker threads.
- Pinned manifest verification in garbage collection. `gc::collect` verifies canonical encoding and `Manifest::recompute_root` on every pinned manifest before planning a sweep, aborting with `GcError::InvalidPinnedManifest` if a pinned manifest's root or encoding is corrupt.
- CLI `--json` error parity (`INV-CLI-2`). Malformed digest arguments on `verify`, `show`, `diff`, `pin`, and `unpin`, malformed `--input` bindings, and invalid `SIM_SEED` environment values emit structured `error_json` on standard output whenever `--json` is passed.
- Per-process temporary nonces and `create_new` cache publication. Store temporary file names and `by-exec` cache temporary files carry a per-process nonce minted once from the clock and open exclusively with `File::create_new` so crash debris after PID reuse cannot collide or overwrite (`temp_file_name_known` and `temp_file_name_stream` take `nonce` as a parameter, breaking).
- Handle-relative output collection without a check-then-open window. Output collection opens each path component relative to held directory handles (`openat` with `O_NOFOLLOW` on Unix and `NtCreateFile` with reparse inspection on Windows), closing audit finding F11 (`INV-SBX-3`).
- Sandbox freshness and work-root locking. Each task sandbox is cleared before execution (`INV-SBX-6`), and `scheduler::run` holds `run.lock` exclusively in the work root (`RunError::WorkLocked`, exit code `3`, `INV-RUN-8`).
- Cache path validation (`INV-RUN-7`). Stored `by-exec` records are honoured only when their artifact paths match the task's declared outputs.
- Verification streaming and access taxonomy (breaking). Manifest verification streams referenced blobs in bounded 64 KiB chunks (`INV-CAS-8`) and distinguishes unreadable manifests and blobs from missing ones via `VerifyError::ManifestUnreadable` and `VerifyError::BlobUnreadable`.
- Stream capture and read resilience. Pipe read errors abort the run rather than finalizing a truncated stream (`StoreStep::ReadSource`, breaking), and interrupted `EINTR` store reads retry automatically.
- Parser, encoder, and builder strictness (breaking). Canonical encoding enforces the 128-level depth cap (`CanonicalErrorKind::DepthLimitExceeded`), parser duplicate-key detection switches to a membership set above 16 keys, `Manifest` and `TaskRecord` reject duplicate task ids and duplicate artifact paths (`ManifestError::DuplicateTaskId`, `ManifestError::DuplicateArtifactPath`), DAG validation rejects duplicate task inputs (`DagError::DuplicateInput`), and `Digest::from_hex` counts and reports full UTF-8 characters accurately.
- Store durability and stale lock recovery. Presence-hit and race-discard puts sync destination directories before reporting durability, and `gc::collect` breaks leftover `gc.lock` files older than 24 hours (`INV-GC-6`).
- Public documentation, Python SDK, and release automation. Added `docs/README.md`, light and dark SVG lockups, architecture and Merkle tamper-conviction diagrams, a `1200x630` social preview card, a real-output `docs/media/money-shot.gif` walkthrough, a typed Python SDK and precompiled `verirun` PyPI wheels (`pyproject.toml`, `python/verirun`), and GitHub Actions OIDC release workflows for crates.io, PyPI, and five-platform GitHub Releases.

## 0.1.0

The first release, published to crates.io on 2026-11-16.

- The deterministic core. RFC 8785 canonical encoding of JSON values, a strict parser with byte-offset errors, domain-separated SHA-256 digest framing behind a factory seam, content-addressed store contracts, run description validation, manifest construction with a Merkle root over task records, manifest verification and diffing, and garbage collection planning. Standard library only.
- The store adapters. SHA-256 behind the core seam, the crash-safe filesystem content store with its four-step atomic put protocol, a parallel scheduler with sandbox rules and process-group timeouts, the by-exec result cache, and garbage collection execution with pins and a grace period.
- The command line program. The `run`, `verify`, `show`, `diff`, and `gc` commands with text and JSON output, the exit code contract from the architecture document, and the seed printed on every invocation.
- The proof suites. Property tests at 1,000 cases on pull requests and 10,000 on main, tamper and crash recovery fault suites, pipe and timeout stress at volume, and hand-rolled benchmarks whose recorded figures live in the README.
