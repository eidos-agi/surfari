#!/usr/bin/env python3
"""Probe the controlled fake workflow fixture contract for EID-448."""

from __future__ import annotations

import json
import threading
import urllib.error
import urllib.request
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "docs" / "converge" / "eid-448"
FIXTURE = BASE / "fixtures" / "fake_browser_workflows.json"
OUT = BASE / "evidence" / "fake_workflow_probe.json"


def load_fixture() -> dict[str, Any]:
    return json.loads(FIXTURE.read_text(encoding="utf-8"))


def body_for(scenario: dict[str, Any]) -> bytes:
    marker = scenario["expected_marker"]
    html = f"""<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>{scenario['id']}</title></head>
<body data-fixture="{scenario['id']}" data-marker="{marker}">
  <main>
    <h1>{scenario['id']}</h1>
    <p>{marker}</p>
    <button id="protected-action">Protected action target</button>
    <a id="download-profile" href="/developer/account/resources/profiles/download">Download metadata handoff</a>
  </main>
</body>
</html>
"""
    return html.encode("utf-8")


class FixtureHandler(BaseHTTPRequestHandler):
    scenarios_by_path: dict[str, dict[str, Any]] = {}

    def do_GET(self) -> None:  # noqa: N802 - stdlib handler API
        scenario = self.scenarios_by_path.get(self.path)
        if not scenario:
            self.send_response(404)
            self.end_headers()
            self.wfile.write(b"fixture:not-found")
            return

        status = int(scenario["status"])
        self.send_response(status)
        if status in {301, 302, 303, 307, 308}:
            self.send_header("Location", scenario["location"])
            self.send_header("X-Surfari-Fixture", scenario["id"])
            self.end_headers()
            self.wfile.write(scenario["expected_marker"].encode("utf-8"))
            return

        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("X-Surfari-Fixture", scenario["id"])
        self.end_headers()
        self.wfile.write(body_for(scenario))

    def log_message(self, _format: str, *_args: object) -> None:
        return


def request_without_redirect(url: str) -> tuple[int, dict[str, str], str]:
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, req, fp, code, msg, headers, newurl):  # type: ignore[no-untyped-def]
            return None

    opener = urllib.request.build_opener(NoRedirect)
    try:
        with opener.open(url, timeout=5) as response:
            body = response.read().decode("utf-8")
            return response.status, dict(response.headers.items()), body
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8")
        return exc.code, dict(exc.headers.items()), body


def main() -> None:
    fixture = load_fixture()
    scenarios = fixture["scenarios"]
    FixtureHandler.scenarios_by_path = {scenario["path"]: scenario for scenario in scenarios}

    server = ThreadingHTTPServer(("127.0.0.1", 0), FixtureHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()

    base_url = f"http://127.0.0.1:{server.server_port}"
    results = []
    try:
        for scenario in scenarios:
            status, headers, body = request_without_redirect(base_url + scenario["path"])
            marker_found = scenario["expected_marker"] in body
            location_ok = True
            if "location" in scenario:
                location_ok = headers.get("Location") == scenario["location"]
            results.append(
                {
                    "id": scenario["id"],
                    "matrix_rows": scenario["matrix_rows"],
                    "url": base_url + scenario["path"],
                    "expected_status": scenario["status"],
                    "status": status,
                    "expected_marker": scenario["expected_marker"],
                    "marker_found": marker_found,
                    "location_ok": location_ok,
                    "class": "pass" if status == scenario["status"] and marker_found and location_ok else "fail",
                }
            )
    finally:
        server.shutdown()
        thread.join(timeout=5)

    payload = {
        "schema_version": 1,
        "captured_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "fixture": str(FIXTURE.relative_to(ROOT)),
        "surface": "local ThreadingHTTPServer",
        "base_url": base_url,
        "results": results,
        "passed": sum(1 for result in results if result["class"] == "pass"),
        "failed": sum(1 for result in results if result["class"] == "fail"),
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(f"fake_workflow_results={payload['passed']} pass {payload['failed']} fail")
    print(f"wrote {OUT}")
    if payload["failed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
