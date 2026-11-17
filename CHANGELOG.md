# Changelog

Every notable change to verirun is recorded here. Versions follow the semantic versioning policy in CONTRIBUTING.md once the first release is published.

## Unreleased

Changes from the post-completion audit programme, after 0.1.0.

- Sandbox freshness. A task's sandbox is cleared before the task executes, so an output collected for a run was always written by that run (audit finding F1, invariant INV-SBX-6).
- Work root ownership. A run holds its work root under a lock file for its whole duration, and a second run against the same work root fails with the new RunError::WorkLocked error instead of sharing sandboxes (audit finding F1, invariant INV-RUN-8). The CLI maps the refusal to exit code 3.
- Cache validation. A stored cache record is honoured only when its artifact paths are exactly the task's declared outputs, so a planted record can no longer be honoured or crash the run (audit finding F2, invariant INV-RUN-7).
- Verification memory. Manifest verification streams every referenced blob instead of buffering each one whole, so verifying a manifest over large artifacts no longer multiplies memory by the largest blobs (audit finding F13).
- Planning cost. Run planning builds its producer map and dependency sets once per run instead of rescanning the graph for every input (audit finding F14).
- Verification error taxonomy (breaking). VerifyError gains ManifestUnreadable and BlobUnreadable: a manifest or blob that exists but cannot be read is reported with the store failure named, no longer as missing (audit findings F18 and F24). Exhaustive matches on VerifyError gain the two variants.
- Stream capture errors. A pipe read error while capturing a task's output now fails the run instead of finalizing a truncated capture as if the stream had ended cleanly (audit finding F19).
- Digest length message. Digest::from_hex counts characters in its length error, as the message has always said (audit finding F20).
- Manifest duplicate rejection (breaking). Manifest reading rejects two records for one task id and two artifacts at one path in a record, shapes no run can produce, through the new ManifestError variants DuplicateTaskId and DuplicateArtifactPath (audit finding F3).
- Encode depth limit. Canonical encoding rejects a programmatically built value nested past the parser's depth limit, through the new CanonicalErrorKind variant DepthLimitExceeded, instead of recursing until the stack fails (audit finding F4).
- Duplicate input rejection (breaking). Run validation rejects a task that declares the same input path twice, through the new DagError variant DuplicateInput, mirroring the output collision rule (audit finding F5).

## 0.1.0

The first release, prepared under roadmap lane R1 and published to crates.io on 2026-11-16 under direction call D1.

- The deterministic core. RFC 8785 canonical encoding of JSON values, a strict parser with byte-offset errors, domain-separated SHA-256 digest framing behind a factory seam, content-addressed store contracts, run description validation, manifest construction with a Merkle root over task records, manifest verification and diffing, and garbage collection planning. Standard library only.
- The store adapters. SHA-256 behind the core seam, the crash-safe filesystem content store with its four-step atomic put protocol, a parallel scheduler with sandbox rules and process-group timeouts, the by-exec result cache, and garbage collection execution with pins and a grace period.
- The command line program. The run, verify, show, diff, and gc commands with text and JSON output, the exit code contract from the architecture document, and the seed printed on every invocation.
- The proof suites. Property tests at 1,000 cases on pull requests and 10,000 on main, tamper and crash recovery fault suites, pipe and timeout stress at volume, and hand-rolled benchmarks whose recorded figures live in the README.
