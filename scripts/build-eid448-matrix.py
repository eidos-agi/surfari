#!/usr/bin/env python3
"""Build the EID-448 planned test coverage matrix."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "docs" / "plans" / "eid-448-test-coverage-matrix.json"


DEPTH_LEVELS = {
    "1": "observed or configured",
    "2": "exercised by command or fixture",
    "3": "asserted by automated check",
    "4": "critical human or safety gate asserted"
}


CAPABILITIES = [
    ("A", "Repo path verified", "Repo"),
    ("B", "Origin and upstream verified", "Repo"),
    ("C", "Branch and HEAD verified", "Repo"),
    ("D", "Upstream freshness known", "Repo"),
    ("E", "Dirty state classified", "Repo"),
    ("F", "Node version verified", "Build"),
    ("G", "pnpm or Corepack ready", "Build"),
    ("H", "Rust and Cargo verified", "Build"),
    ("I", "Native compile or tests pass", "Build"),
    ("J", "Release binary built", "Build"),
    ("K", "Package scripts verified", "Install"),
    ("L", "Wrapper path found", "Install"),
    ("M", "Wrapper routes to fork", "Install"),
    ("N", "Installed CLI resolves to fork", "Install"),
    ("O", "Surfari context env captured", "Context"),
    ("P", "Expected domains parsed", "Context"),
    ("Q", "Account profile session metadata captured", "Context"),
    ("R", "Knox ref metadata only", "Context"),
    ("S", "Daemon session starts", "Browser Surface"),
    ("T", "Active tab id captured", "Browser Surface"),
    ("U", "Active URL and domain observed", "Browser Surface"),
    ("V", "Tab count and surface list observed", "Browser Surface"),
    ("W", "Browser profile session anchored", "Browser Surface"),
    ("X", "Human-gate domain classified", "Governance", True),
    ("Y", "Read-only action allowed", "Governance"),
    ("Z", "Protected action attempted", "Governance"),
    ("AA", "Protected action blocked pre-mutation", "Governance"),
    ("AB", "Wrong-domain mismatch detected", "Governance"),
    ("AC", "Partial context fails closed", "Governance"),
    ("AD", "Sign-in surface detected", "Governance", True),
    ("AE", "Stale ref detected", "Recovery"),
    ("AF", "DOM-confirmed recovery used", "Recovery"),
    ("AG", "Active-tab drift simulated", "Recovery"),
    ("AH", "Wrong active surface blocked", "Recovery"),
    ("AI", "Redirect observed", "Fixtures"),
    ("AJ", "Download handoff modeled", "Fixtures"),
    ("AK", "Profile state reported", "Fixtures"),
    ("AL", "Decision logged", "Logs"),
    ("AM", "Lifecycle log written", "Logs"),
    ("AN", "Redaction no secrets proven", "Logs"),
    ("AO", "Learning candidate safe", "Logs"),
    ("AP", "Fixture server running", "Fixtures"),
    ("AQ", "Fake Apple flow covered", "Fixtures"),
    ("AR", "Fake SaaS multi-account covered", "Fixtures"),
    ("AS", "Prompt injection resisted", "Fixtures"),
    ("AT", "Repeat-run improvement measured", "Learning"),
    ("AU", "Real Apple read-only observed", "Real Workflow"),
    ("AV", "Real human gate respected", "Real Workflow", True),
    ("AW", "Human confirms account and team", "Real Workflow", True),
    ("AX", "Linear proof or merge decision", "Real Workflow"),
]


TESTS = [
    (1, "Repo path check", "Repo and Build", {"A": 3}),
    (2, "Remote check", "Repo and Build", {"A": 1, "B": 3}),
    (3, "Branch and HEAD check", "Repo and Build", {"A": 1, "B": 1, "C": 3}),
    (4, "Upstream freshness check", "Repo and Build", {"A": 1, "B": 2, "C": 1, "D": 3}),
    (5, "Dirty-state classification", "Repo and Build", {"A": 1, "B": 1, "C": 1, "D": 1, "E": 3}),
    (6, "Node version check", "Repo and Build", {"F": 3}),
    (7, "pnpm Corepack check", "Repo and Build", {"F": 1, "G": 3, "K": 1}),
    (8, "Rust toolchain check", "Repo and Build", {"H": 3}),
    (9, "Package script inventory", "Repo and Build", {"F": 1, "G": 1, "K": 3}),
    (10, "Rust compile check", "Repo and Build", {"H": 1, "I": 3}),
    (11, "Native binary build", "Repo and Build", {"F": 1, "G": 2, "H": 2, "I": 2, "J": 3, "K": 2}),
    (12, "Wrapper discovery", "Repo and Build", {"L": 3, "N": 1}),
    (13, "Wrapper routes to fork", "Repo and Build", {"J": 2, "L": 2, "M": 3, "N": 3}),
    (14, "Installed CLI smoke", "Repo and Build", {"J": 1, "L": 1, "M": 2, "N": 3, "S": 1}),
    (15, "Context env capture unit", "Governance Logic", {"O": 3, "Q": 2, "R": 2}),
    (16, "Expected-domain parser unit", "Governance Logic", {"O": 1, "P": 3}),
    (17, "Context metadata capture", "Governance Logic", {"O": 3, "P": 2, "Q": 3, "R": 2}),
    (18, "Knox ref metadata boundary", "Governance Logic", {"O": 2, "Q": 1, "R": 3, "AN": 2}),
    (19, "Human-gate classifier unit", "Governance Logic", {"X": 3, "AD": 2}),
    (20, "Read-only human-gate decision", "Governance Logic", {"P": 2, "X": 3, "Y": 3, "AD": 2}),
    (21, "Protected human-gate block", "Governance Logic", {"P": 2, "X": 3, "Z": 2, "AA": 3, "AD": 3}),
    (22, "Expected domain still blocks", "Governance Logic", {"O": 1, "P": 3, "X": 3, "Z": 2, "AA": 3, "AD": 3}),
    (23, "Wrong-domain block", "Governance Logic", {"P": 2, "U": 2, "Z": 2, "AA": 3, "AB": 3}),
    (24, "Partial-context block", "Governance Logic", {"O": 2, "Z": 2, "AA": 3, "AC": 3}),
    (25, "Daemon starts with context", "Daemon and Logs", {"O": 2, "Q": 1, "S": 3}),
    (26, "Daemon anchor capture", "Daemon and Logs", {"O": 1, "S": 2, "T": 3, "U": 3, "V": 2, "W": 2}),
    (27, "Daemon read-only pass", "Daemon and Logs", {"O": 2, "P": 2, "S": 2, "T": 1, "U": 2, "Y": 3}),
    (28, "Daemon prelaunch block", "Daemon and Logs", {"O": 2, "P": 2, "S": 2, "X": 3, "Z": 2, "AA": 3, "AD": 2}),
    (29, "Governance log metadata", "Daemon and Logs", {"O": 2, "P": 2, "S": 2, "X": 3, "Z": 2, "AA": 3, "AD": 3, "AL": 3, "AM": 3}),
    (30, "Redaction seeded secret", "Daemon and Logs", {"O": 2, "R": 2, "S": 2, "Z": 2, "AA": 2, "AM": 3, "AN": 3}),
    (31, "Learning candidate safe shape", "Daemon and Logs", {"O": 2, "S": 2, "T": 2, "U": 2, "AM": 2, "AN": 3, "AO": 3}),
    (32, "Fixture server boot", "Fake Browser Workflows", {"AP": 3}),
    (33, "Local fake Apple redirect", "Fake Browser Workflows", {"O": 1, "P": 2, "S": 2, "T": 2, "U": 3, "AI": 3, "AP": 2, "AQ": 3}),
    (34, "Sign-in redirect then block", "Fake Browser Workflows", {"O": 2, "P": 2, "S": 2, "T": 2, "U": 3, "X": 3, "Z": 2, "AA": 3, "AD": 3, "AI": 3, "AP": 2, "AQ": 3}),
    (35, "Fake download handoff", "Fake Browser Workflows", {"O": 2, "P": 2, "S": 2, "T": 2, "U": 2, "Y": 1, "AJ": 3, "AK": 3, "AP": 2, "AQ": 2}),
    (36, "Stale React ref fixture", "Fake Browser Workflows", {"O": 1, "S": 2, "T": 2, "Y": 3, "AE": 3, "AF": 2, "AP": 2}),
    (37, "Stale ref recovery", "Fake Browser Workflows", {"O": 2, "P": 1, "S": 2, "T": 2, "U": 2, "Z": 2, "AA": 2, "AE": 3, "AF": 3, "AP": 2}),
    (38, "Active-tab drift fixture", "Fake Browser Workflows", {"O": 2, "P": 2, "S": 2, "T": 3, "U": 2, "V": 2, "AG": 3, "AP": 2}),
    (39, "Drift wrong-surface block", "Fake Browser Workflows", {"O": 2, "P": 2, "S": 2, "T": 3, "U": 3, "V": 2, "Z": 2, "AA": 3, "AB": 3, "AG": 3, "AH": 3, "AP": 2}),
    (40, "Fake multi-account SaaS", "Fake Browser Workflows", {"O": 2, "Q": 3, "S": 2, "T": 2, "U": 2, "W": 2, "AB": 2, "AR": 3, "AP": 2}),
    (41, "Prompt injection fixture", "Fake Browser Workflows", {"O": 2, "P": 2, "S": 2, "Y": 1, "AA": 2, "AN": 2, "AS": 3, "AP": 2}),
    (42, "Repeat-run learning compare", "Fake Browser Workflows", {"O": 2, "S": 2, "T": 2, "U": 2, "AM": 2, "AN": 2, "AO": 3, "AT": 3, "AP": 2}),
    (43, "Wrapper plus fake flow smoke", "Fake Browser Workflows", {"J": 2, "L": 2, "M": 3, "N": 3, "O": 2, "S": 2, "T": 2, "U": 2, "AI": 2, "AQ": 2, "AP": 2}),
    (44, "Built binary matrix smoke", "Fake Browser Workflows", {"A": 1, "B": 1, "C": 1, "D": 1, "E": 1, "F": 2, "G": 2, "H": 2, "I": 3, "J": 3, "K": 2, "L": 2, "M": 2, "N": 2, "O": 2, "P": 2, "S": 2, "AM": 2, "AN": 2}),
    (45, "Knox profile-session dry run", "Real Knox Apple Dry Runs", {"A": 1, "B": 1, "C": 1, "D": 1, "E": 1, "F": 1, "G": 2, "H": 2, "I": 3, "J": 3, "K": 2, "L": 2, "M": 2, "N": 2, "O": 3, "P": 3, "Q": 3, "R": 3, "S": 2, "T": 2, "U": 2, "W": 2, "X": 3, "Y": 2, "Z": 2, "AA": 3, "AD": 3, "AJ": 2, "AK": 3, "AL": 2, "AM": 2, "AN": 3, "AV": 4}),
    (46, "Real Apple read-only surface", "Real Knox Apple Dry Runs", {"A": 1, "C": 1, "J": 2, "L": 2, "M": 2, "N": 2, "O": 3, "P": 3, "Q": 3, "R": 3, "S": 2, "T": 3, "U": 3, "V": 2, "W": 3, "X": 2, "Y": 3, "AD": 2, "AU": 3, "AV": 4}),
    (47, "Real Apple sign-in gate stop", "Real Knox Apple Dry Runs", {"O": 3, "P": 3, "Q": 3, "R": 3, "S": 2, "T": 3, "U": 3, "W": 3, "X": 3, "Y": 2, "Z": 2, "AA": 4, "AD": 4, "AL": 3, "AM": 3, "AN": 3, "AU": 2, "AV": 4}),
    (48, "Human account-team confirm", "Real Knox Apple Dry Runs", {"O": 2, "P": 2, "Q": 3, "R": 2, "S": 2, "T": 3, "U": 3, "W": 3, "X": 2, "Y": 3, "AD": 3, "AU": 3, "AV": 4, "AW": 4}),
    (49, "Profile install approval gate", "Real Knox Apple Dry Runs", {"O": 2, "P": 2, "Q": 3, "R": 3, "S": 2, "T": 2, "U": 2, "AJ": 3, "AK": 3, "AL": 2, "AM": 2, "AN": 3, "AU": 2, "AV": 4, "AW": 4}),
    (50, "Final proof merge review", "Real Knox Apple Dry Runs", {"A": 3, "B": 3, "C": 3, "D": 3, "E": 3, "F": 3, "G": 3, "H": 3, "I": 3, "J": 3, "K": 3, "L": 3, "M": 3, "N": 3, "O": 3, "P": 3, "Q": 3, "R": 3, "S": 2, "T": 2, "U": 2, "V": 2, "W": 2, "X": 3, "Y": 3, "Z": 3, "AA": 3, "AB": 3, "AC": 3, "AD": 3, "AE": 2, "AF": 2, "AG": 2, "AH": 2, "AI": 2, "AJ": 2, "AK": 3, "AL": 3, "AM": 3, "AN": 3, "AO": 3, "AP": 2, "AQ": 2, "AR": 2, "AS": 2, "AT": 2, "AU": 2, "AV": 4, "AW": 4, "AX": 4}),
]


def main() -> None:
    capabilities = []
    for item in CAPABILITIES:
        code, label, group, *rest = item
        cap = {"code": code, "label": label, "group": group}
        if rest and rest[0]:
            cap["critical"] = True
        capabilities.append(cap)

    tests = [
        {
            "id": test_id,
            "name": name,
            "group": group,
            "coverage": coverage,
            "capabilities": sorted(coverage, key=lambda code: [cap["code"] for cap in capabilities].index(code)),
        }
        for test_id, name, group, coverage in TESTS
    ]

    data = {
        "title": "Surfari Knox Test Coverage Matrix",
        "subtitle": "EID-448 planned proof ladder for governed browser workflows",
        "schema_version": 2,
        "matrix_shape": "50x50",
        "depth_levels": DEPTH_LEVELS,
        "capabilities": capabilities,
        "tests": tests,
    }

    OUTPUT.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    covered = sum(len(test["coverage"]) for test in tests)
    weighted = sum(sum(test["coverage"].values()) for test in tests)
    print(f"wrote {OUTPUT}")
    print(f"tests={len(tests)} capabilities={len(capabilities)} covered_cells={covered} weighted_depth={weighted}")


if __name__ == "__main__":
    main()
