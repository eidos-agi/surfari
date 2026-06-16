#!/usr/bin/env python3
"""Regenerate the EID-448 Converge seed and write a score report."""

from __future__ import annotations

import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "docs" / "converge" / "eid-448"
REPORT = BASE / "score_report.md"


def run_step(script: str) -> None:
    subprocess.run([sys.executable, str(ROOT / "scripts" / script)], cwd=ROOT, check=True)


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [
        json.loads(line)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def format_counts(counts: dict[str, int]) -> str:
    return (
        f"{counts.get('pass', 0)} pass, "
        f"{counts.get('fail', 0)} fail, "
        f"{counts.get('blocked', 0)} blocked, "
        f"{counts.get('skip', 0)} skip"
    )


def write_report() -> None:
    matrix = load_json(ROOT / "docs" / "plans" / "eid-448-test-coverage-matrix.json")
    summary = load_json(BASE / "stage_summary.json")
    queue = load_json(BASE / "repair_queue.json")
    loops = load_jsonl(BASE / "research_loops.jsonl")
    rows = load_jsonl(BASE / "converge_rows.jsonl")

    counts = {
        status: sum(1 for row in rows if row["class"] == status)
        for status in ("pass", "fail", "blocked", "skip")
    }
    covered_cells = sum(len(test.get("coverage", {})) for test in matrix["tests"])
    weighted_depth = sum(
        sum(int(depth) for depth in test.get("coverage", {}).values())
        for test in matrix["tests"]
    )
    top_repairs = queue[:10]
    generated_at = datetime.now(timezone.utc).isoformat(timespec="seconds")

    lines = [
        "# EID-448 Converge Score Report",
        "",
        f"Generated at: `{generated_at}`",
        "",
        "## Current Score",
        "",
        f"- Matrix shape: `{matrix['matrix_shape']}`",
        f"- Planned coverage cells: `{covered_cells}`",
        f"- Planned weighted depth: `{weighted_depth}`",
        f"- Evidence rows: `{len(rows)}`",
        f"- Row state: `{format_counts(counts)}`",
        f"- Repair queue: `{len(queue)}`",
        f"- Research loops: `{len(loops)}`",
        "",
        "## Stage Gates",
        "",
        "| Stage | Proof class | Approval | Rows | State | Next blocking rows |",
        "|---|---|---:|---:|---|---|",
    ]
    for stage in summary["stages"]:
        approval = "yes" if stage["human_approval_required"] else "no"
        blockers = ", ".join(stage["next_blocking_rows"]) or "none"
        lines.append(
            "| {matrix_id} {name} | `{proof_class}` | {approval} | {rows} | `{state}` | {blockers} |".format(
                matrix_id=stage["matrix_id"],
                name=stage["name"],
                proof_class=stage["proof_class"],
                approval=approval,
                rows=stage["rows"],
                state=stage["unlock_state"],
                blockers=blockers,
            )
        )

    lines.extend(
        [
            "",
            "## Highest Priority Repairs",
            "",
            "| Target | Class | Gap | Priority | Owner | Next action |",
            "|---|---|---|---:|---|---|",
        ]
    )
    for item in top_repairs:
        lines.append(
            "| {target_id} | `{class_}` | `{gap_type}` | {priority} | {owner} | {next_action} |".format(
                target_id=item["target_id"],
                class_=item["class"],
                gap_type=item["gap_type"],
                priority=item["priority"],
                owner=str(item["owner"]).replace("|", "/"),
                next_action=str(item["next_action"]).replace("|", "/"),
            )
        )

    lines.extend(
        [
            "",
            "## Generated Artifacts",
            "",
            "- `docs/plans/eid-448-test-coverage-matrix.json`",
            "- `docs/plans/eid-448-test-coverage-heatmap.svg`",
            "- `docs/converge/eid-448/converge_rows.jsonl`",
            "- `docs/converge/eid-448/repair_queue.json`",
            "- `docs/converge/eid-448/research_loops.jsonl`",
            "- `docs/converge/eid-448/stage_summary.json`",
            "- `docs/converge/eid-448/evidence_heatmap.svg`",
            "- `docs/converge/eid-448/evidence/fake_workflow_probe.json`",
            "- `docs/converge/eid-448/evidence/browser_fixture_probe.json`",
            "- `docs/converge/eid-448/evidence/wrapper_fixture_probe.json`",
            "- `docs/converge/eid-448/linear_sync_candidates.json`",
            "",
            "## Linear",
            "",
            "- Seed issue: `EID-448`",
            "- Control issue: `EID-449`",
            "- Temporary query label: `Converge`",
        ]
    )

    REPORT.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> None:
    run_step("build-eid448-matrix.py")
    run_step("render-eid448-heatmap.py")
    run_step("probe-eid448-fake-workflows.py")
    run_step("probe-eid448-browser-fixtures.py")
    run_step("probe-eid448-wrapper-fixture.py")
    if (BASE / "evidence" / "apple_dry_run_probe.json").exists():
        print("using existing apple_dry_run_probe.json; run scripts/probe-eid448-apple-dry-run.py to refresh")
    run_step("eid448_converge.py")
    run_step("eid448-linear-sync-candidates.py")
    write_report()
    print(f"wrote {REPORT}")


if __name__ == "__main__":
    main()
