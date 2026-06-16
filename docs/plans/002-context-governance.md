# Context Governance

## Why Now

Surfari records intended context but does not yet enforce it. Governance is the next step that makes Surfari safer than raw browser automation.

## Goal

Gate or reject protected actions when the current command or active tab does not match the intended Surfari context.

## Current State

The shim captures `surfari_context` and `browser_anchor`. Initial governance enforcement now blocks protected actions when Surfari context signals are present and the action has missing, partial, or mismatched domain context. Read-only actions and no-context compatibility mode remain allowed.

## Implementation Plan

1. Define protected actions: form entry, cookies, storage, headers, credential use, and mutation-like clicks. Done in `cli/src/native/surfari/governance.rs`.
2. Compare command URL and active-tab URL against `SURFARI_EXPECTED_DOMAINS`. Done for command `url`/`href` and active-tab URL.
3. Treat unset or partial context as unsafe for protected actions once Surfari context signals are present. Done; plain no-context compatibility mode remains allowed.
4. Return a normal command error before browser auto-launch when a protected action is blocked. Done.
5. Block protected actions on known human-gate domains even when the domain is expected. Done for Apple Sign In domains such as `idmsa.apple.com`.
6. Log the governance decision in the redacted action lifecycle rows. Done for `action_started` and `action_finished`.
7. Next: decide whether some protected actions should use confirmation-required instead of hard block.

## Data/Interface Contract

Add safe governance metadata to logs:

- Decision: allowed or blocked.
- Reason: governance_inactive, read_only_action, no_context, partial_context, domain_mismatch, human_gate_required, or protected_action_allowed.
- Expected domains and observed domain only.
- Human-gate class when one is recognized.

Do not log raw URLs with query strings, page text, or secrets.

## Safety Rules

- Read-only actions should remain compatible.
- Protected actions must not proceed on wrong-domain context.
- Confirmation behavior must use existing confirmation patterns.

## Test Plan

- Wrong-domain protected action is blocked.
- Partial-context protected action is blocked.
- Matching-domain protected action proceeds.
- Read-only actions proceed without new failures.
- Logs include governance metadata and no secrets.

## Proof Artifacts

Attach to EID-397:

- Rust test output.
- Smoke action log path.
- Candidate path if candidates are produced.
- Secret absence checks.

## Linear Links

- EID-397: https://linear.app/eidos-agi/issue/EID-397/enforce-surfari-context-governance

## Done Means

Surfari prevents or gates protected actions outside the active context without breaking safe read-only browsing.
