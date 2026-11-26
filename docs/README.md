# verirun documentation

Reference and operational documentation for `verirun` across all four workspace crates.

## Start here

| If you want to... | Read |
| --- | --- |
| Run your first task graph, cached rerun, diff, and garbage collection | [TUTORIAL.md](TUTORIAL.md) |
| See the CLI command table, benchmarks, and overview | [Root README](../README.md) |
| Embed canonical JSON, framed digests, or DAG validation in Rust | [verirun-core README](../crates/verirun-core/README.md) |
| Drive the content-addressed store and scheduler from Rust | [verirun-store README](../crates/verirun-store/README.md) |

## Specification and invariants

| Document | What it covers |
| --- | --- |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Store directory layout, crash-safe put state machine, RFC 8785 canonical JSON rules, Pattern T domain-separated hashing, hermetic sandbox rules, and exit codes. |
| [INVARIANTS.md](INVARIANTS.md) | Normative invariant catalog (`INV-CAN-*`, `INV-HASH-*`, `INV-CAS-*`, `INV-DAG-*`, `INV-RUN-*`, `INV-MAN-*`, `INV-GC-*`, `INV-CLI-*`, `INV-PLAT-*`) mapped 1-to-1 to tests and fault oracles. |
| [DECISIONS.md](../DECISIONS.md) | Locked workspace decisions covering licensing, dependency boundaries, and forever format compatibility. |
| [SECURITY.md](../SECURITY.md) | Supported versions, sandbox and path-traversal protections, threat model boundaries, and private vulnerability reporting. |

## Engineering history and audit records

| Document | What it covers |
| --- | --- |
| [ROADMAP.md](ROADMAP.md) | The roadmap the project followed after Milestone 4, with the owner's direction calls recorded in it. |
| [AUDIT_PROGRAMME.md](AUDIT_PROGRAMME.md) · [AUDIT_REGISTER.md](AUDIT_REGISTER.md) | Scope and findings of the adversarial audit programme. |
| [PERFECTION_ROADMAP.md](PERFECTION_ROADMAP.md) · [PERFECTION_REGISTER.md](PERFECTION_REGISTER.md) | The perfection programme and the Windows runtime parity work. |
| [CONTRIBUTING.md](../CONTRIBUTING.md) · [CHANGELOG.md](../CHANGELOG.md) | Local CI gate commands, prose and testing rules, and version history. |
