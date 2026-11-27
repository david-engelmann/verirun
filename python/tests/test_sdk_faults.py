from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
if importlib.util.find_spec("verirun") is None:
    sys.path.insert(0, str(ROOT / "python"))

import verirun  # noqa: E402

DIGEST = "0" * 64

STUB_SOURCE = '''\
import sys

args = sys.argv[1:]
if args == ["--version"]:
    print("verirun 0.1.0")
    raise SystemExit(0)
if "pin" in args:
    print("error: unrecognized subcommand 'pin'", file=sys.stderr)
    raise SystemExit(3)
print("error: unexpected argument found", file=sys.stderr)
raise SystemExit(3)
'''

ALWAYS_FAIL_SOURCE = '''\
import sys

print("error: this binary always fails", file=sys.stderr)
raise SystemExit(3)
'''

EXIT_ONE_SOURCE = '''\
import sys

print("error: this binary fails at exit code 1", file=sys.stderr)
raise SystemExit(1)
'''

GARBAGE_SOURCE = '''\
print("this is not json")
'''

LIST_SOURCE = '''\
print("[1, 2, 3]")
'''

NONUTF8_SOURCE = '''\
import os
import sys

args = sys.argv[1:]
if args == ["--version"]:
    os.write(1, b"verirun \\xff\\xfe\\n")
    raise SystemExit(0)
print("error: unexpected argument found", file=sys.stderr)
raise SystemExit(3)
'''


def write_stub(directory: Path, source: str, stem: str) -> Path:
    """Create an executable stub from ``source`` under ``directory``.

    The wrapper is a shell script on POSIX and a batch file on
    Windows, both delegating to the running interpreter, so the
    suite exercises the SDK's real subprocess path on every
    platform.
    """
    script = directory / f"{stem}.py"
    script.write_text(source, encoding="utf-8")
    if os.name == "nt":
        wrapper = directory / f"{stem}.bat"
        wrapper.write_text(
            f'@echo off\n"{sys.executable}" "{script}" %*\n', encoding="utf-8"
        )
    else:
        wrapper = directory / stem
        wrapper.write_text(
            f'#!/bin/sh\nexec "{sys.executable}" "{script}" "$@"\n',
            encoding="utf-8",
        )
        wrapper.chmod(wrapper.stat().st_mode | stat.S_IXUSR)
    return wrapper


def write_spec(directory: Path) -> Path:
    spec_path = directory / "pipeline.json"
    spec = {
        "tasks": [
            {
                "task_id": "upper",
                "executable": "sh",
                "args": ["-c", "true"],
                "env": {},
                "inputs": [],
                "outputs": [],
                "depends_on": [],
            }
        ]
    }
    spec_path.write_text(json.dumps(spec), encoding="utf-8")
    return spec_path


def call_each(binary: Path) -> dict[str, object]:
    """Call every process-backed public function against ``binary``.

    Returns a mapping of function name to the exception it raised.
    ``version`` is exercised separately by the fixtures that can
    fail it, since a binary that answers ``--version`` at all does
    not fail that call, and ``find_binary`` never executes the
    binary it resolves.
    """
    outcomes: dict[str, object] = {}
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        spec_path = write_spec(root)
        calls = {
            "run": lambda: verirun.run(
                spec_path, cas=root / "cas", work=root / "work", binary=binary
            ),
            "verify": lambda: verirun.verify(DIGEST, cas=root, binary=binary),
            "show": lambda: verirun.show(DIGEST, cas=root, binary=binary),
            "diff": lambda: verirun.diff(DIGEST, DIGEST, cas=root, binary=binary),
            "pin": lambda: verirun.pin(DIGEST, cas=root, binary=binary),
            "unpin": lambda: verirun.unpin(DIGEST, cas=root, binary=binary),
            "gc": lambda: verirun.gc(cas=root, binary=binary),
        }
        for name, call in calls.items():
            try:
                call()
            except Exception as exc:
                outcomes[name] = exc
            else:
                outcomes[name] = None
    return outcomes


class MissingBinaryTests(unittest.TestCase):
    """A missing binary raises FileNotFoundError from every function."""

    def test_find_binary_rejects_a_missing_explicit_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            missing = Path(tmp) / "no-such-verirun"
            with self.assertRaises(FileNotFoundError):
                verirun.find_binary(missing)

    def test_every_function_raises_file_not_found(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            missing = Path(tmp) / "no-such-verirun"
            outcomes = call_each(missing)
            version_error: object = None
            try:
                verirun.version(binary=missing)
            except Exception as exc:
                version_error = exc
            outcomes["version"] = version_error
        for name, outcome in outcomes.items():
            self.assertIsInstance(
                outcome, FileNotFoundError, f"{name} raised {outcome!r}"
            )


class FailingBinaryTests(unittest.TestCase):
    """Binaries that run but fail without JSON yield VerirunError.

    The Python interpreter itself stands in as one failing binary: it
    runs, rejects the CLI's arguments on stderr, exits nonzero, and
    never emits a payload. Its own ``--version`` output is not in the
    CLI's form, so no version diagnostic may be attached. Two stubs
    pin the exact exit codes the interpreter cannot produce, 1 and 3,
    and the always-failing stub also fails ``--version`` itself, so
    ``version`` has a failing fixture of its own.
    """

    def test_every_function_raises_verirun_error(self) -> None:
        outcomes = call_each(Path(sys.executable))
        for name, outcome in outcomes.items():
            self.assertIsInstance(
                outcome, verirun.VerirunError, f"{name} raised {outcome!r}"
            )
            self.assertNotEqual(outcome.exit_code, 0, name)
            self.assertIsNone(outcome.kind, name)
            self.assertNotIn("binary version", str(outcome), name)

    def test_exit_code_1_is_preserved_exactly(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            stub = write_stub(Path(tmp), EXIT_ONE_SOURCE, "verirun-exit-one")
            outcomes = call_each(stub)
        for name, outcome in outcomes.items():
            self.assertIsInstance(
                outcome, verirun.VerirunError, f"{name} raised {outcome!r}"
            )
            self.assertEqual(outcome.exit_code, 1, name)

    def test_version_against_an_always_failing_binary(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            stub = write_stub(Path(tmp), ALWAYS_FAIL_SOURCE, "verirun-always-fail")
            with self.assertRaises(verirun.VerirunError) as ctx:
                verirun.version(binary=stub)
        self.assertEqual(ctx.exception.exit_code, 3)


class MalformedPayloadTests(unittest.TestCase):
    """A successful exit with unusable stdout yields VerirunError.

    Both a non-JSON reply and a JSON reply that is not an object
    reach the SDK's not-a-JSON-object path, which must raise rather
    than return a half-built result.
    """

    def test_non_json_stdout_raises(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            stub = write_stub(Path(tmp), GARBAGE_SOURCE, "verirun-garbage")
            outcomes = call_each(stub)
        for name, outcome in outcomes.items():
            self.assertIsInstance(
                outcome, verirun.VerirunError, f"{name} raised {outcome!r}"
            )
            self.assertIn("did not return a JSON object", str(outcome), name)

    def test_non_object_json_stdout_raises(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            stub = write_stub(Path(tmp), LIST_SOURCE, "verirun-list")
            with self.assertRaises(verirun.VerirunError) as ctx:
                verirun.verify(DIGEST, cas=Path(tmp), binary=stub)
        self.assertIn("did not return a JSON object", str(ctx.exception))


class WrongVersionBinaryTests(unittest.TestCase):
    """A 0.1.0-shaped binary fails loudly and names both versions.

    S1.2 probed the real 0.1.0 binary on pin and on run with bound
    inputs; this stub mimics those two probed shapes and extends the
    same payload-less exit 3 shape to the remaining functions, so the
    suite proves the SDK's uniform handling of that shape rather
    than the real binary's behaviour on calls never probed against
    it. The exception messages must carry the binary's version beside
    the SDK's, so a stale binary explains itself instead of leaking a
    bare argument error.
    """

    def test_version_reports_the_stub_version(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            stub = write_stub(Path(tmp), STUB_SOURCE, "verirun")
            self.assertEqual(verirun.version(binary=stub), "0.1.0")

    def test_every_function_raises_verirun_error_at_exit_3(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            stub = write_stub(Path(tmp), STUB_SOURCE, "verirun")
            outcomes = call_each(stub)
        for name, outcome in outcomes.items():
            self.assertIsInstance(
                outcome, verirun.VerirunError, f"{name} raised {outcome!r}"
            )
            self.assertEqual(outcome.exit_code, 3, name)

    def test_pin_failure_names_both_versions(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            stub = write_stub(Path(tmp), STUB_SOURCE, "verirun")
            with self.assertRaises(verirun.VerirunError) as ctx:
                verirun.pin(DIGEST, cas=Path(tmp), binary=stub)
        message = str(ctx.exception)
        self.assertIn("unrecognized subcommand", message)
        self.assertIn("binary version 0.1.0", message)
        self.assertIn(f"SDK version {verirun.__version__}", message)

    def test_run_failure_names_both_versions(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            stub = write_stub(root, STUB_SOURCE, "verirun")
            spec_path = write_spec(root)
            seed = root / "seed.txt"
            seed.write_text("hello\n", encoding="utf-8")
            with self.assertRaises(verirun.VerirunError) as ctx:
                verirun.run(
                    spec_path,
                    cas=root / "cas",
                    work=root / "work",
                    inputs={"seed.txt": seed},
                    binary=stub,
                )
        message = str(ctx.exception)
        self.assertIn("binary version 0.1.0", message)
        self.assertIn(f"SDK version {verirun.__version__}", message)

    def test_non_utf8_version_output_leaves_the_message_unchanged(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            stub = write_stub(Path(tmp), NONUTF8_SOURCE, "verirun-nonutf8")
            with self.assertRaises(verirun.VerirunError) as ctx:
                verirun.pin(DIGEST, cas=Path(tmp), binary=stub)
        message = str(ctx.exception)
        self.assertIn("unexpected argument", message)
        self.assertNotIn("binary version", message)


if __name__ == "__main__":
    unittest.main()
