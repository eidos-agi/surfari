# Learning Distillation

## Why Now

Surfari records learning candidates, but the product value comes from turning those candidates into safe reusable playbooks that improve later browsing.

## Goal

Distill redacted learning candidates into per-domain and per-context guidance that can be injected later.

## Current State

Learning candidates include action outcome, command metadata, result shape, `browser_anchor`, and `surfari_context`. They do not yet produce playbooks or active guidance.

## Implementation Plan

1. Read learning candidates from the active use/session.
2. Group candidates by domain and Surfari context.
3. Extract safe selectors, refs, blockers, success/failure patterns, and screenshot paths.
4. Write playbook artifacts that contain only safe metadata.
5. Retrieve playbooks only when context and domain match.

## Data/Interface Contract

Playbooks may include:

- Domain and context ids.
- Safe selectors and refs.
- Known blocker types.
- Result shapes.
- Safe screenshot/artifact paths.

Playbooks must not include raw page text, credentials, cookies, headers, storage values, or raw screenshots.

## Safety Rules

- Mismatched context must not retrieve another context's playbook.
- Playbooks are advisory until governance and evals prove safe injection.

## Test Plan

- Candidate rows distill into a safe playbook.
- Repeat workflow retrieves matching playbook guidance.
- Mismatched context retrieves nothing.
- Second run reduces failed or redundant actions in a fixture.
- Seeded fake secrets are absent.

## Proof Artifacts

Attach to EID-399:

- Candidate log path.
- Playbook artifact path.
- Repeat-run action counts.
- Failure reduction evidence.
- Secret absence checks.

## Linear Links

- EID-399: https://linear.app/eidos-agi/issue/EID-399/distill-learning-candidates-into-playbooks

## Done Means

Surfari can turn redacted action experience into context-safe reusable browsing guidance.
