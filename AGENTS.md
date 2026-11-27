# Driving and modifying verirun with AI agents

This guide has two parts: first, how an AI agent or automated harness should **drive** `verirun` via its CLI or Python SDK; second, the invariants to preserve when **editing** this repository.

## 1. Driving `verirun` from an agent

Prefer structured `--json` output on the CLI or the typed `verirun` Python 3.10+ SDK (`python/verirun/__init__.py`). Never scrape human text output when `--json` or the Python SDK is available.

### Exit-code contract (`INV-CLI-1`)

Every `verirun` invocation terminates with one of four exit codes:

| Exit code | Class | Agent action |
| --- | --- | --- |
| `0` | Success / verified | Parse JSON stdout (`manifest`, `merkle_root`, `tasks`, `verified`, `equivalent`, or `reachable`). |
| `1` | Task failure or manifest diff | On `run`: inspect `failures` and per-task `status` (`failed`, `timed_out`, `skipped`). On `diff`: inspect `differences`. |
| `2` | Cryptographic / store integrity failure | Stop and report corruption (`blob_mismatch`, `manifest_digest_mismatch`, `root_mismatch`, `blob_missing`, `artifact_size_mismatch`). Never treat exit `2` as a transient retryable error. |
| `3` | Invalid input, spec, or setup | Fix the CLI arguments, JSON syntax, DAG cycle, colliding output, reserved `env` key, unsafe path, or missing `--input` binding before retrying. |

CLI usage errors exit with code `3`, never `2`.

### Python SDK quick pattern

```python
from pathlib import Path
import verirun

pipeline = (
    verirun.PipelineBuilder()
    .add_task(
        task_id="prepare",
        executable="sh",
        args=["-c", "sort inputs/raw.txt > outputs/sorted.txt"],
        inputs=["raw.txt"],
        outputs=["sorted.txt"],
        timeout_ms=15_000,
    )
    .add_task(
        task_id="count",
        executable="sh",
        args=["-c", "awk 'END { print NR }' inputs/sorted.txt > outputs/count.txt"],
        inputs=["sorted.txt"],
        outputs=["count.txt"],
        depends_on=["prepare"],
    )
)

# Validate topological order upfront before touching the filesystem:
pipeline.validate()

result = pipeline.run(
    cas=Path("./cas"),
    work=Path("./work"),
    inputs={"raw.txt": Path("./raw.txt")},
    jobs=4,
    pin_manifest=True,
    check=True,
)
verirun.verify(result.manifest, cas=Path("./cas"))
```

### Hermeticity and specification rules

When generating JSON specs or `TaskSpec` objects, obey these rules so upfront validation does not reject the graph with exit code `3`:

1. **Sandbox paths.** Inside its sandbox, a task reads staged inputs at `inputs/<rel_path>` and writes promised outputs at `outputs/<rel_path>`. Every declared path in `inputs` and `outputs` must be relative, forward-slash separated, non-empty, free of `.` and `..` components, and free of symlinks (`INV-SBX-1`, `INV-SBX-2`, `INV-SBX-3`).
2. **Unique outputs across the graph.** No two tasks in one run may declare the same normalized output path (`INV-DAG-4`).
3. **Scrubbed environment.** Tasks execute with an empty environment plus declared `env` pairs, `LC_ALL=C`, `TZ=UTC`, `SOURCE_DATE_EPOCH=0`, and a private sandbox `TMPDIR` (`INV-RUN-1`, `INV-RUN-2`). Do not declare `PATH`, `HOME`, `TMPDIR`, `TZ`, `LC_ALL`, or `SOURCE_DATE_EPOCH` inside task `env`.
4. **Strict RFC 8785 JSON.** Duplicate object keys are rejected, unknown task fields are rejected, and integer literals must stay within IEEE-754 safe integers (`[-9007199254740991, 9007199254740991]`, `INV-CAN-3`).
5. **Cold vs warm manifest digests.** A cached task records `status: "cached"` rather than `"succeeded"` (`INV-RUN-7`), while `exec_digest`, `stdout`, `stderr`, and `artifacts` stay byte-identical. Two cold runs (or two warm runs) at any `--jobs` count produce identical manifest digests.
6. **Concurrent runs.** Multiple processes may share one `--cas` directory safely (`INV-CAS-5`), provided each active run uses its own `--work` directory (`INV-RUN-8`).

## 2. Working on this repository

Read these documents in order when orienting to the codebase:

| Where | What is there |
| --- | --- |
| [docs/TUTORIAL.md](docs/TUTORIAL.md) | End-to-end CLI walkthrough of `run`, `verify`, `show`, `diff`, `pin`, and `gc` |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Normative specification for crate boundaries, RFC 8785 canonical JSON, Pattern T hashing, CAS put protocol, and Merkle manifests |
| [docs/INVARIANTS.md](docs/INVARIANTS.md) | Formal invariant catalog (`INV-*`) and 1-to-1 test traceability matrix |
| [docs/COMPARISON.md](docs/COMPARISON.md) and [docs/FAQ.md](docs/FAQ.md) | Design scope, explicit non-goals, and operational edge cases |
| [examples/](examples/) | Runnable Python SDK and POSIX CLI pipelines verified in CI |
| [CONTRIBUTING.md](CONTRIBUTING.md) and [DECISIONS.md](DECISIONS.md) | Dependency rules, forever-format rules, and contributor gate commands |

Skip `docs/*REGISTER*.md`, `docs/*ROADMAP*.md`, and `docs/AUDIT_PROGRAMME.md` unless you are specifically asked about historical audit findings. Those files are maintainer programme registers and historical ledgers rather than active API or user documentation.

### Repository invariants enforced in CI

- **Crate dependency boundaries:** `verirun-core` is strictly `std`-only with `#![forbid(unsafe_code)]`. OS FFI `unsafe` code is confined exclusively to `verirun-platform`.
- **Markdown copy bar:** Zero em dashes (`U+2014`) or en dashes (`U+2013`) in any `.md` file in the repository.
- **README links:** Every link in `README.md` must be a canonical `https://` URL so PyPI renders the package description without broken relative links.
- **Python floor:** `python/` and `examples/*.py` must stay compatible with Python `3.10` using only the Python standard library.
- **Full local gate:** Run `make ci` (or individual targets `make fmt`, `make lint`, `make test`, `make doc`, `make python-test`, `make examples`, `make fuzz-check`, `make book`) before proposing changes.
