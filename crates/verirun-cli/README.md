# verirun-cli

`verirun-cli` ships the `verirun` command line executable and the high-level pipeline library over [`verirun-core`](https://crates.io/crates/verirun-core) and [`verirun-store`](https://crates.io/crates/verirun-store).

## Installation

Install from crates.io with Cargo.

```sh
cargo install verirun-cli
```

On Linux and macOS (`x86_64` and `aarch64`), the standalone `install.sh` script downloads a prebuilt release archive and `SHA256SUMS` from [GitHub Releases](https://github.com/david-engelmann/verirun/releases) and verifies the archive SHA-256 digest before extracting `verirun` into `$HOME/.local/bin`.

```sh
curl -fsSL https://raw.githubusercontent.com/david-engelmann/verirun/main/install.sh | sh
```

Pass `--version` or `--install-dir` to pin a release tag or target directory.

```sh
curl -fsSL https://raw.githubusercontent.com/david-engelmann/verirun/main/install.sh | sh -s -- --version v0.2.2 --install-dir "$HOME/.local/bin"
```

Prebuilt release archives also include `cargo-binstall` metadata.

## Commands

```text
verirun run <run-file> --cas <dir> --work <dir> [--jobs <n>] [--input <rel>=<file>] [--pin] [--seed <u64>] [--json]
verirun verify <manifest-digest> --cas <dir> [--json]
verirun show <manifest-digest> --cas <dir> [--json]
verirun diff <manifest-a> <manifest-b> --cas <dir> [--json]
verirun pin <manifest-digest> --cas <dir> [--json]
verirun unpin <manifest-digest> --cas <dir> [--json]
verirun gc --cas <dir> [--grace-secs <u64>] [--dry-run] [--json]
verirun completions <bash|zsh|fish|powershell> [--json]
```

Every store or execution invocation prints the active seed on standard error and maps every outcome to a total exit code contract where `0` means success or verified, `1` means task failure or differing runs, `2` means integrity or tamper failure, and `3` means invalid input or configuration.

## Shell completions

Generate deterministic tab-completion scripts for `bash`, `zsh`, `fish`, or `powershell` directly from the binary without opening a store.

```sh
# Bash
eval "$(verirun completions bash)"

# Zsh
source <(verirun completions zsh)

# Fish
verirun completions fish | source

# PowerShell
verirun completions powershell | Out-String | Invoke-Expression
```

## Documentation

- [Step-by-step tutorial](https://github.com/david-engelmann/verirun/blob/main/docs/TUTORIAL.md)
- [Architecture specification](https://github.com/david-engelmann/verirun/blob/main/docs/ARCHITECTURE.md)
- [Invariants and traceability matrix](https://github.com/david-engelmann/verirun/blob/main/docs/INVARIANTS.md)
- [API reference on docs.rs](https://docs.rs/verirun-cli)

## Licence

Dual licensed under the [MIT licence](https://github.com/david-engelmann/verirun/blob/main/LICENSE-MIT) and the [Apache License 2.0](https://github.com/david-engelmann/verirun/blob/main/LICENSE-APACHE).
