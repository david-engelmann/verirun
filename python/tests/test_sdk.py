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


class VerirunSdkTests(unittest.TestCase):
    def test_version_matches_package(self) -> None:
        self.assertEqual(verirun.version(), verirun.__version__)

    def test_end_to_end_run_verify_diff_gc_and_tamper_conviction(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cas = root / "cas"
            work = root / "work"
            seed_file = root / "seed.txt"
            seed_file.write_text("hello verirun\n", encoding="utf-8")

            if os.name == "nt":
                executable = "cmd"
                args = ["/C", "copy inputs\\seed.txt outputs\\loud.txt >NUL"]
            else:
                executable = "sh"
                args = ["-c", "tr a-z A-Z < inputs/seed.txt > outputs/loud.txt"]

            spec_path = root / "pipeline.json"
            spec = {
                "tasks": [
                    {
                        "task_id": "upper",
                        "executable": executable,
                        "args": args,
                        "env": {},
                        "inputs": ["seed.txt"],
                        "outputs": ["loud.txt"],
                        "depends_on": [],
                    }
                ]
            }
            spec_path.write_text(json.dumps(spec), encoding="utf-8")

            first = verirun.run(
                spec_path,
                cas=cas,
                work=work,
                inputs={"seed.txt": seed_file},
                pin_manifest=True,
                check=True,
            )
            self.assertTrue(first.passed)
            self.assertTrue(first.pinned)
            self.assertEqual(first.tasks[0].status, "succeeded")

            second = verirun.run(
                spec_path,
                cas=cas,
                work=work,
                inputs={"seed.txt": seed_file},
                jobs=4,
                check=True,
            )
            self.assertNotEqual(first.manifest, second.manifest)
            self.assertEqual(second.tasks[0].status, "cached")

            cold_again = verirun.run(
                spec_path,
                cas=root / "cas-fresh",
                work=root / "work-fresh",
                inputs={"seed.txt": seed_file},
                jobs=4,
                check=True,
            )
            self.assertEqual(first.manifest, cold_again.manifest)

            verified = verirun.verify(first.manifest, cas=cas)
            self.assertTrue(verified.verified)

            shown = verirun.show(first.manifest, cas=cas)
            self.assertEqual(shown.merkle_root, verified.merkle_root)
            artifact_digest = shown.tasks[0].artifacts[0].digest

            diff_res = verirun.diff(first.manifest, second.manifest, cas=cas)
            self.assertFalse(diff_res.equivalent)
            self.assertEqual(diff_res.exit_code, 1)
            self.assertTrue(diff_res.differences[0].fields.status)
            self.assertFalse(diff_res.differences[0].fields.artifacts)

            same_diff = verirun.diff(first.manifest, first.manifest, cas=cas)
            self.assertTrue(same_diff.equivalent)
            self.assertEqual(same_diff.exit_code, 0)

            gc_plan = verirun.gc(cas=cas, grace_secs=0, dry_run=True)
            self.assertTrue(gc_plan.dry_run)
            self.assertGreaterEqual(gc_plan.reachable, 1)

            unpinned = verirun.unpin(first.manifest, cas=cas)
            self.assertFalse(unpinned.pinned)

            blob_path = next(cas.rglob(f"*{artifact_digest[2:]}"))
            os.chmod(blob_path, stat.S_IRUSR | stat.S_IWUSR)
            raw = bytearray(blob_path.read_bytes())
            raw[0] ^= 0x01
            blob_path.write_bytes(bytes(raw))

            with self.assertRaises(verirun.VerirunError) as ctx:
                verirun.verify(first.manifest, cas=cas)
            self.assertEqual(ctx.exception.exit_code, 2)
            self.assertEqual(ctx.exception.kind, "blob_mismatch")


if __name__ == "__main__":
    unittest.main()
