#!/usr/bin/env python3
"""End-to-end reproducible evaluation DAG using the typed ``verirun`` SDK.

Demonstrates:
1. Upfront validation and cold execution of a 3-task evaluation pipeline
   (``prepare_dataset`` -> ``score_candidates`` -> ``summarize_report``)
   with ``pin_manifest=True``.
2. Worker-count independence (``jobs=4`` cold run produces the exact same
   manifest digest) and warm ``by-exec`` cache reuse (``jobs=4`` warm run
   serves all three tasks from cache with identical artifact digests and
   deterministic warm manifest digests).
3. Modifying a downstream evaluation threshold and inspecting task-level
   effects with ``verirun.diff(...)``.
4. Convicting a 1-byte disk corruption on a separate unpinned store copy,
   raising ``verirun.VerirunError`` with ``exit_code == 2``.
"""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import shutil
import stat
import sys
import tempfile

REPO_ROOT = Path(__file__).resolve().parents[1]
if importlib.util.find_spec("verirun") is None:
    sys.path.insert(0, str(REPO_ROOT / "python"))

import verirun  # noqa: E402

RAW_CANDIDATES_TSV = (
    "id\tcategory\tlatency_ms\taccuracy_pct\n"
    "cand_c\tcanonical_json\t14\t96\n"
    "cand_a\tmerkle_verify\t11\t99\n"
    "cand_d\tsandbox_escape\t29\t72\n"
    "cand_b\tdag_scheduler\t18\t88\n"
)


def build_eval_pipeline(pass_threshold: int) -> verirun.PipelineBuilder:
    """Construct the 3-stage evaluation DAG parameterized by pass threshold."""
    prepare_cmd = (
        "mkdir -p outputs/dataset && "
        "awk 'NR > 1 { print $1 \"\\t\" $2 \"\\t\" $3 \"\\t\" $4 }' "
        "inputs/raw_prompts.tsv | sort > outputs/dataset/normalized.tsv"
    )
    score_cmd = (
        "mkdir -p outputs/scores && "
        "awk -F'\\t' -v min=\"$PASS_THRESHOLD\" "
        "'{ verdict = ($4 + 0 >= min + 0) ? \"PASS\" : \"FAIL\"; "
        "print $1 \"\\t\" $2 \"\\t\" $4 \"\\t\" verdict }' "
        "inputs/dataset/normalized.tsv > outputs/scores/verdicts.tsv"
    )
    summarize_cmd = (
        "mkdir -p outputs/report && "
        "awk -F'\\t' "
        "'BEGIN { total=0; passed=0; sum=0 } "
        "{ total++; sum+=$3; if ($4 == \"PASS\") passed++ } "
        "END { printf \"{\\\"candidates\\\":%d,\\\"mean_accuracy\\\":%.2f,"
        "\\\"passed\\\":%d}\\n\", total, sum/total, passed }' "
        "inputs/scores/verdicts.tsv > outputs/report/summary.json"
    )

    return (
        verirun.PipelineBuilder()
        .add_task(
            task_id="prepare_dataset",
            executable="sh",
            args=["-c", prepare_cmd],
            env={"DATASET_SPLIT": "eval_v1"},
            inputs=["raw_prompts.tsv"],
            outputs=["dataset/normalized.tsv"],
            timeout_ms=15_000,
        )
        .add_task(
            task_id="score_candidates",
            executable="sh",
            args=["-c", score_cmd],
            env={"PASS_THRESHOLD": str(pass_threshold)},
            inputs=["dataset/normalized.tsv"],
            outputs=["scores/verdicts.tsv"],
            depends_on=["prepare_dataset"],
            timeout_ms=15_000,
        )
        .add_task(
            task_id="summarize_report",
            executable="sh",
            args=["-c", summarize_cmd],
            inputs=["scores/verdicts.tsv"],
            outputs=["report/summary.json"],
            depends_on=["score_candidates"],
            timeout_ms=15_000,
        )
    )


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="verirun-eval-example-") as tmp:
        root = Path(tmp)
        raw_input = root / "raw_prompts.tsv"
        raw_input.write_text(RAW_CANDIDATES_TSV, encoding="utf-8")

        baseline = build_eval_pipeline(pass_threshold=80)
        topo_order = baseline.validate()
        assert topo_order == (
            "prepare_dataset",
            "score_candidates",
            "summarize_report",
        ), topo_order

        # 1. Cold run (jobs=1) with manifest pinning.
        cas = root / "cas"
        work = root / "work"
        cold = baseline.run(
            cas=cas,
            work=work,
            inputs={"raw_prompts.tsv": raw_input},
            jobs=1,
            pin_manifest=True,
            check=True,
        )
        assert cold.passed and cold.pinned
        assert all(t.status == "succeeded" for t in cold.tasks)
        verified_cold = verirun.verify(cold.manifest, cas=cas)
        assert verified_cold.verified
        print(f"[1/4] cold pinned manifest:      {cold.manifest}")
        print(f"      merkle root:               {verified_cold.merkle_root}")

        # 2. Determinism across worker counts + warm by-exec cache hits (jobs=4).
        cold_parallel = baseline.run(
            cas=root / "cas-parallel",
            work=root / "work-parallel",
            inputs={"raw_prompts.tsv": raw_input},
            jobs=4,
            check=True,
        )
        assert cold_parallel.manifest == cold.manifest, (
            "cold manifest digest must be identical across worker counts and paths"
        )

        warm_first = baseline.run(
            cas=cas,
            work=work,
            inputs={"raw_prompts.tsv": raw_input},
            jobs=4,
            check=True,
        )
        warm_second = baseline.run(
            cas=cas,
            work=work,
            inputs={"raw_prompts.tsv": raw_input},
            jobs=4,
            check=True,
        )
        assert all(t.status == "cached" for t in warm_first.tasks)
        assert warm_first.manifest == warm_second.manifest

        cold_shown = verirun.show(cold.manifest, cas=cas)
        warm_shown = verirun.show(warm_first.manifest, cas=cas)
        for cold_rec, warm_rec in zip(cold_shown.tasks, warm_shown.tasks):
            assert cold_rec.exec_digest == warm_rec.exec_digest
            assert cold_rec.artifacts == warm_rec.artifacts
        print(f"[2/4] parallel cold (jobs=4):    {cold_parallel.manifest} (matches cold)")
        print(f"      warm cached (jobs=4):      {warm_first.manifest} (all 3 tasks cached)")

        # 3. Modified pipeline (threshold 80 -> 90) and manifest diff.
        stricter = build_eval_pipeline(pass_threshold=90)
        modified = stricter.run(
            cas=cas,
            work=work,
            inputs={"raw_prompts.tsv": raw_input},
            jobs=4,
            check=True,
        )
        status_by_id = {t.task_id: t.status for t in modified.tasks}
        assert status_by_id == {
            "prepare_dataset": "cached",
            "score_candidates": "succeeded",
            "summarize_report": "succeeded",
        }, status_by_id

        diff_res = verirun.diff(cold.manifest, modified.manifest, cas=cas)
        assert not diff_res.equivalent
        assert diff_res.exit_code == 1
        diff_by_id = {d.task_id: d.fields for d in diff_res.differences}
        assert diff_by_id["prepare_dataset"] is not None
        assert not diff_by_id["prepare_dataset"].artifacts
        assert diff_by_id["score_candidates"] is not None
        assert diff_by_id["score_candidates"].artifacts
        assert diff_by_id["summarize_report"] is not None
        assert diff_by_id["summarize_report"].artifacts
        print(
            f"[3/4] threshold diff (80 vs 90): exit_code={diff_res.exit_code}, "
            "prepare_dataset cached, downstream artifacts updated"
        )

        # 4. 1-byte tamper conviction on a separate unpinned store copy.
        cas_tamper = root / "cas-tamper"
        shutil.copytree(cas, cas_tamper)
        unpinned = verirun.unpin(cold.manifest, cas=cas_tamper)
        assert not unpinned.pinned

        summary_rec = next(
            t for t in cold_shown.tasks if t.task_id == "summarize_report"
        )
        report_digest = summary_rec.artifacts[0].digest
        blob_path = cas_tamper / "objects" / report_digest[:2] / report_digest
        assert blob_path.is_file(), blob_path

        os.chmod(blob_path, stat.S_IRUSR | stat.S_IWUSR)
        corrupted = bytearray(blob_path.read_bytes())
        corrupted[0] ^= 0x01
        blob_path.write_bytes(bytes(corrupted))

        try:
            verirun.verify(cold.manifest, cas=cas_tamper)
        except verirun.VerirunError as err:
            assert err.exit_code == 2, err.exit_code
            assert err.kind == "blob_mismatch", err.kind
            print(
                f"[4/4] 1-byte tamper convicted:   exit_code={err.exit_code}, "
                f"kind={err.kind}"
            )
        else:
            raise AssertionError("expected VerirunError(exit_code=2) on tampered blob")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
