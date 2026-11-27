# CLI and Python SDK

Every `verirun` subcommand supports both human-readable output and machine-parsed `--json` output with identical information content (`INV-CLI-2`), backed by a total exit-code contract (`INV-CLI-1`).

## Exit-code contract

| Exit code | Class | When `verirun` returns it |
| --- | --- | --- |
| `0` | Success / verified | All tasks succeeded or hit verified cache (`run`); manifest and blobs verified (`verify`, `show`, `pin`); runs are equivalent (`diff`); `unpin`, `gc`, or `completions` completed. |
| `1` | Task failure or run diff | One or more tasks failed or timed out (`run`), or two verified manifests differ in status, exit code, streams, or artifacts (`diff`). |
| `2` | Integrity failure | Corrupt or missing manifest, non-canonical manifest bytes, Merkle root mismatch, corrupt or missing referenced blob, or artifact size mismatch (`verify`, `show`, `diff`, `pin`, `gc`, `run`). |
| `3` | Invalid input or setup | CLI flag error, malformed JSON, duplicate JSON keys, out-of-range integer, DAG cycle, unknown dependency, colliding output, unsafe path, reserved `env` key, missing `--input` host file, or active lock. |

## CLI subcommands

```text
verirun [--seed <N>] run <RUN_FILE> --cas <DIR> --work <DIR> [--jobs <N>] [--input <REL_PATH>=<HOST_FILE>]... [--pin] [--json]
verirun [--seed <N>] verify <MANIFEST_DIGEST> --cas <DIR> [--json]
verirun [--seed <N>] show <MANIFEST_DIGEST> --cas <DIR> [--json]
verirun [--seed <N>] diff <MANIFEST_A> <MANIFEST_B> --cas <DIR> [--json]
verirun [--seed <N>] pin <MANIFEST_DIGEST> --cas <DIR> [--json]
verirun [--seed <N>] unpin <MANIFEST_DIGEST> --cas <DIR> [--json]
verirun [--seed <N>] gc --cas <DIR> [--grace-secs <N>] [--dry-run] [--json]
verirun completions <bash|zsh|fish|powershell> [--json]
```

### JSON output shapes

- **`verirun run --json`**:
  ```json
  {
    "manifest": "<64-hex-sha256>",
    "exit_code": 0,
    "pinned": true,
    "tasks": [
      {"task_id": "greet", "status": "succeeded", "exit_code": 0}
    ],
    "failures": []
  }
  ```
- **`verirun verify --json`**:
  ```json
  {
    "manifest": "<64-hex-sha256>",
    "verified": true,
    "merkle_root": "<64-hex-sha256>"
  }
  ```
- **`verirun show --json`**: verifies the manifest and all referenced blobs first (`INV-CLI-5`), then emits `manifest`, `merkle_root`, and `tasks` (each with `task_id`, `status`, `exit_code`, `exec_digest`, `stdout`, `stderr`, and `artifacts`).
- **`verirun diff --json`**: verifies both manifests first (`INV-CLI-4`), then emits `first`, `second`, `equivalent`, and `differences` (each naming `task_id`, `kind`, and changed `fields`).
- **`verirun gc --json`**: emits `dry_run`, `objects`, `object_bytes`, `tmp_files`, `tmp_bytes`, and `reachable`.
- **Failure payloads (`--json`)**: any exit `2` or `3` error emits `{"error": "...", "exit_code": 2|3, ...}` on standard output in addition to the diagnostic line on standard error. Verification failures include structured `kind` (`blob_mismatch`, `manifest_digest_mismatch`, `root_mismatch`, `blob_missing`, `artifact_size_mismatch`, etc.) and offending digests.

## Typed Python SDK (`verirun`)

The Python SDK targets Python `>=3.10` with zero third-party dependencies and full `mypy` / `pyright` type annotations.

### Building and validating task graphs

`TaskSpec` and `PipelineBuilder` mirror core validation rules in pure Python before invoking the binary:

```python
from pathlib import Path
import verirun

builder = (
    verirun.PipelineBuilder()
    .add_task(
        task_id="prepare",
        executable="sh",
        args=["-c", "sort inputs/raw.csv > outputs/sorted.csv"],
        env={"DATASET_SPLIT": "eval"},
        inputs=["raw.csv"],
        outputs=["sorted.csv"],
        timeout_ms=10_000,
    )
    .add_task(
        task_id="summarize",
        executable="sh",
        args=["-c", "awk 'END { print NR }' inputs/sorted.csv > outputs/count.txt"],
        inputs=["sorted.csv"],
        outputs=["count.txt"],
        depends_on=["prepare"],
    )
)

# Deterministic topological order without touching disk:
order = builder.validate()  # ('prepare', 'summarize')

# Execute with host input binding and automatic failure check:
run_res = builder.run(
    cas=Path("./cas"),
    work=Path("./work"),
    inputs={"raw.csv": Path("./data/raw.csv")},
    jobs=4,
    pin_manifest=True,
    check=True,
)
```

### SDK functions and return types

| Function | Returns | Notes |
| --- | --- | --- |
| `verirun.run(run_file, *, cas, work, inputs=None, jobs=None, pin_manifest=False, seed=None, check=False)` | `RunResult` | Pass `check=True` to raise `VerirunError(exit_code=1)` if any task fails or times out. |
| `verirun.run_tasks(tasks, *, cas, work, ...)` | `RunResult` | Shorthand for `PipelineBuilder(tasks).run(...)`. |
| `verirun.verify(digest, *, cas)` | `VerifyResult` | Raises `VerirunError(exit_code=2)` on any integrity failure. |
| `verirun.show(digest, *, cas)` | `ManifestSummary` | Verifies first, then returns typed `TaskRecordSummary` and `ArtifactSummary` tuples. |
| `verirun.diff(first, second, *, cas)` | `DiffResult` | Returns `equivalent: bool`, `exit_code` (`0` or `1`), and `differences`; raises on exit `2`/`3`. |
| `verirun.pin(digest, *, cas)` / `verirun.unpin(digest, *, cas)` | `PinResult` | Manages garbage-collection root pins under `<cas>/pins/`. |
| `verirun.gc(*, cas, grace_secs=3600, dry_run=False)` | `GcResult` | Sweeps unreachable objects older than `grace_secs`. |
| `verirun.completions(shell="bash")` | `str` | Returns completion script text for `bash`, `zsh`, `fish`, or `powershell`. |
| `verirun.version()` / `verirun.find_binary()` | `str` / `Path` | Queries binary version or resolves `VERIRUN_BIN`, wheel scripts, `target/`, or `PATH`. |

### Catching integrity vs specification errors

```python
try:
    verirun.verify(manifest_digest, cas="./cas")
except verirun.VerirunError as err:
    if err.exit_code == 2:
        # Cryptographic / disk integrity conviction (e.g. err.kind == "blob_mismatch")
        raise RuntimeError(f"tamper detected ({err.kind}): {err}") from err
    raise
```
