# Learning Distillation

## Why Now

Surfari records learning candidates, but the product value comes from turning those candidates into safe reusable playbooks that improve later browsing.

## Goal

Distill redacted learning candidates into per-domain and per-context guidance that can be injected later.

## Current State

Learning candidates include action outcome, command metadata, result shape, `browser_anchor`, and `surfari_context`.

The runtime foundation now supports a versioned external rule store, exact URL-base retrieval, allowlisted tag matching, deterministic safe-token similarity, and redacted retrieval receipts. Rules are reloaded for each failed action, so approved corrections do not require a binary rebuild or daemon restart.

## Implementation Plan

1. Read learning candidates from the active use/session.
2. Group candidates by domain, approved tags, and Surfari context.
3. Extract safe selectors, refs, blockers, success/failure classifications, and screenshot paths.
4. Write proposed playbook artifacts that contain only safe metadata.
5. Require an explicit proposed -> approved/rejected transition.
6. Retrieve approved playbooks by URL base, then tags, then deterministic safe-token similarity.
7. Record a redacted retrieval receipt with rule id/version and match method.

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
- Only allowlisted classifications may enter tags or semantic terms; raw error tokens are never indexed.
- Corrupt, oversized, unknown-schema, or unapproved stores fail closed without breaking browser execution.
- The action path makes no model call.

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
