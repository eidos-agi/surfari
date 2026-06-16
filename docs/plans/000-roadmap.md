# Surfari Roadmap

## Why Now

Surfari is becoming a governed learning browser fork instead of a thin wrapper around `agent-browser`. The map needs to preserve the current learning shim and sequence the safety work without mixing product boundaries.

## Goal

Create a milestone map that makes Surfari shippable: safer than raw browser automation, faster on repeated work, compatible with agent-browser, and backed by deterministic proof.

## Current State

The current branch has uncommitted Surfari shim work:

- Redacted daemon-side action lifecycle logs.
- Learning candidates.
- Browser/session/tab anchors.
- Structured Surfari context envelope.

Linear baseline and related Surfari work:

- EID-394: browser anchors in action logs.
- EID-395: structured context envelope.
- EID-172: later public Surfari case study, not part of the core shipping setup.

## Implementation Plan

1. Finalize the current Surfari learning shim baseline.
2. Enforce Surfari context governance.
3. Add Knox credential bridge.
4. Distill learning candidates into playbooks.
5. Build the Surfari eval harness.
6. Define the Surfari shipping gate.

## Data/Interface Contract

Surfari plans must preserve the current log substrate:

- `action_started` and `action_finished` rows.
- `learning_candidate` rows.
- `browser_anchor`.
- `surfari_context`.
- Redacted command/result summaries only.

Linear references must point to Surfari project issues or the later Surfari case-study issue.

## Safety Rules

- No raw credentials, tokens, cookies, headers, MFA values, or storage values.
- No raw page text in logs or candidates.
- No real account mutation in eval harnesses.
- Knox work uses references and explicit approval gates.

## Test Plan

Every milestone starts with the baseline proof commands and adds milestone-specific tests for governance, credential handling, learning, evals, or release readiness.

## Proof Artifacts

Proof comments in Linear must include:

- Command results and pass counts.
- Action log and learning candidate paths.
- Seeded fake-secret absence checks.
- Plan doc path.
- Explicit blockers when proof cannot be completed.

## Linear Links

- Surfari project: https://linear.app/eidos-agi/project/surfari-9c25460dc14b
- EID-396: https://linear.app/eidos-agi/issue/EID-396/finalize-surfari-learning-shim-baseline
- EID-397: https://linear.app/eidos-agi/issue/EID-397/enforce-surfari-context-governance
- EID-398: https://linear.app/eidos-agi/issue/EID-398/add-knox-credential-bridge
- EID-399: https://linear.app/eidos-agi/issue/EID-399/distill-learning-candidates-into-playbooks
- EID-400: https://linear.app/eidos-agi/issue/EID-400/build-surfari-eval-harness
- EID-401: https://linear.app/eidos-agi/issue/EID-401/define-surfari-shipping-gate

## Done Means

The roadmap exists in repo docs, the milestone issues exist in Linear, and baseline proof is attached to EID-396.
