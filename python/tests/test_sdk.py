from __future__ import annotations

from dataclasses import FrozenInstanceError, is_dataclass
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


def _shell_copy_cmd(src_rel: str, dst_rel: str) -> tuple[str, list[str]]:
    if os.name == "nt":
        src_win = src_rel.replace("/", "\\")
        dst_win = dst_rel.replace("/", "\\")
        return "cmd", ["/C", f"copy inputs\\{src_win} outputs\\{dst_win} >NUL"]
    return "sh", ["-c", f"cp inputs/{src_rel} outputs/{dst_rel}"]


class VerirunSdkTests(unittest.TestCase):
    def test_version_matches_package(self) -> None:
        self.assertEqual(verirun.__version__, "0.2.2")
        self.assertEqual(verirun.version(), verirun.__version__)

    def test_completions_all_shells_and_invalid_shell(self) -> None:
        default_script = verirun.completions()
        self.assertEqual(default_script, verirun.completions("bash"))
        for shell in ("bash", "zsh", "fish", "powershell"):
            script = verirun.completions(shell)
            self.assertTrue(script.endswith("\n"))
            for subcmd in (
                "run",
                "verify",
                "show",
                "diff",
                "gc",
                "pin",
                "unpin",
                "completions",
            ):
                self.assertIn(subcmd, script)
            for flag in (
                "--cas",
                "--work",
                "--jobs",
                "--input",
                "--pin",
                "--dry-run",
                "--json",
                "--seed",
                "--help",
                "--version",
            ):
                self.assertIn(flag, script)

        for invalid_shell in ("tcsh", "invalid-shell", ""):
            with self.assertRaises(verirun.VerirunError) as ctx:
                verirun.completions(invalid_shell)
            self.assertEqual(ctx.exception.exit_code, 3)

        with self.assertRaises(verirun.VerirunError) as type_ctx:
            verirun.completions(123)  # type: ignore[arg-type]
        self.assertEqual(type_ctx.exception.exit_code, 3)

    def test_task_spec_normalization_and_immutability(self) -> None:
        self.assertTrue(is_dataclass(verirun.TaskSpec))
        raw_env = {"Z_KEY": "last", "A_KEY": "first"}
        spec = verirun.TaskSpec(
            task_id="compile",
            executable="sh",
            args=["-c", "echo ok"],
            env=raw_env,
            inputs=["src/in.txt"],
            outputs=["dist/out.txt"],
            depends_on=["prep"],
            timeout_ms=5000,
        )
        raw_env["MUTATED"] = "ignored"
        self.assertEqual(spec.args, ("-c", "echo ok"))
        self.assertEqual(spec.inputs, ("src/in.txt",))
        self.assertEqual(spec.outputs, ("dist/out.txt",))
        self.assertEqual(spec.depends_on, ("prep",))
        self.assertEqual(list(spec.env.items()), [("A_KEY", "first"), ("Z_KEY", "last")])
        self.assertEqual(
            spec.to_dict(),
            {
                "task_id": "compile",
                "executable": "sh",
                "args": ["-c", "echo ok"],
                "env": {"A_KEY": "first", "Z_KEY": "last"},
                "inputs": ["src/in.txt"],
                "outputs": ["dist/out.txt"],
                "depends_on": ["prep"],
                "timeout_ms": 5000,
            },
        )

        no_timeout = verirun.TaskSpec(task_id="noop", executable="sh")
        self.assertNotIn("timeout_ms", no_timeout.to_dict())

        with self.assertRaises(FrozenInstanceError):
            spec.task_id = "other"  # type: ignore[misc]
        with self.assertRaises(TypeError):
            spec.env["NEW"] = "blocked"  # type: ignore[index]

        for bad_kwargs in (
            {"task_id": "t", "executable": "sh", "args": "-c echo"},
            {"task_id": "t", "executable": "sh", "timeout_ms": True},
        ):
            with self.assertRaises(verirun.VerirunError) as ctx:
                verirun.TaskSpec(**bad_kwargs)  # type: ignore[arg-type]
            self.assertEqual(ctx.exception.exit_code, 3)
            self.assertEqual(ctx.exception.kind, "invalid_spec")

    def test_pipeline_builder_topological_order_and_serialization(self) -> None:
        builder = (
            verirun.PipelineBuilder()
            .add_task(
                task_id="join",
                executable="sh",
                args=["-c", "true"],
                depends_on=["right", "left"],
            )
            .add_task(
                verirun.TaskSpec(
                    task_id="right",
                    executable="sh",
                    args=("-c", "true"),
                    depends_on=("root",),
                )
            )
            .add_task(
                task_id="left",
                executable="sh",
                args=["-c", "true"],
                depends_on=["root"],
            )
            .add_task(task_id="root", executable="sh", args=["-c", "true"])
        )
        self.assertEqual(builder.validate(), ("root", "left", "right", "join"))

        with tempfile.TemporaryDirectory() as tmp:
            written = builder.write(Path(tmp) / "nested" / "pipeline.json")
            parsed = json.loads(written.read_text(encoding="utf-8"))
            self.assertEqual(parsed, builder.to_dict())
            self.assertEqual(len(parsed["tasks"]), 4)

    def test_pipeline_builder_rejects_invalid_specs_upfront(self) -> None:
        def assert_invalid(builder: verirun.PipelineBuilder) -> verirun.VerirunError:
            with self.assertRaises(verirun.VerirunError) as ctx:
                builder.validate()
            self.assertEqual(ctx.exception.exit_code, 3)
            self.assertEqual(ctx.exception.kind, "invalid_spec")
            return ctx.exception

        # Empty pipeline
        assert_invalid(verirun.PipelineBuilder())

        # Blank or unsafe task_id and duplicate task_id
        for bad_id in ("", "   ", ".", "..", "a/b", "a\\b"):
            assert_invalid(
                verirun.PipelineBuilder().add_task(task_id=bad_id, executable="sh")
            )
        assert_invalid(
            verirun.PipelineBuilder()
            .add_task(task_id="dup", executable="sh")
            .add_task(task_id="dup", executable="sh")
        )

        # Blank executable and invalid timeout_ms
        for bad_exe in ("", "   "):
            assert_invalid(
                verirun.PipelineBuilder().add_task(task_id="t", executable=bad_exe)
            )
        for bad_timeout in (0, -10, 9_007_199_254_740_992):
            assert_invalid(
                verirun.PipelineBuilder().add_task(
                    task_id="t", executable="sh", timeout_ms=bad_timeout
                )
            )

        # Reserved env keys
        for reserved in ("PATH", "HOME", "TMPDIR", "TZ", "LC_ALL", "SOURCE_DATE_EPOCH"):
            err = assert_invalid(
                verirun.PipelineBuilder().add_task(
                    task_id="t", executable="sh", env={reserved: "override"}
                )
            )
            self.assertIn(reserved, str(err))

        # Unsafe relative paths in inputs and outputs. Dot
        # components and empty components are not here: they
        # normalize away, as they do at the CLI, and are pinned
        # as accepted spellings in
        # test_pipeline_builder_normalizes_path_spellings.
        unsafe_paths = (
            "",
            "/etc/passwd",
            "C:/temp/out.txt",
            "dir\\file.txt",
            ".",
            "..",
            "../escape.txt",
            "dir/../escape.txt",
        )
        for bad_path in unsafe_paths:
            assert_invalid(
                verirun.PipelineBuilder().add_task(
                    task_id="t", executable="sh", inputs=[bad_path]
                )
            )
            assert_invalid(
                verirun.PipelineBuilder().add_task(
                    task_id="t", executable="sh", outputs=[bad_path]
                )
            )

        # Duplicate inputs in one task and colliding outputs across tasks
        assert_invalid(
            verirun.PipelineBuilder().add_task(
                task_id="t", executable="sh", inputs=["same.txt", "same.txt"]
            )
        )
        assert_invalid(
            verirun.PipelineBuilder()
            .add_task(task_id="a", executable="sh", outputs=["shared/out.txt"])
            .add_task(task_id="b", executable="sh", outputs=["shared/out.txt"])
        )

        # Collisions the CLI refuses under INV-DAG-7, 8, and 9:
        # the builder refuses them upfront too.
        assert_invalid(
            verirun.PipelineBuilder()
            .add_task(task_id="a", executable="sh", outputs=["Out.txt"])
            .add_task(task_id="b", executable="sh", outputs=["out.txt"])
        )
        assert_invalid(
            verirun.PipelineBuilder()
            .add_task(task_id="a", executable="sh", outputs=["a.txt "])
            .add_task(task_id="b", executable="sh", outputs=["a.txt"])
        )
        assert_invalid(
            verirun.PipelineBuilder().add_task(
                task_id="t", executable="sh", inputs=["In.txt", "in.txt"]
            )
        )
        assert_invalid(
            verirun.PipelineBuilder().add_task(
                task_id="t", executable="sh", inputs=["a.txt ", "a.txt"]
            )
        )
        assert_invalid(
            verirun.PipelineBuilder()
            .add_task(task_id="Build", executable="sh")
            .add_task(task_id="build", executable="sh")
        )
        assert_invalid(
            verirun.PipelineBuilder()
            .add_task(task_id="build", executable="sh")
            .add_task(task_id="build ", executable="sh")
        )
        assert_invalid(
            verirun.PipelineBuilder().add_task(task_id="...", executable="sh")
        )

        # Reserved env keys under ASCII case folding, as the CLI
        # compares them: environment lookup on Windows is
        # case-insensitive.
        for folded in ("Path", "home", "tz"):
            err = assert_invalid(
                verirun.PipelineBuilder().add_task(
                    task_id="t", executable="sh", env={folded: "override"}
                )
            )
            self.assertIn(folded, str(err))

        # Unknown dependency, self-dependency, and multi-node cycles
        assert_invalid(
            verirun.PipelineBuilder().add_task(
                task_id="a", executable="sh", depends_on=["missing"]
            )
        )
        assert_invalid(
            verirun.PipelineBuilder().add_task(
                task_id="self", executable="sh", depends_on=["self"]
            )
        )
        cycle_err = assert_invalid(
            verirun.PipelineBuilder()
            .add_task(task_id="setup", executable="sh")
            .add_task(task_id="alpha", executable="sh", depends_on=["setup", "gamma"])
            .add_task(task_id="beta", executable="sh", depends_on=["alpha"])
            .add_task(task_id="gamma", executable="sh", depends_on=["beta"])
        )
        self.assertIn("alpha, beta, gamma", str(cycle_err))

    def test_pipeline_builder_normalizes_path_spellings(self) -> None:
        # Spellings the CLI normalizes (dot components, empty
        # components, a trailing separator) validate here too,
        # and two spellings of one path collide after
        # normalization, as they do at the CLI.
        builder = verirun.PipelineBuilder().add_task(
            task_id="t",
            executable="sh",
            inputs=["./seed.txt", "dir//data.txt"],
            outputs=["out/"],
        )
        self.assertEqual(builder.validate(), ("t",))

        def assert_invalid(builder: verirun.PipelineBuilder) -> None:
            with self.assertRaises(verirun.VerirunError) as ctx:
                builder.validate()
            self.assertEqual(ctx.exception.kind, "invalid_spec")

        assert_invalid(
            verirun.PipelineBuilder()
            .add_task(task_id="a", executable="sh", outputs=["a/./b.txt"])
            .add_task(task_id="b", executable="sh", outputs=["a/b.txt"])
        )
        assert_invalid(
            verirun.PipelineBuilder().add_task(
                task_id="t", executable="sh", inputs=["x.txt", "./x.txt"]
            )
        )

    def test_pipeline_builder_run_and_run_tasks_end_to_end(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            seed_file = root / "seed.txt"
            seed_file.write_text("alpha beta\n", encoding="utf-8")

            exe1, args1 = _shell_copy_cmd("seed.txt", "stage1.txt")
            exe2, args2 = _shell_copy_cmd("stage1.txt", "final.txt")

            builder = (
                verirun.PipelineBuilder()
                .add_task(
                    task_id="stage_one",
                    executable=exe1,
                    args=args1,
                    env={"PIPELINE_STAGE": "1"},
                    inputs=["seed.txt"],
                    outputs=["stage1.txt"],
                )
                .add_task(
                    task_id="stage_two",
                    executable=exe2,
                    args=args2,
                    env={"PIPELINE_STAGE": "2"},
                    inputs=["stage1.txt"],
                    outputs=["final.txt"],
                    depends_on=["stage_one"],
                )
            )
            self.assertEqual(builder.validate(), ("stage_one", "stage_two"))

            cas_builder = root / "cas-builder"
            work_builder = root / "work-builder"
            built_run = builder.run(
                cas=cas_builder,
                work=work_builder,
                inputs={"seed.txt": seed_file},
                pin_manifest=True,
                check=True,
            )
            self.assertTrue(built_run.passed)
            self.assertTrue(built_run.pinned)
            self.assertEqual(
                tuple(t.task_id for t in built_run.tasks),
                ("stage_one", "stage_two"),
            )
            verified = verirun.verify(built_run.manifest, cas=cas_builder)
            self.assertTrue(verified.verified)

            cas_helper = root / "cas-helper"
            work_helper = root / "work-helper"
            helper_run = verirun.run_tasks(
                [
                    builder.tasks[0],
                    builder.tasks[1].to_dict(),
                ],
                cas=cas_helper,
                work=work_helper,
                inputs={"seed.txt": seed_file},
                jobs=2,
                check=True,
            )
            self.assertEqual(built_run.manifest, helper_run.manifest)

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


    def test_check_mode_raises_with_failures_in_message(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            spec_path = root / "pipeline.json"
            spec = {
                "tasks": [
                    {
                        "task_id": "boom",
                        "executable": "verirun-no-such-executable",
                        "args": [],
                        "env": {},
                        "inputs": [],
                        "outputs": [],
                        "depends_on": [],
                    }
                ]
            }
            spec_path.write_text(json.dumps(spec), encoding="utf-8")

            with self.assertRaises(verirun.VerirunError) as ctx:
                verirun.run(
                    spec_path, cas=root / "cas", work=root / "work", check=True
                )
            self.assertEqual(ctx.exception.exit_code, 1)
            self.assertIn("boom", str(ctx.exception))
            self.assertEqual(ctx.exception.payload, {})

    def test_check_mode_exit_code_failure_uses_the_generic_message(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            if os.name == "nt":
                executable = "cmd"
                args = ["/C", "exit 3"]
            else:
                executable = "sh"
                args = ["-c", "exit 3"]
            spec_path = root / "pipeline.json"
            spec = {
                "tasks": [
                    {
                        "task_id": "boom",
                        "executable": executable,
                        "args": args,
                        "env": {},
                        "inputs": [],
                        "outputs": [],
                        "depends_on": [],
                    }
                ]
            }
            spec_path.write_text(json.dumps(spec), encoding="utf-8")

            with self.assertRaises(verirun.VerirunError) as ctx:
                verirun.run(
                    spec_path, cas=root / "cas", work=root / "work", check=True
                )
            self.assertEqual(ctx.exception.exit_code, 1)
            self.assertEqual(str(ctx.exception), "verirun run failed (task failure)")
            self.assertEqual(ctx.exception.payload, {})


if __name__ == "__main__":
    unittest.main()
