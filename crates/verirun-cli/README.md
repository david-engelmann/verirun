# verirun-cli

`verirun-cli` ships the `verirun` command line executable and the high-level pipeline library over [`verirun-core`](https://crates.io/crates/verirun-core) and [`verirun-store`](https://crates.io/crates/verirun-store).

## Installation

```sh
cargo install verirun-cli
```

Prebuilt release archives with `cargo-binstall` metadata are also published on [GitHub Releases](https://github.com/david-engelmann/verirun/releases).

## Commands

```text
verirun run <run-file> --cas <dir> --work <dir> [--jobs <n>] [--input <rel>=<file>] [--pin] [--seed <u64>] [--json]
verirun verify <manifest-digest> --cas <dir> [--json]
verirun show <manifest-digest> --cas <dir> [--json]
verirun diff <manifest-a> <manifest-b> --cas <dir> [--json]
verirun pin <manifest-digest> --cas <dir> [--json]
verirun unpin <manifest-digest> --cas <dir> [--json]
verirun gc --cas <dir> [--grace-secs <u64>] [--dry-run] [--json]
```

Every invocation prints the active seed on standard error and maps every outcome to a total exit code contract where `0` means success or verified, `1` means task failure or differing runs, `2` means integrity or tamper failure, and `3` means invalid input or configuration.

## Documentation

- [Step-by-step tutorial](https://github.com/david-engelmann/verirun/blob/main/docs/TUTORIAL.md)
- [Architecture specification](https://github.com/david-engelmann/verirun/blob/main/docs/ARCHITECTURE.md)
- [Invariants and traceability matrix](https://github.com/david-engelmann/verirun/blob/main/docs/INVARIANTS.md)
- [API reference on docs.rs](https://docs.rs/verirun-cli)

## Licence

Dual licensed under the [MIT licence](https://github.com/david-engelmann/verirun/blob/main/LICENSE-MIT) and the [Apache License 2.0](https://github.com/david-engelmann/verirun/blob/main/LICENSE-APACHE).
