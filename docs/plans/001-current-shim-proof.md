# Current Shim Proof

## Why Now

The branch already contains the core Surfari learning shim. Before adding governance or credential features, preserve and prove this baseline.

## Goal

Document and verify the current daemon-side Surfari substrate: action lifecycle logs, redaction, learning candidates, browser anchors, and structured context.

## Current State

The Surfari fork records daemon-side events through `execute_command`. Normal command output is unchanged.

Implemented baseline:

- `action_started` and `action_finished` rows.
- Append-only JSONL action logs.
- Learning candidates.
- Redaction of sensitive commands/results.
- Browser anchor metadata.
- Structured Surfari context metadata.

## Implementation Plan

1. Run the baseline Rust proof commands.
2. Run an isolated binary smoke with temporary `HOME`, action log path, use id, and Surfari context envs.
3. Assert seeded fake secrets are absent from action logs and candidates.
4. Attach proof to EID-396.
5. Keep this plan updated if the baseline contract changes.

## Data/Interface Contract

Action logs and candidates may include safe metadata only:

- Action id, command id, action, session, use id, cwd, duration, success.
- Redacted command metadata.
- Result shape and error metadata.
- `browser_anchor`.
- `surfari_context`.

They must not include raw page text, screenshots, cookies, headers, storage values, passwords, MFA values, tokens, or profile paths.

## Safety Rules

- Logs are best-effort and must not break command execution.
- Secret absence is proven with seeded fake strings.
- Raw profile paths are recorded only as byte length and SHA-256 hash.

## Test Plan

```bash
cd /Users/dshanklin/repos-eidos-agi/surfari/cli
cargo test surfari -- --test-threads=1
cargo fmt -- --check
cargo clippy
cargo test
```

Binary smoke:

- Set isolated `HOME`.
- Set `SURFARI_ACTION_LOG_PATH`.
- Set `SURFARI_USE_ID`.
- Set context envs including `SURFARI_EXPECTED_DOMAINS` and `SURFARI_BROWSER_PROFILE_PATH`.
- Run non-browser commands like `stream status` and `close`.
- Verify lifecycle rows, candidates, context, anchors, and secret absence.

## Proof Artifacts

Attach to EID-396:

- Test pass counts.
- Action log path.
- Learning candidate path.
- Smoke summary JSON.
- Secret absence results.

## Linear Links

- EID-396: https://linear.app/eidos-agi/issue/EID-396/finalize-surfari-learning-shim-baseline
- EID-394: https://linear.app/eidos-agi/issue/EID-394/add-surfari-browser-anchors-to-action-logs
- EID-395: https://linear.app/eidos-agi/issue/EID-395/add-structured-surfari-context-envelope-to-action-logs

## Done Means

EID-396 has proof attached and the current shim baseline is ready to preserve before new feature work.
