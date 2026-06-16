#!/usr/bin/env python3
"""Probe the Eidos ab wrapper shape against the built Surfari binary.

This script does not mutate the real eidosagi.com wrapper. It creates a
temporary wrapper clone, points node_modules/.bin/agent-browser at the built
Surfari binary, and drives the local fake workflow fixture through ./ab.
"""

from __future__ import annotations

import importlib.util
import json
import os
import shutil
import subprocess
import threading
from datetime import datetime, timezone
from http.server import ThreadingHTTPServer
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "docs" / "converge" / "eid-448"
OUT = BASE / "evidence" / "wrapper_fixture_probe.json"
FIXTURE_PROBE = ROOT / "scripts" / "probe-eid448-fake-workflows.py"
SOURCE_WRAPPER = Path(
    "/Volumes/MacMiniStorage/Eidos/repos-eidos-agi/eidosagi.com/tools/agent-browser/ab"
)
DEFAULT_BINARY = Path("/tmp/surfari-eid448-target/release/agent-browser")


def load_fixture_module() -> Any:
    spec = importlib.util.spec_from_file_location("eid448_fake_workflows", FIXTURE_PROBE)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load fixture probe module from {FIXTURE_PROBE}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run_wrapper(wrapper: Path, env: dict[str, str], args: list[str]) -> dict[str, Any]:
    proc = subprocess.run(
        [str(wrapper), *args],
        cwd=wrapper.parent,
        env=env,
        text=True,
        capture_output=True,
        timeout=30,
    )
    return {
        "args": args,
        "returncode": proc.returncode,
        "stdout_excerpt": proc.stdout.strip()[:500],
        "stderr_excerpt": proc.stderr.strip()[:500],
        "saw_fake_workflow": "fake_prompt_injection" in proc.stdout
        or "prompt-injection:untrusted-page-text" in proc.stdout,
        "saw_version": "agent-browser 0.27.1" in proc.stdout,
    }


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]


def main() -> None:
    binary = Path(os.environ.get("AGENT_BROWSER_BINARY", str(DEFAULT_BINARY)))
    if not binary.exists():
        raise SystemExit(f"agent-browser binary not found: {binary}")
    if not SOURCE_WRAPPER.exists():
        raise SystemExit(f"source wrapper not found: {SOURCE_WRAPPER}")

    fixture_mod = load_fixture_module()
    fixture = fixture_mod.load_fixture()
    fixture_mod.FixtureHandler.scenarios_by_path = {
        scenario["path"]: scenario for scenario in fixture["scenarios"]
    }
    server = ThreadingHTTPServer(("127.0.0.1", 0), fixture_mod.FixtureHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()

    tmp = Path("/tmp") / f"sfw-{os.getpid()}"
    if tmp.exists():
        shutil.rmtree(tmp)
    wrapper_dir = tmp / "agent-browser"
    bin_dir = wrapper_dir / "node_modules" / ".bin"
    bin_dir.mkdir(parents=True)
    shutil.copy2(SOURCE_WRAPPER, wrapper_dir / "ab")
    os.chmod(wrapper_dir / "ab", 0o755)
    os.symlink(binary, bin_dir / "agent-browser")

    log_path = tmp / "surfari-actions.jsonl"
    profile_path = tmp / "profile"
    session = f"sfw{os.getpid()}"
    env = {
        **os.environ,
        "HOME": str(tmp / "home"),
        "AGENT_BROWSER_SOCKET_DIR": str(tmp / "sock"),
        "AGENT_BROWSER_SESSION": session,
        "SURFARI_CONTEXT_ID": "eid448-wrapper-fixture",
        "SURFARI_ORG_ID": "eidos",
        "SURFARI_EXPECTED_DOMAINS": "127.0.0.1",
        "SURFARI_ACTION_LOG_PATH": str(log_path),
        "SURFARI_USE_ID": "eid448-wrapper-fixture",
        "SURFARI_BROWSER_PROFILE_PATH": str(profile_path),
    }
    base_url = f"http://127.0.0.1:{server.server_port}"
    commands: list[dict[str, Any]] = []
    try:
        wrapper = wrapper_dir / "ab"
        commands.append(run_wrapper(wrapper, env, ["--version"]))
        commands.append(
            run_wrapper(
                wrapper,
                env,
                ["--profile", str(profile_path), "open", f"{base_url}/fixtures/prompt-injection"],
            )
        )
        commands.append(run_wrapper(wrapper, env, ["snapshot"]))
    finally:
        close_result = run_wrapper(wrapper_dir / "ab", env, ["close", "--all"])
        server.shutdown()
        thread.join(timeout=5)

    action_rows = read_jsonl(log_path)
    payload = {
        "schema_version": 1,
        "captured_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "source_wrapper": str(SOURCE_WRAPPER),
        "temporary_wrapper": str(wrapper_dir / "ab"),
        "binary": str(binary),
        "surface": "temporary clone of Eidos ab wrapper plus built Surfari binary",
        "base_url": base_url,
        "commands": commands,
        "close": close_result,
        "action_log_rows": len(action_rows),
        "checks": {
            "version_routes_to_built_binary": commands[0]["returncode"] == 0
            and commands[0]["saw_version"],
            "wrapper_opened_fake_workflow": commands[1]["returncode"] == 0
            and commands[1]["saw_fake_workflow"],
            "wrapper_snapshotted_fake_workflow": commands[2]["returncode"] == 0
            and commands[2]["saw_fake_workflow"],
            "surfari_action_logs_written": len(action_rows) >= 4,
        },
    }
    payload["passed"] = all(payload["checks"].values())
    OUT.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(
        "wrapper_fixture_probe="
        f"{'pass' if payload['passed'] else 'fail'} "
        f"action_log_rows={payload['action_log_rows']}"
    )
    print(f"wrote {OUT}")
    if not payload["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
