#!/usr/bin/env python3
"""Evaluate the EID-448 matrix chain into Converge/Praxis artifacts."""

from __future__ import annotations

import argparse
import html
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "docs" / "converge" / "eid-448"
MATRIX = ROOT / "docs" / "plans" / "eid-448-test-coverage-matrix.json"
CHAIN = BASE / "matrix_chain.json"
PROBES = BASE / "evidence" / "probes.json"
ROWS_OUT = BASE / "converge_rows.jsonl"
REPAIR_OUT = BASE / "repair_queue.json"
RESEARCH_OUT = BASE / "research_loops.jsonl"
SUMMARY_OUT = BASE / "stage_summary.json"
HEATMAP_OUT = BASE / "evidence_heatmap.svg"


STATUS_COLORS = {
    "pass": "#42d6a4",
    "fail": "#fb7185",
    "blocked": "#f5b84b",
    "skip": "#64748b",
}

STATUS_ORDER = {"fail": 0, "blocked": 1, "pass": 2, "skip": 3}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--matrix", type=Path, default=MATRIX)
    parser.add_argument("--chain", type=Path, default=CHAIN)
    parser.add_argument("--probes", type=Path, default=PROBES)
    parser.add_argument("--base", type=Path, default=BASE)
    return parser.parse_args()


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows),
        encoding="utf-8",
    )


def esc(value: object) -> str:
    return html.escape(str(value), quote=True)


def stage_for_test(chain: dict[str, Any], test_id: int) -> dict[str, Any]:
    for stage in chain["stages"]:
        start, end = stage["test_id_range"]
        if start <= test_id <= end:
            return stage
    raise ValueError(f"test {test_id} is not covered by any matrix stage")


def default_probe(probes: dict[str, Any]) -> dict[str, Any]:
    return {
        "class": probes.get("default_status", "fail"),
        "probe": probes.get("default_probe", "No evidence captured yet."),
        "evidence": "No proof envelope has been captured for this planned row.",
        "next_action": "Create or run the deterministic probe for this row.",
        "proof_envelope": {
            "environment": "unproven",
            "surface": "none",
            "proof_id": None,
            "external_dependencies": [],
            "bypassed_controls": [],
            "side_effects": [],
            "fails_to_test": ["all claimed behavior for this row"],
        },
    }


def row_from_test(
    test: dict[str, Any],
    matrix: dict[str, Any],
    chain: dict[str, Any],
    probes: dict[str, Any],
) -> dict[str, Any]:
    test_id = int(test["id"])
    stage = stage_for_test(chain, test_id)
    probe = {**default_probe(probes), **probes.get("probes", {}).get(str(test_id), {})}
    status = probe["class"]
    coverage = test.get("coverage", {})
    target = (
        f"{test['name']} proves {len(coverage)} planned capabilities at "
        f"weighted depth {sum(int(v) for v in coverage.values())}."
    )
    delta = delta_for_status(status)
    return {
        "target_id": f"EID448-T{test_id:03d}",
        "matrix_id": stage["matrix_id"],
        "matrix_name": stage["name"],
        "target": target,
        "planned_capabilities": coverage,
        "probe": probe.get("probe"),
        "delta": probe.get("delta", delta),
        "class": status,
        "evidence": probe.get("evidence"),
        "next_action": probe.get("next_action"),
        "owner": probe.get("owner", owner_for_status(status)),
        "proof_class": stage["proof_class"],
        "proof_envelope": probe.get("proof_envelope"),
        "unlock_next_when": stage["unlock_next_when"],
        "stop_conditions": stage["stop_conditions"],
    }


def delta_for_status(status: str) -> str:
    if status == "pass":
        return "target is currently supported by captured evidence"
    if status == "blocked":
        return "target cannot be proven until a named dependency or approval resolves"
    if status == "skip":
        return "target is intentionally out of scope"
    return "target is planned but not proven"


def owner_for_status(status: str) -> str:
    if status == "pass":
        return "none"
    if status == "blocked":
        return "named gate owner required"
    return "next coding agent"


def classify_gap(row: dict[str, Any]) -> str:
    text = " ".join(
        str(row.get(key, ""))
        for key in ("target", "probe", "evidence", "next_action")
    ).lower()
    if row["class"] == "pass":
        return "none"
    if any(term in text for term in ["compile/link", "stalled", "stale", "drift", "unknown"]):
        return "research_loop"
    if row["matrix_id"] == "M5" and any(
        term in text for term in ["human", "approval", "mfa", "passkey", "legal", "payment"]
    ):
        return "policy_gap"
    if "approval-gated" in text or "human approval" in text:
        return "policy_gap"
    if row["class"] == "blocked":
        return "blocked_dependency"
    return "repair_item"


def build_repair_queue(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    items = []
    for row in rows:
        if row["class"] == "pass" or row["class"] == "skip":
            continue
        gap_type = classify_gap(row)
        priority = priority_for_row(row, gap_type)
        items.append(
            {
                "target_id": row["target_id"],
                "matrix_id": row["matrix_id"],
                "class": row["class"],
                "gap_type": gap_type,
                "priority": priority,
                "owner": row["owner"],
                "next_action": row["next_action"],
                "evidence": row["evidence"],
            }
        )
    return sorted(items, key=lambda item: (item["priority"], item["target_id"]))


def priority_for_row(row: dict[str, Any], gap_type: str) -> int:
    stage_num = int(row["matrix_id"].removeprefix("M"))
    if gap_type == "policy_gap":
        return 10 + stage_num
    if gap_type == "research_loop":
        return 20 + stage_num
    if row["class"] == "blocked":
        return 30 + stage_num
    return 40 + stage_num


def build_research_loops(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    loops = []
    loop_index = 1
    for row in rows:
        gap_type = classify_gap(row)
        if gap_type not in {"research_loop", "policy_gap"}:
            continue
        loops.append(
            {
                "loop_id": f"RL-{loop_index:03d}",
                "spawned_by": row["target_id"],
                "gap_type": gap_type,
                "question": question_for_row(row, gap_type),
                "method": method_for_gap(row, gap_type),
                "evidence": [],
                "finding": None,
                "matrix_changes": [],
                "next_action": row["next_action"],
            }
        )
        loop_index += 1
    return loops


def question_for_row(row: dict[str, Any], gap_type: str) -> str:
    if gap_type == "policy_gap":
        return f"What front-loaded human approval or stop rule is needed for {row['target_id']}?"
    return f"What must be learned to make {row['target_id']} independently provable?"


def method_for_gap(row: dict[str, Any], gap_type: str) -> str:
    if gap_type == "policy_gap":
        return "Compare the target against the approval packet, forbidden actions, stop conditions, and human authority gates."
    return "Collect deterministic probe output, inspect failure mode, and update the matrix, fixture, or code with the smallest falsifiable repair."


def build_stage_summary(chain: dict[str, Any], rows: list[dict[str, Any]]) -> dict[str, Any]:
    out = {
        "schema_version": 1,
        "north_star": chain["north_star"],
        "stages": [],
    }
    for stage in chain["stages"]:
        stage_rows = [row for row in rows if row["matrix_id"] == stage["matrix_id"]]
        counts = {status: sum(1 for row in stage_rows if row["class"] == status) for status in STATUS_COLORS}
        required_total = len(stage_rows)
        unlock_state = "pass" if counts["fail"] == 0 and counts["blocked"] == 0 else "blocked" if counts["fail"] == 0 else "fail"
        out["stages"].append(
            {
                "matrix_id": stage["matrix_id"],
                "name": stage["name"],
                "proof_class": stage["proof_class"],
                "human_approval_required": stage["human_approval_required"],
                "rows": required_total,
                "counts": counts,
                "unlock_state": unlock_state,
                "next_blocking_rows": [
                    row["target_id"]
                    for row in sorted(stage_rows, key=lambda row: STATUS_ORDER[row["class"]])
                    if row["class"] in {"fail", "blocked"}
                ][:5],
            }
        )
    return out


def render_evidence_heatmap(matrix: dict[str, Any], rows: list[dict[str, Any]], out: Path) -> None:
    capabilities = matrix["capabilities"]
    row_map = {row["target_id"]: row for row in rows}
    tests = matrix["tests"]

    width = 2560
    height = 1700
    x0 = 520
    y0 = 250
    cell = 20
    gap = 3
    svg = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img">',
        '<rect width="2560" height="1700" fill="#0b1020"/>',
        '<rect x="40" y="40" width="2480" height="1620" rx="18" fill="#111827"/>',
        '<text x="72" y="96" font-family="Inter, Arial, sans-serif" font-size="38" font-weight="700" fill="#ecfdf5">EID-448 Evidence Heatmap</text>',
        '<text x="72" y="132" font-family="Inter, Arial, sans-serif" font-size="18" fill="#9fb0c7">Same 50x50 matrix, colored by current Converge row state rather than planned depth.</text>',
    ]
    legend = [("pass", STATUS_COLORS["pass"]), ("fail", STATUS_COLORS["fail"]), ("blocked", STATUS_COLORS["blocked"]), ("skip", STATUS_COLORS["skip"])]
    for idx, (label, color) in enumerate(legend):
        lx = 1450 + idx * 150
        svg.append(f'<rect x="{lx}" y="82" width="22" height="22" rx="5" fill="{color}"/>')
        svg.append(f'<text x="{lx + 30}" y="98" font-family="Inter, Arial, sans-serif" font-size="13" fill="#9fb0c7">{esc(label)}</text>')

    for col, cap in enumerate(capabilities):
        cx = x0 + col * (cell + gap)
        svg.append(f'<text x="{cx + cell / 2}" y="{y0 - 18}" text-anchor="middle" font-family="Inter, Arial, sans-serif" font-size="10" font-weight="700" fill="#ecfdf5">{esc(cap["code"])}</text>')

    for idx, test in enumerate(tests):
        target_id = f"EID448-T{int(test['id']):03d}"
        row = row_map[target_id]
        color = STATUS_COLORS[row["class"]]
        ty = y0 + idx * (cell + gap)
        svg.append(f'<text x="92" y="{ty + 16}" font-family="Inter, Arial, sans-serif" font-size="11" fill="#9fb0c7">T{int(test["id"]):02d}</text>')
        svg.append(f'<text x="138" y="{ty + 16}" font-family="Inter, Arial, sans-serif" font-size="12" fill="#ecfdf5">{esc(test["name"])}</text>')
        for col, cap in enumerate(capabilities):
            cx = x0 + col * (cell + gap)
            covered = cap["code"] in test.get("coverage", {})
            fill = color if covered else "#172033"
            opacity = "1" if covered else "0.55"
            svg.append(
                f'<rect x="{cx}" y="{ty}" width="{cell}" height="{cell}" rx="6" fill="{fill}" opacity="{opacity}" stroke="#263247" stroke-width="1">'
                f'<title>{esc(target_id)} {esc(test["name"])} / {esc(cap["code"])} {esc(cap["label"])}: {esc(row["class"])}</title></rect>'
            )
        svg.append(f'<text x="{x0 + len(capabilities) * (cell + gap) + 20}" y="{ty + 16}" font-family="Inter, Arial, sans-serif" font-size="12" fill="#9fb0c7">{esc(row["class"])}</text>')

    passed = sum(1 for row in rows if row["class"] == "pass")
    failed = sum(1 for row in rows if row["class"] == "fail")
    blocked = sum(1 for row in rows if row["class"] == "blocked")
    svg.append(f'<text x="{x0}" y="1460" font-family="Inter, Arial, sans-serif" font-size="14" fill="#9fb0c7">Rows: {passed} pass, {failed} fail, {blocked} blocked. Source: docs/converge/eid-448/converge_rows.jsonl.</text>')
    svg.append("</svg>\n")
    out.write_text("\n".join(svg), encoding="utf-8")


def main() -> None:
    args = parse_args()
    matrix = load_json(args.matrix)
    chain = load_json(args.chain)
    probes = load_json(args.probes)
    rows = [row_from_test(test, matrix, chain, probes) for test in matrix["tests"]]
    repair_queue = build_repair_queue(rows)
    research_loops = build_research_loops(rows)
    stage_summary = build_stage_summary(chain, rows)

    write_jsonl(args.base / "converge_rows.jsonl", rows)
    write_json(args.base / "repair_queue.json", repair_queue)
    write_jsonl(args.base / "research_loops.jsonl", research_loops)
    write_json(args.base / "stage_summary.json", stage_summary)
    render_evidence_heatmap(matrix, rows, args.base / "evidence_heatmap.svg")

    counts = {status: sum(1 for row in rows if row["class"] == status) for status in STATUS_COLORS}
    print(f"rows={len(rows)} pass={counts['pass']} fail={counts['fail']} blocked={counts['blocked']} skip={counts['skip']}")
    print(f"repair_queue={len(repair_queue)} research_loops={len(research_loops)}")
    print(f"wrote {args.base}")


if __name__ == "__main__":
    main()
