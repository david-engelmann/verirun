"""Typed Python SDK and CLI wrapper for ``verirun``."""

from __future__ import annotations

from dataclasses import dataclass
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import sysconfig
from typing import Any, Mapping, Sequence

__all__ = [
    "__version__",
    "ArtifactSummary",
    "DiffFieldChanges",
    "DiffResult",
    "GcResult",
    "ManifestSummary",
    "PinResult",
    "RunResult",
    "TaskDiffSummary",
    "TaskFailure",
    "TaskRecordSummary",
    "TaskSummary",
    "VerifyResult",
    "VerirunError",
    "diff",
    "find_binary",
    "gc",
    "pin",
    "run",
    "show",
    "unpin",
    "verify",
    "version",
]

__version__ = "0.2.0"

PathInput = str | os.PathLike[str]


class VerirunError(RuntimeError):
    """Raised on integrity failures (exit 2), invalid inputs (exit 3), or task failures."""

    def __init__(
        self,
        message: str,
        *,
        exit_code: int,
        kind: str | None = None,
        payload: dict[str, Any] | None = None,
    ) -> None:
        super().__init__(message)
        self.exit_code = exit_code
        self.kind = kind
        self.payload = payload or {}


@dataclass(frozen=True, slots=True)
class TaskSummary:
    """Per-task status summary inside ``RunResult``."""

    task_id: str
    status: str
    exit_code: int | None


@dataclass(frozen=True, slots=True)
class TaskFailure:
    """Failure detail emitted for a failed or timed-out task."""

    task_id: str
    detail: str


@dataclass(frozen=True, slots=True)
class RunResult:
    """Outcome of ``verirun run --json``."""

    manifest: str
    exit_code: int
    pinned: bool
    tasks: tuple[TaskSummary, ...]
    failures: tuple[TaskFailure, ...]

    @property
    def passed(self) -> bool:
        """True when every task succeeded or hit a verified cache record."""
        return self.exit_code == 0

    def raise_for_failures(self) -> None:
        """Raise ``VerirunError`` when any task failed or timed out."""
        if not self.passed:
            failed = ", ".join(f"{f.task_id}: {f.detail}" for f in self.failures)
            raise VerirunError(
                f"verirun run failed ({failed or 'task failure'})",
                exit_code=self.exit_code,
            )


@dataclass(frozen=True, slots=True)
class VerifyResult:
    """Outcome of ``verirun verify --json``."""

    manifest: str
    verified: bool
    merkle_root: str


@dataclass(frozen=True, slots=True)
class ArtifactSummary:
    """One verified output artifact inside ``TaskRecordSummary``."""

    path: str
    digest: str
    bytes: int
    executable: bool


@dataclass(frozen=True, slots=True)
class TaskRecordSummary:
    """Full task record returned by ``verirun show --json``."""

    task_id: str
    status: str
    exit_code: int | None
    exec_digest: str
    stdout: str
    stderr: str
    artifacts: tuple[ArtifactSummary, ...]


@dataclass(frozen=True, slots=True)
class ManifestSummary:
    """Verified manifest returned by ``verirun show --json``."""

    manifest: str
    merkle_root: str
    tasks: tuple[TaskRecordSummary, ...]


@dataclass(frozen=True, slots=True)
class DiffFieldChanges:
    """Boolean flags naming which task record fields differed."""

    status: bool
    exit_code: bool
    artifacts: bool
    stdout: bool
    stderr: bool


@dataclass(frozen=True, slots=True)
class TaskDiffSummary:
    """One differing task between two verified manifests."""

    task_id: str
    kind: str
    fields: DiffFieldChanges | None = None


@dataclass(frozen=True, slots=True)
class DiffResult:
    """Outcome of ``verirun diff --json``."""

    first: str
    second: str
    equivalent: bool
    differences: tuple[TaskDiffSummary, ...]
    exit_code: int


@dataclass(frozen=True, slots=True)
class PinResult:
    """Outcome of ``verirun pin --json`` or ``verirun unpin --json``."""

    manifest: str
    pinned: bool
    merkle_root: str | None = None


@dataclass(frozen=True, slots=True)
class GcResult:
    """Outcome of ``verirun gc --json``."""

    dry_run: bool
    objects: int
    object_bytes: int
    tmp_files: int
    tmp_bytes: int
    reachable: int


def find_binary(binary: PathInput | None = None) -> Path:
    """Locate the bundled or installed ``verirun`` executable."""
    if binary is not None:
        candidate = Path(binary)
        if candidate.is_file():
            return candidate
        raise FileNotFoundError(f"verirun binary not found at {candidate}")

    env_override = os.environ.get("VERIRUN_BIN", "").strip()
    if env_override:
        candidate = Path(env_override)
        if candidate.is_file():
            return candidate
        raise FileNotFoundError(f"VERIRUN_BIN points to missing file {candidate}")

    exe_names = ("verirun.exe", "verirun") if os.name == "nt" else ("verirun",)
    search_dirs: list[Path] = []
    scripts_dir = sysconfig.get_path("scripts")
    if scripts_dir:
        search_dirs.append(Path(scripts_dir))
    py_dir = Path(sys.executable).resolve().parent
    search_dirs.extend([py_dir, py_dir / "Scripts", py_dir / "bin"])

    repo_root = Path(__file__).resolve().parents[2]
    search_dirs.extend(
        [
            repo_root / "target" / "release",
            repo_root / "target" / "debug",
        ]
    )

    for directory in search_dirs:
        for name in exe_names:
            candidate = directory / name
            if candidate.is_file():
                return candidate

    found = shutil.which("verirun")
    if found:
        return Path(found)

    raise FileNotFoundError(
        "could not locate verirun; install verirun or set VERIRUN_BIN"
    )


def _invoke_json(
    args: Sequence[str],
    *,
    allowed_codes: tuple[int, ...] = (0,),
    binary: PathInput | None = None,
) -> tuple[dict[str, Any], int]:
    exe = find_binary(binary)
    proc = subprocess.run(
        [str(exe), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=False,
    )
    payload: dict[str, Any] | None = None
    stdout = proc.stdout.strip()
    if stdout:
        try:
            payload = json.loads(stdout)
        except json.JSONDecodeError:
            payload = None

    if proc.returncode not in allowed_codes:
        if isinstance(payload, dict) and "error" in payload:
            raise VerirunError(
                str(payload["error"]),
                exit_code=int(payload.get("exit_code", proc.returncode)),
                kind=payload.get("kind"),
                payload=payload,
            )
        raise VerirunError(
            proc.stderr.strip() or stdout or "verirun failed",
            exit_code=proc.returncode,
        )

    if not isinstance(payload, dict):
        raise VerirunError(
            "verirun did not return a JSON object",
            exit_code=proc.returncode or 3,
        )
    return payload, proc.returncode


def version(*, binary: PathInput | None = None) -> str:
    """Return the version reported by ``verirun --version``."""
    exe = find_binary(binary)
    proc = subprocess.run(
        [str(exe), "--version"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=False,
    )
    if proc.returncode != 0:
        raise VerirunError(
            proc.stderr.strip() or "failed to query verirun version",
            exit_code=proc.returncode,
        )
    parts = proc.stdout.strip().split()
    return parts[-1] if parts else proc.stdout.strip()


def run(
    run_file: PathInput,
    *,
    cas: PathInput,
    work: PathInput,
    inputs: Mapping[str, PathInput] | None = None,
    jobs: int | None = None,
    pin_manifest: bool = False,
    seed: int | None = None,
    check: bool = False,
    binary: PathInput | None = None,
) -> RunResult:
    """Execute ``verirun run --json`` and return a typed ``RunResult``."""
    cmd: list[str] = []
    if seed is not None:
        cmd.extend(["--seed", str(seed)])
    cmd.extend(
        [
            "run",
            os.fspath(run_file),
            "--cas",
            os.fspath(cas),
            "--work",
            os.fspath(work),
            "--json",
        ]
    )
    if jobs is not None:
        cmd.extend(["--jobs", str(jobs)])
    if pin_manifest:
        cmd.append("--pin")
    if inputs:
        for rel_path, host_file in inputs.items():
            cmd.extend(["--input", f"{rel_path}={os.fspath(host_file)}"])
    data, code = _invoke_json(cmd, allowed_codes=(0, 1), binary=binary)
    tasks = tuple(
        TaskSummary(
            task_id=row["task_id"],
            status=row["status"],
            exit_code=row.get("exit_code"),
        )
        for row in data.get("tasks", ())
    )
    failures = tuple(
        TaskFailure(task_id=row["task_id"], detail=row["detail"])
        for row in data.get("failures", ())
    )
    result = RunResult(
        manifest=str(data["manifest"]),
        exit_code=int(data.get("exit_code", code)),
        pinned=bool(data.get("pinned", False)),
        tasks=tasks,
        failures=failures,
    )
    if check:
        result.raise_for_failures()
    return result


def verify(
    digest: str,
    *,
    cas: PathInput,
    binary: PathInput | None = None,
) -> VerifyResult:
    """Verify a manifest and every referenced blob in ``cas``."""
    data, _ = _invoke_json(
        ["verify", digest, "--cas", os.fspath(cas), "--json"],
        allowed_codes=(0,),
        binary=binary,
    )
    return VerifyResult(
        manifest=str(data["manifest"]),
        verified=bool(data["verified"]),
        merkle_root=str(data["merkle_root"]),
    )


def show(
    digest: str,
    *,
    cas: PathInput,
    binary: PathInput | None = None,
) -> ManifestSummary:
    """Verify and inspect a manifest's task records and artifact digests."""
    data, _ = _invoke_json(
        ["show", digest, "--cas", os.fspath(cas), "--json"],
        allowed_codes=(0,),
        binary=binary,
    )
    tasks = tuple(
        TaskRecordSummary(
            task_id=row["task_id"],
            status=row["status"],
            exit_code=row.get("exit_code"),
            exec_digest=row["exec_digest"],
            stdout=row["stdout"],
            stderr=row["stderr"],
            artifacts=tuple(
                ArtifactSummary(
                    path=art["path"],
                    digest=art["digest"],
                    bytes=int(art["bytes"]),
                    executable=bool(art["executable"]),
                )
                for art in row.get("artifacts", ())
            ),
        )
        for row in data.get("tasks", ())
    )
    return ManifestSummary(
        manifest=str(data["manifest"]),
        merkle_root=str(data["merkle_root"]),
        tasks=tasks,
    )


def diff(
    first: str,
    second: str,
    *,
    cas: PathInput,
    binary: PathInput | None = None,
) -> DiffResult:
    """Verify two manifests and compare their task records."""
    data, code = _invoke_json(
        ["diff", first, second, "--cas", os.fspath(cas), "--json"],
        allowed_codes=(0, 1),
        binary=binary,
    )
    diffs: list[TaskDiffSummary] = []
    for row in data.get("differences", ()):
        raw_fields = row.get("fields")
        fields = (
            DiffFieldChanges(
                status=bool(raw_fields["status"]),
                exit_code=bool(raw_fields["exit_code"]),
                artifacts=bool(raw_fields["artifacts"]),
                stdout=bool(raw_fields["stdout"]),
                stderr=bool(raw_fields["stderr"]),
            )
            if isinstance(raw_fields, dict)
            else None
        )
        diffs.append(
            TaskDiffSummary(
                task_id=row["task_id"],
                kind=row["kind"],
                fields=fields,
            )
        )
    return DiffResult(
        first=str(data["first"]),
        second=str(data["second"]),
        equivalent=bool(data["equivalent"]),
        differences=tuple(diffs),
        exit_code=code,
    )


def pin(
    digest: str,
    *,
    cas: PathInput,
    binary: PathInput | None = None,
) -> PinResult:
    """Verify and pin ``digest`` against garbage collection."""
    data, _ = _invoke_json(
        ["pin", digest, "--cas", os.fspath(cas), "--json"],
        allowed_codes=(0,),
        binary=binary,
    )
    return PinResult(
        manifest=str(data["manifest"]),
        pinned=bool(data["pinned"]),
        merkle_root=data.get("merkle_root"),
    )


def unpin(
    digest: str,
    *,
    cas: PathInput,
    binary: PathInput | None = None,
) -> PinResult:
    """Remove a garbage-collection pin from ``digest``."""
    data, _ = _invoke_json(
        ["unpin", digest, "--cas", os.fspath(cas), "--json"],
        allowed_codes=(0,),
        binary=binary,
    )
    return PinResult(
        manifest=str(data["manifest"]),
        pinned=bool(data["pinned"]),
    )


def gc(
    *,
    cas: PathInput,
    grace_secs: int = 3600,
    dry_run: bool = False,
    binary: PathInput | None = None,
) -> GcResult:
    """Sweep unreachable store objects older than ``grace_secs``."""
    cmd = [
        "gc",
        "--cas",
        os.fspath(cas),
        "--grace-secs",
        str(grace_secs),
        "--json",
    ]
    if dry_run:
        cmd.append("--dry-run")
    data, _ = _invoke_json(cmd, allowed_codes=(0,), binary=binary)
    return GcResult(
        dry_run=bool(data["dry_run"]),
        objects=int(data["objects"]),
        object_bytes=int(data["object_bytes"]),
        tmp_files=int(data["tmp_files"]),
        tmp_bytes=int(data["tmp_bytes"]),
        reachable=int(data["reachable"]),
    )
