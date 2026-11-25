# Decisions

Locked by David on 2026-11-12. Any change to a decision in this file needs a new explicit ruling from the project owner. Implementation work does not reinterpret these decisions.

- Q1 GitHub owner and namespace is `david-engelmann` on GitHub.
- Q2 Repository visibility is public from creation.
- Q3 Licence is dual `MIT OR Apache-2.0`.
- Q4 Rust toolchain and edition use Rust 2024 edition with a pinned stable toolchain in `rust-toolchain.toml` and `Cargo.lock` committed.
- Q5 Zero-dependency core rule holds strictly. `verirun-core` uses the Rust standard library only. External crates (`sha2`, `clap`, `serde_json`, `proptest`, plus target-gated `libc` on Unix and `windows-sys` on Windows inside `verirun-platform`) are restricted to OS boundaries, I/O adapters, the CLI, and dev-dependencies.
- Q6 CI seed stress volume runs 1,000 cases per pull request and 10,000 cases on `main` through `PROPTEST_CASES` and `SIM_SEED`.
- Q7 Formal invariant traceability matrix in `docs/INVARIANTS.md` maps every safety, durability, platform, and latency invariant one-to-one to the property tests, fault oracles, or benchmarks enforcing it.
- Q8 Repository scope is isolated to `verirun` with all changes gated by `verirun` CI.
