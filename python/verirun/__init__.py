"""Typed Python SDK and CLI wrapper for ``verirun``."""

from __future__ import annotations

from dataclasses import dataclass
import heapq
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import sysconfig
import tempfile
from types import MappingProxyType
from typing import Any, Mapping, Sequence, overload

__all__ = [
    "__version__",
    "ArtifactSummary",
    "DiffFieldChanges",
    "DiffResult",
    "GcResult",
    "ManifestSummary",
    "PinResult",
    "PipelineBuilder",
    "RunResult",
    "TaskDiffSummary",
    "TaskFailure",
    "TaskRecordSummary",
    "TaskSpec",
    "TaskSummary",
    "VerifyResult",
    "VerirunError",
    "completions",
    "diff",
    "find_binary",
    "gc",
    "pin",
    "run",
    "run_tasks",
    "show",
    "unpin",
    "verify",
    "version",
]

__version__ = "0.2.2"

PathInput = str | os.PathLike[str]

_MAX_SAFE_INTEGER = 9_007_199_254_740_991
_RESERVED_ENV_KEYS = frozenset(
    {"PATH", "HOME", "TMPDIR", "TZ", "LC_ALL", "SOURCE_DATE_EPOCH"}
)


def _case_folded(text: str) -> str:
    """Unicode lowercase mapping, applied per character.

    Mirrors the CLI's collision folding in verirun-core, which
    lowercases per character, so the two agree on every input.
    """
    return "".join(ch.lower() for ch in text)


def _win32_stripped(text: str) -> str:
    """The path with trailing spaces and dots stripped per component.

    Mirrors the CLI's Win32-stripped form. Win32 strips those
    characters from each path component when it opens a file.
    """
    return "/".join(part.rstrip(" .") for part in text.split("/"))


def _is_reserved_env_key(key: str) -> bool:
    """Whether an env key names a reserved key, folding ASCII case.

    Mirrors the CLI's comparison. Environment lookup on Windows
    is case-insensitive, so a declared Path is refused as PATH.
    """
    folded = "".join(
        chr(ord(ch) + 32) if "A" <= ch <= "Z" else ch for ch in key
    )
    return folded in _RESERVED_ENV_KEYS_FOLDED


_RESERVED_ENV_KEYS_FOLDED = frozenset(
    name.lower() for name in _RESERVED_ENV_KEYS
)
_TASK_FIELDS = frozenset(
    {
        "task_id",
        "executable",
        "args",
        "env",
        "inputs",
        "outputs",
        "depends_on",
        "timeout_ms",
    }
)
_EMPTY_ENV: Mapping[str, str] = MappingProxyType({})
_UNSET = object()


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


def _invalid_spec(message: str) -> VerirunError:
    return VerirunError(message, exit_code=3, kind="invalid_spec")


def _normalize_str_tuple(value: Any, field_name: str) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
        raise _invalid_spec(f"{field_name!r} must be a sequence of strings")
    items: list[str] = []
    for item in value:
        if not isinstance(item, str):
            raise _invalid_spec(f"{field_name!r} items must be strings")
        items.append(item)
    return tuple(items)


def _normalize_env_mapping(value: Any) -> Mapping[str, str]:
    if value is None:
        return _EMPTY_ENV
    if not isinstance(value, Mapping):
        raise _invalid_spec("'env' must be a mapping of string keys to string values")
    pairs: dict[str, str] = {}
    for key, val in value.items():
        if not isinstance(key, str) or not isinstance(val, str):
            raise _invalid_spec("'env' keys and values must be strings")
        pairs[key] = val
    if not pairs:
        return _EMPTY_ENV
    return MappingProxyType(dict(sorted(pairs.items())))


def _validate_relative_path(raw: str, *, task_id: str, field_name: str) -> str:
    """Validate a declared path and return its normalized form.

    Mirrors the CLI's path rules in verirun-core. Dot components
    and empty components are normalized away, so spellings such
    as ./seed.txt, dir//file.txt, and dir/ name the same path
    here as they do at the CLI.
    """
    if not raw:
        raise _invalid_spec(f"task {task_id!r} {field_name} path must not be empty")
    if raw.startswith("/") or (
        len(raw) >= 2 and raw[0].isascii() and raw[0].isalpha() and raw[1] == ":"
    ):
        raise _invalid_spec(
            f"task {task_id!r} {field_name} path {raw!r} must be relative"
        )
    if "\\" in raw:
        raise _invalid_spec(
            f"task {task_id!r} {field_name} path {raw!r} must not contain backslashes"
        )
    parts: list[str] = []
    for part in raw.split("/"):
        if part == "..":
            raise _invalid_spec(
                f"task {task_id!r} {field_name} path {raw!r} contains unsafe component {part!r}"
            )
        if part in {"", "."}:
            continue
        parts.append(part)
    if not parts:
        raise _invalid_spec(f"task {task_id!r} {field_name} path must not be empty")
    return "/".join(parts)


@dataclass(frozen=True, init=False, slots=True)
class TaskSpec:
    """Declarative specification for one deterministic task in a run graph."""

    task_id: str
    executable: str
    args: tuple[str, ...] = ()
    env: Mapping[str, str] = _EMPTY_ENV
    inputs: tuple[str, ...] = ()
    outputs: tuple[str, ...] = ()
    depends_on: tuple[str, ...] = ()
    timeout_ms: int | None = None

    def __init__(
        self,
        task_id: str,
        executable: str,
        args: Sequence[str] = (),
        env: Mapping[str, str] | None = None,
        inputs: Sequence[str] = (),
        outputs: Sequence[str] = (),
        depends_on: Sequence[str] = (),
        timeout_ms: int | None = None,
    ) -> None:
        if not isinstance(task_id, str):
            raise _invalid_spec("'task_id' must be a string")
        if not isinstance(executable, str):
            raise _invalid_spec("'executable' must be a string")
        if timeout_ms is not None and (
            isinstance(timeout_ms, bool) or not isinstance(timeout_ms, int)
        ):
            raise _invalid_spec("'timeout_ms' must be an integer of at least 1")
        object.__setattr__(self, "task_id", task_id)
        object.__setattr__(self, "executable", executable)
        object.__setattr__(self, "args", _normalize_str_tuple(args, "args"))
        object.__setattr__(self, "env", _normalize_env_mapping(env))
        object.__setattr__(self, "inputs", _normalize_str_tuple(inputs, "inputs"))
        object.__setattr__(self, "outputs", _normalize_str_tuple(outputs, "outputs"))
        object.__setattr__(
            self, "depends_on", _normalize_str_tuple(depends_on, "depends_on")
        )
        object.__setattr__(self, "timeout_ms", timeout_ms)

    def to_dict(self) -> dict[str, Any]:
        """Serialize this task into its ``verirun`` JSON object representation."""
        out: dict[str, Any] = {
            "task_id": self.task_id,
            "executable": self.executable,
            "args": list(self.args),
            "env": dict(self.env),
            "inputs": list(self.inputs),
            "outputs": list(self.outputs),
            "depends_on": list(self.depends_on),
        }
        if self.timeout_ms is not None:
            out["timeout_ms"] = self.timeout_ms
        return out


def _coerce_task_spec(item: TaskSpec | Mapping[str, Any]) -> TaskSpec:
    if isinstance(item, TaskSpec):
        return item
    if isinstance(item, Mapping):
        unknown = [key for key in item if key not in _TASK_FIELDS]
        if unknown:
            raise _invalid_spec(f"unknown task field {unknown[0]!r}")
        if "task_id" not in item:
            raise _invalid_spec("task is missing required field 'task_id'")
        if "executable" not in item:
            raise _invalid_spec("task is missing required field 'executable'")
        return TaskSpec(
            task_id=item["task_id"],
            executable=item["executable"],
            args=item.get("args", ()),
            env=item.get("env"),
            inputs=item.get("inputs", ()),
            outputs=item.get("outputs", ()),
            depends_on=item.get("depends_on", ()),
            timeout_ms=item.get("timeout_ms"),
        )
    raise _invalid_spec("each task must be a TaskSpec or mapping")


class PipelineBuilder:
    """Fluent builder and upfront validator for ``verirun`` task DAG specifications."""

    def __init__(
        self, tasks: Sequence[TaskSpec | Mapping[str, Any]] = ()
    ) -> None:
        if isinstance(tasks, (str, bytes, bytearray, Mapping)) or not isinstance(
            tasks, Sequence
        ):
            raise _invalid_spec("'tasks' must be a sequence of TaskSpec or mappings")
        self._tasks: list[TaskSpec] = [_coerce_task_spec(item) for item in tasks]

    @property
    def tasks(self) -> tuple[TaskSpec, ...]:
        """Return the tasks added so far in declaration order."""
        return tuple(self._tasks)

    @overload
    def add_task(self, task_id: TaskSpec, /) -> PipelineBuilder: ...

    @overload
    def add_task(
        self,
        task_id: str,
        executable: str,
        args: Sequence[str] = (),
        env: Mapping[str, str] | None = None,
        inputs: Sequence[str] = (),
        outputs: Sequence[str] = (),
        depends_on: Sequence[str] = (),
        timeout_ms: int | None = None,
    ) -> PipelineBuilder: ...

    def add_task(
        self,
        task_id: TaskSpec | Mapping[str, Any] | str,
        executable: str | object = _UNSET,
        args: Sequence[str] | object = _UNSET,
        env: Mapping[str, str] | None | object = _UNSET,
        inputs: Sequence[str] | object = _UNSET,
        outputs: Sequence[str] | object = _UNSET,
        depends_on: Sequence[str] | object = _UNSET,
        timeout_ms: int | None | object = _UNSET,
    ) -> PipelineBuilder:
        """Append a task via ``TaskSpec`` or field arguments and return ``self``."""
        extras = (executable, args, env, inputs, outputs, depends_on, timeout_ms)
        if isinstance(task_id, (TaskSpec, Mapping)):
            if any(val is not _UNSET for val in extras):
                raise _invalid_spec(
                    "cannot pass field arguments alongside a TaskSpec or mapping"
                )
            self._tasks.append(_coerce_task_spec(task_id))
            return self

        if executable is _UNSET or not isinstance(executable, str):
            raise _invalid_spec("'executable' must be a string")
        spec = TaskSpec(
            task_id=task_id,
            executable=executable,
            args=() if args is _UNSET else args,  # type: ignore[arg-type]
            env=None if env is _UNSET else env,  # type: ignore[arg-type]
            inputs=() if inputs is _UNSET else inputs,  # type: ignore[arg-type]
            outputs=() if outputs is _UNSET else outputs,  # type: ignore[arg-type]
            depends_on=() if depends_on is _UNSET else depends_on,  # type: ignore[arg-type]
            timeout_ms=None if timeout_ms is _UNSET else timeout_ms,  # type: ignore[arg-type]
        )
        self._tasks.append(spec)
        return self

    def validate(self) -> tuple[str, ...]:
        """Validate the task graph upfront and return deterministic topological order."""
        if not self._tasks:
            raise _invalid_spec("pipeline must contain at least one task")

        seen_ids: set[str] = set()
        folded_ids: dict[str, str] = {}
        stripped_ids: dict[str, str] = {}
        for task in self._tasks:
            tid = task.task_id
            if (
                not tid.strip()
                or tid in {".", ".."}
                or "/" in tid
                or "\\" in tid
                or _win32_stripped(tid) == ""
            ):
                raise _invalid_spec(
                    f"invalid task_id {tid!r}: must be a non-blank single path component"
                )
            if tid in seen_ids:
                raise _invalid_spec(f"duplicate task_id {tid!r}")
            seen_ids.add(tid)
            folded = _case_folded(tid)
            if folded in folded_ids:
                raise _invalid_spec(
                    f"task_id {tid!r} differs only in letter case from task_id {folded_ids[folded]!r}"
                )
            folded_ids[folded] = tid
            stripped = _case_folded(_win32_stripped(tid))
            if stripped in stripped_ids:
                raise _invalid_spec(
                    f"task_id {tid!r} and task_id {stripped_ids[stripped]!r} "
                    "name the same directory on Windows, where trailing "
                    "spaces and dots are stripped"
                )
            stripped_ids[stripped] = tid

            if not task.executable.strip():
                raise _invalid_spec(f"task {tid!r} executable must not be blank")

            if task.timeout_ms is not None and (
                task.timeout_ms < 1 or task.timeout_ms > _MAX_SAFE_INTEGER
            ):
                raise _invalid_spec(
                    f"task {tid!r} timeout_ms must be between 1 and {_MAX_SAFE_INTEGER}"
                )

            for key in task.env:
                if not key.strip():
                    raise _invalid_spec(f"task {tid!r} env key must not be blank")
                if _is_reserved_env_key(key):
                    raise _invalid_spec(
                        f"task {tid!r} env declares reserved key {key!r}"
                    )

        for task in self._tasks:
            for dep in task.depends_on:
                if dep == task.task_id:
                    raise _invalid_spec(
                        f"task {task.task_id!r} cannot depend on itself"
                    )
                if dep not in seen_ids:
                    raise _invalid_spec(
                        f"task {task.task_id!r} depends on unknown task {dep!r}"
                    )

        output_owners: dict[str, str] = {}
        folded_output_owners: dict[str, tuple[str, str]] = {}
        stripped_output_owners: dict[str, tuple[str, str]] = {}
        for task in self._tasks:
            seen_inputs: set[str] = set()
            folded_inputs: dict[str, str] = {}
            stripped_inputs: dict[str, str] = {}
            for raw_in in task.inputs:
                norm_in = _validate_relative_path(
                    raw_in, task_id=task.task_id, field_name="input"
                )
                if norm_in in seen_inputs:
                    raise _invalid_spec(
                        f"task {task.task_id!r} declares duplicate input {norm_in!r}"
                    )
                seen_inputs.add(norm_in)
                folded_in = _case_folded(norm_in)
                if folded_in in folded_inputs:
                    raise _invalid_spec(
                        f"task {task.task_id!r} input {norm_in!r} differs "
                        f"only in letter case from input {folded_inputs[folded_in]!r}"
                    )
                folded_inputs[folded_in] = norm_in
                stripped_in = _case_folded(_win32_stripped(norm_in))
                if stripped_in in stripped_inputs:
                    raise _invalid_spec(
                        f"task {task.task_id!r} input {norm_in!r} and input "
                        f"{stripped_inputs[stripped_in]!r} name the same file "
                        "on Windows, where trailing spaces and dots are stripped"
                    )
                stripped_inputs[stripped_in] = norm_in

            for raw_out in task.outputs:
                norm_out = _validate_relative_path(
                    raw_out, task_id=task.task_id, field_name="output"
                )
                if norm_out in output_owners:
                    first_owner = output_owners[norm_out]
                    raise _invalid_spec(
                        f"output path {norm_out!r} collides between {first_owner!r} and {task.task_id!r}"
                    )
                output_owners[norm_out] = task.task_id
                folded_out = _case_folded(norm_out)
                if folded_out in folded_output_owners:
                    first_path, first_owner = folded_output_owners[folded_out]
                    raise _invalid_spec(
                        f"output path {norm_out!r} differs only in letter "
                        f"case from output path {first_path!r} claimed by {first_owner!r}"
                    )
                folded_output_owners[folded_out] = (norm_out, task.task_id)
                stripped_out = _case_folded(_win32_stripped(norm_out))
                if stripped_out in stripped_output_owners:
                    first_path, first_owner = stripped_output_owners[stripped_out]
                    raise _invalid_spec(
                        f"output path {norm_out!r} and output path "
                        f"{first_path!r} claimed by {first_owner!r} name the "
                        "same file on Windows, where trailing spaces and "
                        "dots are stripped"
                    )
                stripped_output_owners[stripped_out] = (norm_out, task.task_id)

        deps_by_id = {
            task.task_id: sorted(set(task.depends_on)) for task in self._tasks
        }
        indegree = {tid: len(deps) for tid, deps in deps_by_id.items()}
        dependents: dict[str, list[str]] = {tid: [] for tid in deps_by_id}
        for tid, deps in deps_by_id.items():
            for dep in deps:
                dependents[dep].append(tid)

        ready = [tid for tid, deg in indegree.items() if deg == 0]
        heapq.heapify(ready)
        order: list[str] = []
        while ready:
            current = heapq.heappop(ready)
            order.append(current)
            for child in dependents[current]:
                indegree[child] -= 1
                if indegree[child] == 0:
                    heapq.heappush(ready, child)

        if len(order) != len(self._tasks):
            remnants = sorted(tid for tid, deg in indegree.items() if deg > 0)
            raise _invalid_spec(
                f"dependency cycle detected among tasks: {', '.join(remnants)}"
            )

        return tuple(order)

    def to_dict(self) -> dict[str, Any]:
        """Validate and return the run specification dictionary."""
        self.validate()
        return {"tasks": [task.to_dict() for task in self._tasks]}

    def to_json(self, *, indent: int | None = 2) -> str:
        """Validate and serialize the run specification to JSON text."""
        return json.dumps(self.to_dict(), indent=indent) + "\n"

    def write(self, path: PathInput) -> Path:
        """Validate and write the run specification JSON file to ``path``."""
        payload = self.to_json()
        target = Path(path)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(payload, encoding="utf-8")
        return target

    def run(
        self,
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
        """Validate, materialize a temporary spec file, and run via ``verirun``."""
        self.validate()
        with tempfile.TemporaryDirectory(prefix="verirun-spec-") as tmp_dir:
            spec_path = self.write(Path(tmp_dir) / "pipeline.json")
            return run(
                spec_path,
                cas=cas,
                work=work,
                inputs=inputs,
                jobs=jobs,
                pin_manifest=pin_manifest,
                seed=seed,
                check=check,
                binary=binary,
            )


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
    target_dirs = [
        repo_root / "target" / "release",
        repo_root / "target" / "debug",
    ]

    def _newest_exe_mtime(directory: Path) -> int:
        best = -1
        for name in exe_names:
            candidate = directory / name
            if candidate.is_file():
                best = max(best, candidate.stat().st_mtime_ns)
        return best

    target_dirs.sort(key=_newest_exe_mtime, reverse=True)
    search_dirs.extend(target_dirs)

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


def _probe_version(exe: Path) -> str | None:
    """Best-effort version of a resolved binary, for error messages.

    Returns the version only when the binary answers ``--version``
    in the CLI's own ``verirun <version>`` form. Any failure, and
    any other output shape, yields ``None`` and the caller's
    message stands unchanged.
    """
    try:
        proc = subprocess.run(
            [str(exe), "--version"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=False,
            timeout=10,
        )
    except (OSError, ValueError, subprocess.SubprocessError):
        return None
    if not proc.stdout:
        return None
    parts = proc.stdout.strip().split()
    if proc.returncode == 0 and len(parts) == 2 and parts[0] == "verirun":
        return parts[1]
    return None


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
        message = proc.stderr.strip() or stdout or "verirun failed"
        probed = _probe_version(exe)
        if probed is not None:
            message = f"{message} (binary version {probed}, SDK version {__version__})"
        raise VerirunError(
            message,
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


def completions(
    shell: str = "bash",
    *,
    binary: PathInput | None = None,
) -> str:
    """Return the shell completion script for ``bash``, ``zsh``, ``fish``, or ``powershell``."""
    if not isinstance(shell, str):
        raise _invalid_spec("'shell' must be a string")
    data, _ = _invoke_json(
        ["completions", shell, "--json"],
        allowed_codes=(0,),
        binary=binary,
    )
    return str(data["script"])


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


def run_tasks(
    tasks: Sequence[TaskSpec | Mapping[str, Any]],
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
    """Validate and execute an in-memory sequence of tasks via ``PipelineBuilder``."""
    return PipelineBuilder(tasks).run(
        cas=cas,
        work=work,
        inputs=inputs,
        jobs=jobs,
        pin_manifest=pin_manifest,
        seed=seed,
        check=check,
        binary=binary,
    )


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
