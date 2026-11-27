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
print("error: unexpected argument '--input' found", file=sys.stderr)
raise SystemExit(3)
'''


def write_stub(directory: Path) -> Path:
    """Create an executable stub mimicking the 0.1.0 CLI's surface.

    The stub answers ``--version`` as 0.1.0, rejects ``pin`` as an
    unrecognized subcommand, and rejects everything else the way the
    0.1.0 argument parser rejected flags it did not know, always
    without a JSON payload.
    """
    script = directory / "verirun_stub.py"
    script.write_text(STUB_SOURCE, encoding="utf-8")
    if os.name == "nt":
        wrapper = directory / "verirun.bat"
        wrapper.write_text(
            f'@echo off\n"{sys.executable}" "{script}" %*\n', encoding="utf-8"
        )
    else:
        wrapper = directory / "verirun"
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
    ``version`` is exercised separately, since a binary that answers
    ``--version`` at all does not fail that call.
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
    """A binary that runs but fails without JSON yields VerirunError.

    The Python interpreter itself stands in as the failing binary: it
    runs, rejects the CLI's arguments on stderr, exits nonzero, and
    never emits a payload. Its own ``--version`` output is not in the
    CLI's form, so no version diagnostic may be attached.
    """

    def test_every_function_raises_verirun_error(self) -> None:
        outcomes = call_each(Path(sys.executable))
        for name, outcome in outcomes.items():
            self.assertIsInstance(
                outcome, verirun.VerirunError, f"{name} raised {outcome!r}"
            )
            assert isinstance(outcome, verirun.VerirunError)
            self.assertNotEqual(outcome.exit_code, 0, name)
            self.assertIsNone(outcome.kind, name)
            self.assertNotIn("binary version", str(outcome), name)


class WrongVersionBinaryTests(unittest.TestCase):
    """A 0.1.0-shaped binary fails loudly and names both versions.

    The payload-less failures the 0.1.0 CLI produces must surface as
    VerirunError at exit code 3, and the message must carry the
    binary's version beside the SDK's, so a stale binary explains
    itself instead of leaking a bare argument error.
    """

    def test_version_reports_the_stub_version(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            stub = write_stub(Path(tmp))
            self.assertEqual(verirun.version(binary=stub), "0.1.0")

    def test_every_function_raises_verirun_error_at_exit_3(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            stub = write_stub(Path(tmp))
            outcomes = call_each(stub)
        for name, outcome in outcomes.items():
            self.assertIsInstance(
                outcome, verirun.VerirunError, f"{name} raised {outcome!r}"
            )
            assert isinstance(outcome, verirun.VerirunError)
            self.assertEqual(outcome.exit_code, 3, name)

    def test_pin_failure_names_both_versions(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            stub = write_stub(Path(tmp))
            with self.assertRaises(verirun.VerirunError) as ctx:
                verirun.pin(DIGEST, cas=Path(tmp), binary=stub)
        message = str(ctx.exception)
        self.assertIn("unrecognized subcommand", message)
        self.assertIn("binary version 0.1.0", message)
        self.assertIn(f"SDK version {verirun.__version__}", message)

    def test_run_failure_names_both_versions(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            stub = write_stub(root)
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


if __name__ == "__main__":
    unittest.main()
