# Changelog

Every notable change to verirun is recorded here. Versions follow the semantic versioning policy in CONTRIBUTING.md once the first release is published.

## 0.1.0

The first release, prepared under roadmap lane R1. It publishes when the project owner decides (roadmap direction call D1).

- The deterministic core. RFC 8785 canonical encoding of JSON values, a strict parser with byte-offset errors, domain-separated SHA-256 digest framing behind a factory seam, content-addressed store contracts, run description validation, manifest construction with a Merkle root over task records, manifest verification and diffing, and garbage collection planning. Standard library only.
- The store adapters. SHA-256 behind the core seam, the crash-safe filesystem content store with its four-step atomic put protocol, a parallel scheduler with sandbox rules and process-group timeouts, the by-exec result cache, and garbage collection execution with pins and a grace period.
- The command line program. The run, verify, show, diff, and gc commands with text and JSON output, the exit code contract from the architecture document, and the seed printed on every invocation.
- The proof suites. Property tests at 1,000 cases on pull requests and 10,000 on main, tamper and crash recovery fault suites, pipe and timeout stress at volume, and hand-rolled benchmarks whose recorded figures live in the README.
