# verirun Documentation Index

Normative specifications, step-by-step walkthroughs, and crate references for `verirun`. The rendered documentation book is published at <https://david-engelmann.github.io/verirun/>.

## Guides and Specifications

| Document | Contents |
| --- | --- |
| [TUTORIAL.md](TUTORIAL.md) | End-to-end CLI walkthrough covering cold DAG execution, verified `by-exec` caching, `verify`, `show`, `diff`, `pin`, and `gc`. |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Normative specification for crate boundaries, RFC 8785 canonical JSON, `verirun/v1/*` domain-separated hashing, 4-step atomic CAS puts, hermetic sandboxes, and exit codes. |
| [INVARIANTS.md](INVARIANTS.md) | Formal invariant catalog (`INV-CAN-*`, `INV-HASH-*`, `INV-CAS-*`, `INV-DAG-*`, `INV-RUN-*`, `INV-MAN-*`, `INV-GC-*`, `INV-CLI-*`, `INV-PLAT-*`) mapped 1-to-1 to tests and fault oracles. |
| [COMPARISON.md](COMPARISON.md) | Trade-offs against `make`/`just`, `Bazel`/`Buck2`/`Nix`, container workflow runners, and ad-hoc Python eval scripts. |
| [FAQ.md](FAQ.md) | Operational answers on cold vs warm manifests (`INV-RUN-7`), IEEE-754 safe-integer bounds, concurrent `--cas` sharing, and garbage collection. |

## Crate & SDK References

| Document | Contents |
| --- | --- |
| [Root README](../README.md) | Python `PipelineBuilder` and CLI quickstart, pipeline and Merkle diagrams, exit-code contract, and throughput baselines. |
| [verirun-core README](../crates/verirun-core/README.md) | Zero-dependency JSON parser, RFC 8785 encoder, digest framing, DAG validator, and manifest model. |
| [verirun-platform README](../crates/verirun-platform/README.md) | Unix process groups / Windows Job Objects, no-follow `DirHandle`, and atomic `rename_replace`. |
| [verirun-store README](../crates/verirun-store/README.md) | Sharded filesystem CAS, subprocess runner, deterministic scheduler, `by-exec` cache, and `gc`. |
| [examples/README.md](../examples/README.md) · [AGENTS.md](../AGENTS.md) | Runnable Python and POSIX shell pipelines plus agent integration rules. |

## Governance and Programme Records

| Document | Contents |
| --- | --- |
| [DECISIONS.md](../DECISIONS.md) · [SECURITY.md](../SECURITY.md) | Dependency boundaries, forever-format rules, sandbox threat model, and vulnerability reporting. |
| [CONTRIBUTING.md](../CONTRIBUTING.md) · [CHANGELOG.md](../CHANGELOG.md) | Local verification targets, coding invariants, and release notes. |
| [ROADMAP.md](ROADMAP.md) · [AUDIT_PROGRAMME.md](AUDIT_PROGRAMME.md) · [AUDIT_REGISTER.md](AUDIT_REGISTER.md) · [PERFECTION_ROADMAP.md](PERFECTION_ROADMAP.md) · [PERFECTION_REGISTER.md](PERFECTION_REGISTER.md) · [SUCCESSOR_ROADMAP.md](SUCCESSOR_ROADMAP.md) · [SUCCESSOR_REGISTER.md](SUCCESSOR_REGISTER.md) | Historical engineering roadmaps and adversarial audit closure registers. |
