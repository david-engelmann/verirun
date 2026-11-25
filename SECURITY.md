# Security policy

## Supported versions

Security fixes land on the latest minor release line of `verirun` (`0.2.x`). Because the canonical encoding rules, domain-separated digest tags, and version 1 manifest format are frozen across releases, upgrading within or across minor releases preserves verification of existing stores.

## Threat model and boundaries

`verirun` enforces deterministic execution hygiene and cryptographic store integrity rather than kernel-level sandboxing against a hostile local root or kernel adversary.

- Every blob, artifact record, task record, and manifest commits to domain-separated SHA-256 digests over RFC 8785 canonical JSON. `verirun verify` and `verirun show` re-hash every referenced object and check recorded artifact byte lengths and Merkle roots before trusting stored data.
- Declared input and output paths are restricted to normalized relative paths inside the task sandbox. Output collection walks components through held directory handles with `O_NOFOLLOW` on Unix and `FILE_OPEN_REPARSE_POINT` inspection on Windows, preventing check-then-open symlink swaps.
- Child processes receive a cleared environment containing only declared pairs, the three fixed defaults (`LC_ALL=C`, `SOURCE_DATE_EPOCH=0`, `TZ=UTC`), and the private sandbox `TMPDIR`. Tasks execute in dedicated Unix process groups or Windows job objects that are terminated on timeout and on drop.
- Tasks execute as normal OS subprocesses under the invoking user account. Running untrusted code that requires network, syscall, or mandatory access isolation should wrap `verirun` inside a container, virtual machine, or OS sandbox.

## Reporting a vulnerability

Please report suspected vulnerabilities privately through [GitHub Security Advisories](https://github.com/david-engelmann/verirun/security/advisories/new) rather than opening a public issue. Include a minimal reproduction, the affected crate and version, and the invariant in [docs/INVARIANTS.md](docs/INVARIANTS.md) that the issue violates or bypasses. Reports receive an initial response within seven days.
