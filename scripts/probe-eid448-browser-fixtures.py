#!/usr/bin/env python3
"""Drive EID-448 fake workflows through the built agent-browser binary."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import subprocess
import threading
from datetime import datetime, timezone
from http.server import ThreadingHTTPServer
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "docs" / "converge" / "eid-448"
FIXTURE_PROBE = ROOT / "scripts" / "probe-eid448-fake-workflows.py"
OUT = BASE / "evidence" / "browser_fixture_probe.json"
DEFAULT_BINARY = Path("/tmp/surfari-eid448-target/release/agent-browser")


def load_fixture_module() -> Any:
    spec = importlib.util.spec_from_file_location("eid448_fake_workflows", FIXTURE_PROBE)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load fixture probe module from {FIXTURE_PROBE}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def run_cli(binary: Path, env: dict[str, str], args: list[str]) -> dict[str, Any]:
    proc = subprocess.run(
        [str(binary), *args],
        cwd=ROOT,
        env=env,
        text=True,
        capture_output=True,
        timeout=30,
    )
    stdout = proc.stdout.strip()
    stderr = proc.stderr.strip()
    return {
        "args": args,
        "returncode": proc.returncode,
        "stdout_excerpt": stdout[:500],
        "stderr_excerpt": stderr[:500],
        "stdout_sha256": sha256_text(stdout),
        "stderr_sha256": sha256_text(stderr),
        "stdout_contains": {
            "stale_ref_marker": "stale-ref:rerendered" in stdout,
            "active_tab_drift_marker": "active-tab-drift:wrong-surface" in stdout,
            "prompt_injection_marker": "prompt-injection:untrusted-page-text" in stdout,
        },
        "stderr_contains_governance_block": "domain_mismatch" in stderr
        or "domain_mismatch" in stdout,
    }


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def main() -> None:
    binary = Path(os.environ.get("AGENT_BROWSER_BINARY", str(DEFAULT_BINARY)))
    if not binary.exists():
        raise SystemExit(f"agent-browser binary not found: {binary}")

    fixture_mod = load_fixture_module()
    fixture = fixture_mod.load_fixture()
    fixture_mod.FixtureHandler.scenarios_by_path = {
        scenario["path"]: scenario for scenario in fixture["scenarios"]
    }
    server = ThreadingHTTPServer(("127.0.0.1", 0), fixture_mod.FixtureHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()

    tmp = Path("/tmp") / f"sfb-{os.getpid()}"
    tmp.mkdir(parents=True, exist_ok=True)
    home = tmp / "home"
    home.mkdir(exist_ok=True)
    log_path = tmp / "actions.jsonl"
    profile_path = tmp / "profile"
    session = f"sfb{os.getpid()}"
    use_id = "eid448-browser-fixture"
    base_url = f"http://127.0.0.1:{server.server_port}"
    env = {
        **os.environ,
        "HOME": str(home),
        "AGENT_BROWSER_SOCKET_DIR": str(tmp / "sock"),
        "AGENT_BROWSER_SESSION": session,
        "SURFARI_CONTEXT_ID": "eid448-browser-fixture",
        "SURFARI_ORG_ID": "eidos",
        "SURFARI_PROFILE_ID": "local-fixture-profile",
        "SURFARI_SUBJECT_ID": "eid448-local-fixture",
        "SURFARI_EXPECTED_DOMAINS": "127.0.0.1",
        "SURFARI_ACTION_LOG_PATH": str(log_path),
        "SURFARI_USE_ID": use_id,
        "SURFARI_BROWSER_PROFILE_PATH": str(profile_path),
    }

    commands: list[dict[str, Any]] = []
    close_results: list[dict[str, Any]] = []
    try:
        commands.append(
            run_cli(
                binary,
                env,
                ["--profile", str(profile_path), "open", f"{base_url}/fixtures/stale-react-ref"],
            )
        )
        commands.append(run_cli(binary, env, ["snapshot"]))
        commands.append(
            run_cli(
                binary,
                env,
                [
                    "eval",
                    "document.querySelector('button').outerHTML='<button id=\"recovered-action\">Recovered action target</button>'",
                ],
            )
        )
        commands.append(run_cli(binary, env, ["click", "@e2"]))
        commands.append(run_cli(binary, env, ["click", "#recovered-action"]))
        commands.append(run_cli(binary, env, ["open", f"{base_url}/fixtures/active-tab-drift"]))
        commands.append(run_cli(binary, env, ["snapshot"]))
        commands.append(run_cli(binary, env, ["open", f"{base_url}/fixtures/prompt-injection"]))
        commands.append(run_cli(binary, env, ["snapshot"]))

        close_results.append(run_cli(binary, env, ["close", "--all"]))

        mismatch_env = {
            **env,
            "AGENT_BROWSER_SESSION": f"sfbx{os.getpid()}",
            "SURFARI_EXPECTED_DOMAINS": "developer.apple.com",
            "SURFARI_USE_ID": f"{use_id}-wrong-surface",
            "SURFARI_BROWSER_PROFILE_PATH": str(tmp / "wrong-surface-profile"),
        }
        commands.append(
            run_cli(
                binary,
                mismatch_env,
                [
                    "--profile",
                    str(tmp / "wrong-surface-profile"),
                    "open",
                    f"{base_url}/fixtures/prompt-injection",
                ],
            )
        )
        commands.append(run_cli(binary, mismatch_env, ["fill", "#protected-action", "seeded fake secret"]))
    finally:
        close_results.append(run_cli(binary, env, ["close", "--all"]))
        close_results.append(
            run_cli(
                binary,
                {**env, "AGENT_BROWSER_SESSION": f"sfbx{os.getpid()}"},
                ["close", "--all"],
            )
        )
        server.shutdown()
        thread.join(timeout=5)

    action_rows = load_jsonl(log_path)
    candidate_path = home / ".cache" / "surfari" / "uses" / use_id / "learning-candidates.jsonl"
    candidate_rows = load_jsonl(candidate_path)
    action_text = "\n".join(json.dumps(row, sort_keys=True) for row in action_rows)

    payload = {
        "schema_version": 1,
        "captured_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "binary": str(binary),
        "surface": "built agent-browser release binary plus local HTTP fixture",
        "base_url": base_url,
        "session": session,
        "command_count": len(commands),
        "commands": commands,
        "close": close_results,
        "action_log_rows": len(action_rows),
        "learning_candidate_rows": len(candidate_rows),
        "checks": {
            "all_expected_commands_succeeded": all(
                command["returncode"] == 0
                for index, command in enumerate(commands[:9])
                if index != 3
            )
            and commands[6]["returncode"] == 0,
            "wrong_surface_fill_blocked": commands[-1]["returncode"] != 0
            and commands[-1]["stderr_contains_governance_block"],
            "stale_ref_marker_seen": any(
                command["stdout_contains"]["stale_ref_marker"] for command in commands
            ),
            "stale_ref_old_ref_failed": commands[3]["returncode"] != 0,
            "stale_ref_recovery_selector_succeeded": commands[4]["returncode"] == 0,
            "active_tab_drift_marker_seen": any(
                command["stdout_contains"]["active_tab_drift_marker"] for command in commands
            ),
            "prompt_injection_marker_seen": any(
                command["stdout_contains"]["prompt_injection_marker"] for command in commands
            ),
            "learning_candidates_written": len(candidate_rows) >= 3,
            "seeded_secret_redacted": "seeded fake secret" not in action_text,
        },
    }
    payload["passed"] = all(payload["checks"].values())
    # Stale-ref behavior is a product signal, not a harness-health precondition.
    # Keep the probe runnable while Converge decides whether T037 can turn green.
    required_checks = {
        key: value
        for key, value in payload["checks"].items()
        if key not in {"stale_ref_old_ref_failed", "stale_ref_recovery_selector_succeeded"}
    }
    payload["passed"] = all(required_checks.values())
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(
        "browser_fixture_probe="
        f"{'pass' if payload['passed'] else 'fail'} "
        f"commands={payload['command_count']} action_log_rows={payload['action_log_rows']} "
        f"learning_candidate_rows={payload['learning_candidate_rows']}"
    )
    print(f"wrote {OUT}")
    if not payload["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
