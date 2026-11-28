## Summary

Describe the defect fixed or capability added and name the section of `docs/ARCHITECTURE.md` or invariant in `docs/INVARIANTS.md` that governs the change.

## Verification checklist

- [ ] `cargo fmt --all -- --check` passes, including the fuzz workspace (`cargo fmt --manifest-path fuzz/Cargo.toml --all -- --check`)
- [ ] `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passes
- [ ] `SIM_SEED=20261112 PROPTEST_CASES=1000 cargo test --workspace --all-features --locked --no-fail-fast` passes
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked` passes
- [ ] `cargo bench --workspace --locked -- --test` passes
- [ ] `cargo deny check` passes
- [ ] `cargo audit` passes
- [ ] `cargo build --locked -p verirun-cli --bin verirun && python3 -m unittest discover -s python/tests -v` passes
- [ ] `verirun-core` remains zero external dependencies and frozen digest tags and manifest formats remain compatible
