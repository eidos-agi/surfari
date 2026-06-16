# Eval Harness

## Why Now

Surfari needs product-grade proof across realistic browser tasks, not only unit tests.

## Goal

Build deterministic local fixtures that prove Surfari is safer, more context-aware, and faster on repeat workflows.

## Current State

The Rust test suite proves the shim contract. There is no dedicated Surfari product harness for multi-org browsing, wrong accounts, credentials, prompt injection, or learning improvement.

## Implementation Plan

1. Add local fake apps for public browsing, logged-in SaaS, multi-org state, wrong-account state, credential-needed flows, prompt injection, repeat workflows, and redaction leaks.
2. Run Surfari commands against those apps with isolated sessions and context envs.
3. Capture action logs, learning candidates, screenshots/artifacts when safe, and summary metrics.
4. Compare first-run and second-run action counts for learning improvement.

## Data/Interface Contract

Harness output must include:

- Fixture name.
- Context id.
- Expected outcome.
- Actual outcome.
- Action log path.
- Candidate path.
- Secret absence result.
- Repeat-run metrics when applicable.

## Safety Rules

- Use fake credentials and fake accounts only.
- No real portals, real credentials, or real account mutations.
- Prompt injection fixtures must be treated as hostile page content.

## Test Plan

- Public browsing fixture succeeds.
- Multi-org correct-account fixture succeeds.
- Wrong-account fixture blocks or flags.
- Credential-needed fixture uses only fake secrets.
- Prompt-injection fixture does not override governance.
- Repeat workflow improves on second run.
- Redaction leak fixture proves fake secrets absent.

## Proof Artifacts

Attach to EID-400:

- Harness output summary.
- Action log and candidate paths.
- Fixture pass/fail counts.
- Secret absence results.
- Repeat-run metrics.

## Linear Links

- EID-400: https://linear.app/eidos-agi/issue/EID-400/build-surfari-eval-harness

## Done Means

The harness can produce deterministic local proof that Surfari is safer and more useful than raw browser automation.
