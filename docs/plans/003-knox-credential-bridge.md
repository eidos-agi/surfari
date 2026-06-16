# Knox Credential Bridge

## Why Now

Surfari will eventually need credentials, but credentials must not become chat, log, repo, Linear, or learning data.

## Goal

Use Knox references for credential-needed workflows with explicit approval gates and complete redaction.

## Current State

Surfari records `SURFARI_KNOX_REF` as context metadata only. It does not call Knox or retrieve secrets.

## Implementation Plan

1. Treat `SURFARI_KNOX_REF` and command-level credential refs as references only.
2. Add approval-gated retrieval through Knox for credential-needed actions.
3. Pass retrieved values only to the immediate browser action.
4. Record only credential refs, retrieval status, and redacted result metadata.
5. Preserve MFA, CAPTCHA, and user-consent blockers.

## Data/Interface Contract

Logs may include:

- `knox_ref`.
- retrieval status.
- approval status.
- credential use category.

Logs must not include the retrieved secret, derived token, raw input value, or credential-bearing error.

## Safety Rules

- No Knox retrieval without approval.
- No bypass of MFA, CAPTCHA, or explicit user approval.
- Missing or denied credentials fail closed.

## Test Plan

- Approved retrieval works and redacts the secret.
- Denied retrieval fails closed.
- Missing secret fails closed.
- Wrong ref fails closed.
- Seeded fake secrets are absent from logs, candidates, errors, and debug output.

## Proof Artifacts

Attach to EID-398:

- Test pass counts.
- Action log path.
- Candidate path.
- Secret absence checks.
- Knox-denied and Knox-missing outcomes.

## Linear Links

- EID-398: https://linear.app/eidos-agi/issue/EID-398/add-knox-credential-bridge

## Done Means

Surfari can use credential references safely without storing or exposing raw credentials.
