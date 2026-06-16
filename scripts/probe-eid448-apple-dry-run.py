#!/usr/bin/env python3
"""Run the approved metadata-only EID-448 Apple Developer dry run.

This probe uses the real Eidos `ab` wrapper after approval. It performs only
read-only actions, strips query/fragment data from recorded URLs, and stops at
Apple sign-in/human gates.
"""

from __future__ import annotations

import json
import os
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import urlparse, urlunparse


ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "docs" / "converge" / "eid-448"
OUT = BASE / "evidence" / "apple_dry_run_probe.json"
WRAPPER = Path(
    "/Volumes/MacMiniStorage/Eidos/repos-eidos-agi/eidosagi.com/tools/agent-browser/ab"
)
APPLE_URL = "https://developer.apple.com/account/resources/profiles/list"
EXPECTED_DOMAINS = {"developer.apple.com", "idmsa.apple.com", "appleid.apple.com"}


def sanitized_url(raw: str) -> str | None:
    raw = raw.strip()
    if not raw:
        return None
    parsed = urlparse(raw)
    if not parsed.scheme or not parsed.netloc:
        return None
    return urlunparse((parsed.scheme, parsed.netloc, parsed.path, "", "", ""))


def domain(raw: str | None) -> str | None:
    if not raw:
        return None
    parsed = urlparse(raw)
    return parsed.netloc.lower() or None


def run_ab(env: dict[str, str], args: list[str]) -> dict[str, Any]:
    proc = subprocess.run(
        [str(WRAPPER), *args],
        cwd=WRAPPER.parent,
        env=env,
        text=True,
        capture_output=True,
        timeout=45,
    )
    stdout = proc.stdout.strip()
    stderr = proc.stderr.strip()
    return {
        "args": args,
        "returncode": proc.returncode,
        "stdout_lines": stdout.splitlines()[:3],
        "stderr_lines": stderr.splitlines()[:3],
    }


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]


def main() -> None:
    if not WRAPPER.exists():
        raise SystemExit(f"wrapper not found: {WRAPPER}")

    tmp = Path("/tmp") / f"sfa-{os.getpid()}"
    tmp.mkdir(parents=True, exist_ok=True)
    log_path = tmp / "actions.jsonl"
    profile_path = tmp / "profile"
    session = f"sfa{os.getpid()}"
    env = {
        **os.environ,
        "HOME": str(tmp / "home"),
        "AGENT_BROWSER_SOCKET_DIR": str(tmp / "sock"),
        "AGENT_BROWSER_SESSION": session,
        "SURFARI_CONTEXT_ID": "eid448-apple-dry-run",
        "SURFARI_ORG_ID": "eidos",
        "SURFARI_ACCOUNT_ID": "apple-developer",
        "SURFARI_PROFILE_ID": "isolated-apple-dry-run",
        "SURFARI_SUBJECT_ID": "Eidos Knox",
        "SURFARI_KNOX_REF": "knox://eidos/knox/apple-developer",
        "SURFARI_EXPECTED_DOMAINS": ",".join(sorted(EXPECTED_DOMAINS)),
        "SURFARI_ACTION_LOG_PATH": str(log_path),
        "SURFARI_USE_ID": "eid448-apple-dry-run",
        "SURFARI_BROWSER_PROFILE_PATH": str(profile_path),
    }

    commands: list[dict[str, Any]] = []
    try:
        commands.append(run_ab(env, ["--version"]))
        commands.append(run_ab(env, ["--profile", str(profile_path), "open", APPLE_URL]))
        commands.append(run_ab(env, ["get", "url"]))
    finally:
        close_result = run_ab(env, ["close", "--all"])

    current_url_raw = ""
    if len(commands) >= 3 and commands[2]["stdout_lines"]:
        current_url_raw = commands[2]["stdout_lines"][-1]
    current_url = sanitized_url(current_url_raw)
    current_domain = domain(current_url)
    action_rows = read_jsonl(log_path)
    action_text = "\n".join(json.dumps(row, sort_keys=True) for row in action_rows)

    checks = {
        "wrapper_routes_to_surfari": commands[0]["returncode"] == 0
        and any("agent-browser 0.27.1" in line for line in commands[0]["stdout_lines"]),
        "open_command_completed": commands[1]["returncode"] == 0,
        "current_domain_allowed": current_domain in EXPECTED_DOMAINS,
        "no_protected_action_attempted": all(
            row.get("action") in {"launch", "navigate", "url", "close"}
            for row in action_rows
        ),
        "no_forbidden_secret_terms_in_log": not any(
            term in action_text.lower()
            for term in ["password", "mfa", "otp", "passkey", "provisioningprofile"]
        ),
    }
    human_gate_observed = current_domain in {"idmsa.apple.com", "appleid.apple.com"}

    payload = {
        "schema_version": 1,
        "captured_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "workflow": "Eidos Knox Apple Developer profile resume",
        "wrapper": str(WRAPPER),
        "target_url": APPLE_URL,
        "current_url_sanitized": current_url,
        "current_domain": current_domain,
        "human_gate_observed": human_gate_observed,
        "commands": commands,
        "close": close_result,
        "action_log_rows": len(action_rows),
        "allowed_domains": sorted(EXPECTED_DOMAINS),
        "forbidden_actions_attempted": [],
        "checks": checks,
        "passed": all(checks.values()),
        "stopped_before": [
            "password entry",
            "MFA or OTP entry",
            "passkey approval",
            "legal agreement",
            "payment or billing",
            "profile download",
            "profile install",
            "final submission",
        ],
    }
    OUT.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(
        "apple_dry_run_probe="
        f"{'pass' if payload['passed'] else 'fail'} "
        f"domain={current_domain} human_gate={human_gate_observed} "
        f"action_log_rows={len(action_rows)}"
    )
    print(f"wrote {OUT}")
    if not payload["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
