#!/usr/bin/env python3
"""Export stable EID-448 repair rows as Linear sync candidates."""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "docs" / "converge" / "eid-448"
REPAIR_QUEUE = BASE / "repair_queue.json"
ROWS = BASE / "converge_rows.jsonl"
OUT = BASE / "linear_sync_candidates.json"


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def load_jsonl(path: Path) -> dict[str, dict[str, Any]]:
    rows = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        row = json.loads(line)
        rows[row["target_id"]] = row
    return rows


def stable_candidate(item: dict[str, Any], row: dict[str, Any]) -> bool:
    if item["class"] == "pass":
        return False
    if item["gap_type"] in {"policy_gap", "blocked_dependency"}:
        return False
    if "Create or run the deterministic probe" in str(item["next_action"]):
        return False
    return row.get("matrix_id") in {"M1", "M2", "M3", "M4"}


def main() -> None:
    queue = load_json(REPAIR_QUEUE)
    rows = load_jsonl(ROWS)
    candidates = []
    for item in queue:
        row = rows[item["target_id"]]
        if not stable_candidate(item, row):
            continue
        candidates.append(
            {
                "target_id": item["target_id"],
                "title": f"Converge repair: {item['target_id']} {row['target']}",
                "linear_team": "Eidos AGI",
                "labels": ["Converge", "Agent Ready", "Proof Required"],
                "related_to": ["EID-448", "EID-449"],
                "priority": item["priority"],
                "gap_type": item["gap_type"],
                "description": "\n".join(
                    [
                        "## Converge Repair Row",
                        "",
                        f"- Target: `{item['target_id']}`",
                        f"- Matrix: `{row['matrix_id']}`",
                        f"- Class: `{item['class']}`",
                        f"- Gap: `{item['gap_type']}`",
                        f"- Evidence: {item['evidence']}",
                        f"- Next action: {item['next_action']}",
                        "",
                        "## Source",
                        "",
                        "- `docs/converge/eid-448/repair_queue.json`",
                        "- `docs/converge/eid-448/converge_rows.jsonl`",
                    ]
                ),
            }
        )

    payload = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "mode": "dry_run_candidates",
        "create_rule": "Only stable non-policy, non-blocked repair rows with specific next actions become candidates.",
        "candidate_count": len(candidates),
        "candidates": candidates[:10],
    }
    OUT.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(f"linear_sync_candidates={payload['candidate_count']}")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
