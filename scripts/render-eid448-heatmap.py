#!/usr/bin/env python3
"""Render the EID-448 Surfari test coverage matrix as a data-backed SVG."""

from __future__ import annotations

import argparse
import html
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_INPUT = ROOT / "docs" / "plans" / "eid-448-test-coverage-matrix.json"
DEFAULT_OUTPUT = ROOT / "docs" / "plans" / "eid-448-test-coverage-heatmap.svg"


COLORS = {
    "bg": "#0b1020",
    "panel": "#111827",
    "panel_alt": "#0f172a",
    "grid": "#263247",
    "empty": "#172033",
    "empty_alt": "#1c263a",
    "depth_1": "#1f7a8c",
    "depth_2": "#17b897",
    "depth_3": "#67e8c7",
    "depth_4": "#f5b84b",
    "critical": "#f5b84b",
    "critical_hi": "#ffd36e",
    "text": "#ecfdf5",
    "muted": "#9fb0c7",
    "subtle": "#607089",
    "accent": "#38bdf8"
}


DEPTH_LABELS = {
    1: "observed",
    2: "exercised",
    3: "asserted",
    4: "critical gate",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, default=DEFAULT_INPUT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    return parser.parse_args()


def esc(value: object) -> str:
    return html.escape(str(value), quote=True)


def load_matrix(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        data = json.load(handle)
    capabilities = data.get("capabilities", [])
    tests = data.get("tests", [])
    shape = data.get("matrix_shape")
    if shape:
        expected_rows, expected_cols = [int(part) for part in shape.lower().split("x")]
    else:
        expected_rows = expected_cols = len(capabilities)
    if len(capabilities) != expected_cols:
        raise ValueError(f"expected {expected_cols} capabilities, got {len(capabilities)}")
    if len(tests) != expected_rows:
        raise ValueError(f"expected {expected_rows} tests, got {len(tests)}")
    codes = [cap["code"] for cap in capabilities]
    if len(set(codes)) != len(codes):
        raise ValueError("capability codes must be unique")
    unknown = sorted(
        {
            code
            for test in tests
            for code in coverage_for_test(test)
            if code not in codes
        }
    )
    if unknown:
        raise ValueError(f"unknown capability codes: {', '.join(unknown)}")
    for test in tests:
        for code, depth in coverage_for_test(test).items():
            if depth not in {1, 2, 3, 4}:
                raise ValueError(f"invalid depth {depth!r} for test {test.get('id')} capability {code}")
    return data


def coverage_for_test(test: dict) -> dict[str, int]:
    if "coverage" in test:
        return {str(code): int(depth) for code, depth in test["coverage"].items()}
    return {str(code): 2 for code in test.get("capabilities", [])}


def group_spans(items: list[dict], key: str) -> list[tuple[str, int, int]]:
    spans: list[tuple[str, int, int]] = []
    start = 0
    current = items[0][key]
    for index, item in enumerate(items[1:], start=1):
        if item[key] != current:
            spans.append((current, start, index - start))
            start = index
            current = item[key]
    spans.append((current, start, len(items) - start))
    return spans


def cell_color(cap: dict, depth: int | None, row_index: int) -> str:
    if depth is None:
        return COLORS["empty_alt"] if row_index % 2 else COLORS["empty"]
    if cap.get("critical") and depth >= 3:
        return COLORS["critical_hi"] if depth == 4 else COLORS["critical"]
    return COLORS[f"depth_{depth}"]


def render(data: dict) -> str:
    capabilities = data["capabilities"]
    tests = data["tests"]
    for test in tests:
        test["coverage_map"] = coverage_for_test(test)
        test["capability_set"] = set(test["coverage_map"])
        test["count"] = len(test["coverage_map"])
        test["weighted_depth"] = sum(test["coverage_map"].values())

    width = 2560
    height = 1700
    margin_left = 520
    margin_top = 250
    cell = 20
    gap = 3
    matrix_w = len(capabilities) * (cell + gap) - gap
    matrix_h = len(tests) * (cell + gap) - gap
    x0 = margin_left
    y0 = margin_top

    max_count = max(test["count"] for test in tests)
    avg_count = sum(test["count"] for test in tests) / len(tests)
    coverage_cells = sum(test["count"] for test in tests)
    weighted_depth = sum(test["weighted_depth"] for test in tests)
    max_weighted_depth = len(tests) * len(capabilities) * 4
    total_cells = len(tests) * len(capabilities)
    coverage_pct = coverage_cells / total_cells * 100
    depth_pct = weighted_depth / max_weighted_depth * 100

    svg: list[str] = []
    svg.append(
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}" role="img" aria-labelledby="title desc">'
    )
    svg.append(f'<title id="title">{esc(data["title"])}</title>')
    svg.append(
        f'<desc id="desc">Data-backed {len(tests)} by {len(capabilities)} heatmap with '
        f'{coverage_cells} covered cells and weighted depth {weighted_depth}.</desc>'
    )
    svg.append("<defs>")
    svg.append(
        '<linearGradient id="bgGrad" x1="0" y1="0" x2="1" y2="1">'
        '<stop offset="0" stop-color="#0b1020"/><stop offset="1" stop-color="#111827"/>'
        "</linearGradient>"
    )
    svg.append(
        '<filter id="softShadow" x="-10%" y="-10%" width="120%" height="120%">'
        '<feDropShadow dx="0" dy="12" stdDeviation="16" flood-color="#020617" flood-opacity="0.35"/>'
        "</filter>"
    )
    svg.append("</defs>")
    svg.append(f'<rect width="{width}" height="{height}" fill="url(#bgGrad)"/>')
    svg.append(
        f'<rect x="40" y="40" width="{width - 80}" height="{height - 80}" '
        f'rx="18" fill="{COLORS["panel"]}" filter="url(#softShadow)"/>'
    )

    svg.append(
        f'<text x="72" y="96" font-family="Inter, Arial, sans-serif" '
        f'font-size="38" font-weight="700" fill="{COLORS["text"]}">{esc(data["title"])}</text>'
    )
    svg.append(
        f'<text x="72" y="132" font-family="Inter, Arial, sans-serif" '
        f'font-size="18" fill="{COLORS["muted"]}">{esc(data["subtitle"])}</text>'
    )
    svg.append(
        f'<text x="72" y="166" font-family="Inter, Arial, sans-serif" '
        f'font-size="15" fill="{COLORS["subtle"]}">{len(tests)} ordered tests x '
        f'{len(capabilities)} ordered capabilities. {coverage_cells}/{total_cells} cells covered '
        f'({coverage_pct:.1f}%). Weighted depth {weighted_depth}/{max_weighted_depth} '
        f'({depth_pct:.1f}%). Average {avg_count:.1f}, max {max_count}.</text>'
    )

    legend_x = 1370
    legend_y = 82
    legend = [
        ("Not covered", COLORS["empty"]),
        ("Observed", COLORS["depth_1"]),
        ("Exercised", COLORS["depth_2"]),
        ("Asserted", COLORS["depth_3"]),
        ("Critical gate", COLORS["depth_4"]),
    ]
    for index, (label, color) in enumerate(legend):
        lx = legend_x + index * 142
        svg.append(f'<rect x="{lx}" y="{legend_y}" width="22" height="22" rx="5" fill="{color}"/>')
        svg.append(
            f'<text x="{lx + 30}" y="{legend_y + 16}" font-family="Inter, Arial, sans-serif" '
            f'font-size="13" fill="{COLORS["muted"]}">{esc(label)}</text>'
        )

    for group, start, span in group_spans(capabilities, "group"):
        gx = x0 + start * (cell + gap)
        gw = span * (cell + gap) - gap
        svg.append(
            f'<rect x="{gx}" y="{y0 - 74}" width="{gw}" height="26" rx="8" '
            f'fill="{COLORS["panel_alt"]}" stroke="{COLORS["grid"]}" stroke-width="1"/>'
        )
        svg.append(
            f'<text x="{gx + gw / 2}" y="{y0 - 56}" text-anchor="middle" '
            f'font-family="Inter, Arial, sans-serif" font-size="10" fill="{COLORS["muted"]}">'
            f'{esc(group)}</text>'
        )

    for col, cap in enumerate(capabilities):
        cx = x0 + col * (cell + gap)
        svg.append(
            f'<text x="{cx + cell / 2}" y="{y0 - 18}" text-anchor="middle" '
            f'font-family="Inter, Arial, sans-serif" font-size="10" font-weight="700" '
            f'fill="{COLORS["text"]}">{esc(cap["code"])}</text>'
        )

    for group, start, span in group_spans(tests, "group"):
        gy = y0 + start * (cell + gap)
        gh = span * (cell + gap) - gap
        svg.append(
            f'<rect x="58" y="{gy - 4}" width="10" height="{gh + 8}" rx="5" fill="{COLORS["accent"]}" opacity="0.65"/>'
        )
        svg.append(
            f'<text x="82" y="{gy + 18}" font-family="Inter, Arial, sans-serif" '
            f'font-size="11" font-weight="700" fill="{COLORS["accent"]}">{esc(group)}</text>'
        )

    for row, test in enumerate(tests):
        ty = y0 + row * (cell + gap)
        if row % 2:
            svg.append(
                f'<rect x="72" y="{ty - 2}" width="{matrix_w + margin_left - 72 + 92}" height="{cell + 4}" '
                f'rx="8" fill="#0b1224" opacity="0.42"/>'
            )
        svg.append(
            f'<text x="92" y="{ty + 21}" font-family="Inter, Arial, sans-serif" '
            f'font-size="11" fill="{COLORS["muted"]}">T{test["id"]:02d}</text>'
        )
        svg.append(
            f'<text x="138" y="{ty + 21}" font-family="Inter, Arial, sans-serif" '
            f'font-size="12" fill="{COLORS["text"]}">{esc(test["name"])}</text>'
        )
        for col, cap in enumerate(capabilities):
            depth = test["coverage_map"].get(cap["code"])
            cx = x0 + col * (cell + gap)
            color = cell_color(cap, depth, row)
            opacity = "1" if depth is not None else "0.72"
            stroke = COLORS["critical_hi"] if depth is not None and cap.get("critical") else COLORS["grid"]
            label = DEPTH_LABELS.get(depth, "not covered") if depth is not None else "not covered"
            svg.append(
                f'<rect x="{cx}" y="{ty}" width="{cell}" height="{cell}" rx="7" '
                f'fill="{color}" stroke="{stroke}" stroke-width="1" opacity="{opacity}">'
                f'<title>T{test["id"]:02d} {esc(test["name"])} / {esc(cap["code"])} '
                f'{esc(cap["label"])}: {esc(label)}</title></rect>'
            )
        count_x = x0 + matrix_w + 22
        bar_w = 128
        fill_w = bar_w * test["count"] / len(capabilities)
        svg.append(f'<rect x="{count_x}" y="{ty + 7}" width="{bar_w}" height="18" rx="9" fill="{COLORS["empty"]}"/>')
        svg.append(f'<rect x="{count_x}" y="{ty + 7}" width="{fill_w:.1f}" height="18" rx="9" fill="{COLORS["depth_2"]}"/>')
        svg.append(
            f'<text x="{count_x + bar_w + 12}" y="{ty + 21}" font-family="Inter, Arial, sans-serif" '
            f'font-size="12" fill="{COLORS["muted"]}">{test["count"]:02d}/30</text>'
        )

    footer_y = y0 + matrix_h + 46
    svg.append(
        f'<text x="{x0}" y="{footer_y}" font-family="Inter, Arial, sans-serif" '
        f'font-size="14" fill="{COLORS["muted"]}">Source: docs/plans/eid-448-test-coverage-matrix.json. '
        f'Renderer: scripts/render-eid448-heatmap.py.</text>'
    )

    svg.append("</svg>")
    return "\n".join(svg) + "\n"


def main() -> None:
    args = parse_args()
    data = load_matrix(args.input)
    svg = render(data)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(svg, encoding="utf-8")
    coverage = sum(len(test["capabilities"]) for test in data["tests"])
    weighted = sum(sum(coverage_for_test(test).values()) for test in data["tests"])
    print(f"rendered {args.output}")
    print(
        f"tests={len(data['tests'])} capabilities={len(data['capabilities'])} "
        f"covered_cells={coverage} weighted_depth={weighted}"
    )


if __name__ == "__main__":
    main()
