# EID-448 Converge Score Report

Generated at: `2026-06-16T15:32:06+00:00`

## Current Score

- Matrix shape: `50x50`
- Planned coverage cells: `400`
- Planned weighted depth: `936`
- Evidence rows: `50`
- Row state: `47 pass, 0 fail, 3 blocked, 0 skip`
- Repair queue: `3`
- Research loops: `3`

## Stage Gates

| Stage | Proof class | Approval | Rows | State | Next blocking rows |
|---|---|---:|---:|---|---|
| M1 Repo and Build Matrix | `pass_real_surface` | no | 14 | `pass` | none |
| M2 Governance Logic Matrix | `pass_controlled_harness` | no | 10 | `pass` | none |
| M3 Daemon and Logs Matrix | `pass_controlled_harness` | no | 7 | `pass` | none |
| M4 Fake Browser Workflow Matrix | `pass_controlled_harness` | no | 13 | `pass` | none |
| M5 Real Knox Apple Dry Run Matrix | `pass_real_surface` | yes | 6 | `blocked` | EID448-T048, EID448-T049, EID448-T050 |

## Highest Priority Repairs

| Target | Class | Gap | Priority | Owner | Next action |
|---|---|---|---:|---|---|
| EID448-T048 | `blocked` | `policy_gap` | 15 | human operator | Capture metadata-only human confirmation that the visible team matches LJWV44N8BF without storing credentials or secrets. |
| EID448-T049 | `blocked` | `policy_gap` | 15 | human operator | After human approval, record only handoff metadata for profile download/install; never store provisioning profile contents. |
| EID448-T050 | `blocked` | `policy_gap` | 15 | human approval plus coding agent | Review generated Converge rows, action logs, approval packet, and Linear proof after the approved dry run. |

## Generated Artifacts

- `docs/plans/eid-448-test-coverage-matrix.json`
- `docs/plans/eid-448-test-coverage-heatmap.svg`
- `docs/converge/eid-448/converge_rows.jsonl`
- `docs/converge/eid-448/repair_queue.json`
- `docs/converge/eid-448/research_loops.jsonl`
- `docs/converge/eid-448/stage_summary.json`
- `docs/converge/eid-448/evidence_heatmap.svg`
- `docs/converge/eid-448/evidence/fake_workflow_probe.json`
- `docs/converge/eid-448/evidence/browser_fixture_probe.json`
- `docs/converge/eid-448/evidence/wrapper_fixture_probe.json`
- `docs/converge/eid-448/linear_sync_candidates.json`

## Linear

- Seed issue: `EID-448`
- Control issue: `EID-449`
- Temporary query label: `Converge`
