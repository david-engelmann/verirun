# verirun-store

`verirun-store` holds the storage, execution, scheduling, and garbage collection adapters of [verirun](https://github.com/david-engelmann/verirun). It pairs [`verirun-core`](https://crates.io/crates/verirun-core) with SHA-256 hashing and [`verirun-platform`](https://crates.io/crates/verirun-platform) while forbidding unsafe code throughout its own sources.

## Modules

- `sha256` implements `StreamingHasher` and `HasherFactory` over `sha2`.
- `cas_fs` implements the crash-safe filesystem content-addressed store with its four-step atomic put protocol, streaming verified reads, manifest pins, and `by-exec` cache paths.
- `runner` executes tasks in cleared per-task sandboxes with a hermetic environment, concurrent pipe drains, process-tree timeouts, and handle-relative artifact collection on Unix and Windows.
- `scheduler` coordinates parallel workers in deterministic lexicographic ready order, validates and publishes `by-exec` cache entries, propagates failures to transitive dependents, and produces the run manifest.
- `gc` marks reachable objects from verified pinned manifests and sweeps unreachable objects and stale temporary files older than the grace threshold under an exclusive collection lock.
- `clock` isolates the workspace's single wall-clock read for garbage collection ages and per-process temporary nonces.
- `error` provides typed `StoreError`, `RunError`, and `GcError` diagnostics naming the path and protocol step involved.

## Usage

```rust
use verirun_store::cas_fs::Cas;

let root = std::env::temp_dir().join("verirun-doc-store");
let cas = Cas::open(&root).expect("open store");
let digest = cas.put_bytes(b"deterministic payload").expect("put bytes");
let bytes = cas.read_to_vec(&digest).expect("verified read");
assert_eq!(bytes, b"deterministic payload");
```

## Documentation

- [Architecture specification](https://github.com/david-engelmann/verirun/blob/main/docs/ARCHITECTURE.md)
- [Invariants and traceability matrix](https://github.com/david-engelmann/verirun/blob/main/docs/INVARIANTS.md)
- [API reference on docs.rs](https://docs.rs/verirun-store)

## Licence

Dual licensed under the [MIT licence](https://github.com/david-engelmann/verirun/blob/main/LICENSE-MIT) and the [Apache License 2.0](https://github.com/david-engelmann/verirun/blob/main/LICENSE-APACHE).
