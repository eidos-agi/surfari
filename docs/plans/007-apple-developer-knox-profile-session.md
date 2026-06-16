# Apple Developer Knox Profile Session

## Why Now

The Eidos Knox Apple Developer signing incident is the first concrete product
eval for Surfari as a governed browser layer. The workflow exposed active-tab
drift, stale React refs, browser profile ambiguity, and Apple Sign In human
gates that raw `agent-browser` does not model.

## Goal

Make Surfari usable for the next safe resume of the Eidos Knox provisioning
profile workflow without automating Apple login, MFA, passkeys, legal
agreements, payments, or final submissions.

## Current State

The fork is an Eidos copy of `vercel-labs/agent-browser` with a local learning
shim on `codex/surfari-learning-shim`. The branch records redacted action logs,
learning candidates, Surfari context, browser anchors, and governance decisions.

The Apple Developer incident state is:

- Product: `Eidos Knox`.
- Local target: `KnoxiPhone`.
- Bundle id: `ai.eidos.knox.iphone`.
- Team ID: `LJWV44N8BF`.
- Expected domains: `developer.apple.com,idmsa.apple.com`.
- Server-side profile row: `Eidos Knox App Store Connect 2026-06-14`.
- Local profile state: missing until the human clears Apple Sign In and the
  profile is downloaded.

## Online Definition

Recommended definition for the new Surfari to be online:

1. Local online: this fork builds, passes the Surfari guard tests, and the local
   `ab` or Surfari wrapper can invoke the forked binary for governed sessions.
2. Knox workflow online: a metadata-only Apple Developer playbook or harness can
   answer whether the active browser surface is on the intended profile,
   Surfari session, web-account session, and expected domain before protected
   actions.
3. Team online: the learning shim and Knox profile-session slice are merged to
   `origin/main` or an approved Eidos branch with Linear proof attached.
4. Published online: npm, Homebrew, hosted docs, hosted dashboard, or package
   rename only after package ownership, credentials, and release approval are
   available.

Hosted docs, hosted dashboard, and package publishing are not required to unblock
the Knox signing incident. A local build plus wrapper routing is the immediate
usable surface.

## Implementation Plan

1. Preserve the current learning shim baseline and keep upstream sync separate.
2. Add Apple Sign In human-gate governance for `idmsa.apple.com` and
   `appleid.apple.com`.
3. Add a deterministic eval fixture for active-tab drift where a non-Apple tab
   steals focus before a protected action.
4. Add a deterministic eval fixture for stale React refs after a safe page
   re-render.
5. Add a deterministic eval fixture for authenticated download handoff where the
   server profile exists but the local profile is missing.
6. Route the incident wrapper inputs into Surfari context envs:
   `SURFARI_CONTEXT_ID`, `SURFARI_PROFILE_ID`, `SURFARI_EXPECTED_DOMAINS`, and
   `SURFARI_KNOX_REF`.
7. Build and link the fork locally, then update the wrapper only after the binary
   proof is clean.

## Data/Interface Contract

The Apple Developer workflow may log:

- Product, target, bundle id, Team ID, expected domains, and profile row name.
- Knox credential reference only.
- Active tab id, active target id, sanitized active URL, and title hash.
- Human-gate class such as `apple_sign_in`.
- Download handoff state: server profile exists, local profile missing,
  downloaded, installed, or blocked.

The workflow must not log raw Apple passwords, MFA values, passkey material, App
Store Connect API keys, private keys, provisioning profile contents, cookies,
headers, payment data, or legal agreement content.

## Safety Rules

- Stop on `idmsa.apple.com` for protected actions and report
  `human_gate_required`.
- Allow read-only observation on human-gate domains for proof and diagnosis.
- Prefer DOM-confirmed actions over stale `@eNN` refs after React re-renders.
- Treat the local `apple-dev-guard` as incident evidence until its checks are
  moved into Surfari.
- Knox coupling stays as references and requests only.

## Test Plan

```bash
cd /Users/dshanklin/repos-eidos-agi/surfari/cli
cargo test surfari -- --test-threads=1
cargo test native::surfari::governance -- --test-threads=1
cargo fmt -- --check
```

Later eval proof should cover:

- Wrong active tab blocks protected action.
- Stale ref recovery recommends DOM-confirmed action.
- `idmsa.apple.com` blocks protected action as `human_gate_required`.
- Profile download handoff records metadata only.

## Proof Artifacts

Attach to EID-448:

- Git remotes, branch, dirty state, and upstream position.
- Test command outputs and pass counts.
- Local binary path or wrapper path once built and linked.
- Any blocker for pnpm, publishing, hosted docs, dashboard, credentials, or
  release ownership.

## Linear Links

- EID-448: https://linear.app/eidos-agi/issue/EID-448/teach-surfari-profilesession-workflows-from-the-eidos-knox-apple
- EID-447: https://linear.app/eidos-agi/issue/EID-447/verify-apple-developer-activation-and-clear-eidos-knox-signing
- EID-397: https://linear.app/eidos-agi/issue/EID-397/enforce-surfari-context-governance
- EID-398: https://linear.app/eidos-agi/issue/EID-398/add-knox-credential-bridge
- EID-400: https://linear.app/eidos-agi/issue/EID-400/build-surfari-eval-harness

## Done Means

The local fork can be built and routed through Surfari for the Knox Apple
Developer resume flow, protected actions fail closed at Apple Sign In, and the
remaining Apple login/MFA/profile-download gate is documented with metadata-only
proof.
