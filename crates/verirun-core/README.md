# verirun-core

`verirun-core` is the pure deterministic core of [verirun](https://github.com/david-engelmann/verirun). It implements the JSON data model, a strict recursive descent parser with byte-offset diagnostics, RFC 8785 (JCS) canonical encoding over the safe integer domain, domain-separated SHA-256 digest framing behind a streaming hasher trait, task graph validation, sandbox relative path rules, and Merkle manifest construction and diffing.

This crate depends strictly on the Rust standard library and forbids unsafe code. It performs no filesystem, process, network, clock, or randomness access. Concrete hash implementations and storage live in [`verirun-store`](https://crates.io/crates/verirun-store).

## Modules

- `value` provides the JSON value representation used by canonical encoding.
- `parse` parses byte slices strictly while rejecting duplicate keys, lone surrogates, and excessive nesting with byte offsets.
- `canonical` encodes values under RFC 8785 with UTF-16 property sorting and ECMA-262 number formatting.
- `digest` defines `Digest`, `DomainTag`, `StreamingHasher`, `HasherFactory`, and Pattern S and Pattern T tuple framing.
- `cas` provides pure path derivation, temporary file naming, put phase types, and garbage collection reachability planning.
- `dag` validates `TaskSpec` graphs in a fixed check order and computes deterministic Kahn topological schedules.
- `paths` validates and normalizes sandbox relative paths without touching the host filesystem.
- `manifest` builds, encodes, parses, verifies, and diffs `Manifest`, `TaskRecord`, and `ArtifactRecord` values.
- `error` defines all typed domain errors with full diagnostic context.

## Usage

```rust
use verirun_core::{canonical, parse};

let value = parse::parse(br#"{"b":[true,null],"a":1}"#).expect("valid JSON");
let bytes = canonical::to_vec(&value).expect("in-domain canonical JSON");
assert_eq!(bytes, br#"{"a":1,"b":[true,null]}"#);
```

## Documentation

- [Architecture specification](https://github.com/david-engelmann/verirun/blob/main/docs/ARCHITECTURE.md)
- [Invariants and traceability matrix](https://github.com/david-engelmann/verirun/blob/main/docs/INVARIANTS.md)
- [API reference on docs.rs](https://docs.rs/verirun-core)

## Licence

Dual licensed under the [MIT licence](https://github.com/david-engelmann/verirun/blob/main/LICENSE-MIT) and the [Apache License 2.0](https://github.com/david-engelmann/verirun/blob/main/LICENSE-APACHE).
