# Shipping Gate

## Why Now

Surfari should ship only after the product is proven safer, more repeatable, and more useful than raw agent-browser for governed browsing.

## Goal

Define release readiness criteria and proof packaging.

## Current State

Surfari has a learning shim baseline and a milestone roadmap. It still needs governance enforcement, Knox credential bridging, learning distillation, and eval harness proof before shipping.

## Implementation Plan

1. Require all core milestone issues to close with proof.
2. Verify CLI/help/docs parity.
3. Verify install path and plugin skill behavior.
4. Package proof artifacts and release notes.

## Data/Interface Contract

Shipping proof must include:

- Test command results.
- Eval harness summary.
- Redaction proof.
- Compatibility proof.
- Install/doctor proof.
- Linear issue links.

## Safety Rules

- No release if seeded fake secrets leak.
- No release if protected actions can run in wrong context.
- No release if Knox credential handling logs raw secrets.

## Test Plan

- Baseline Rust proof passes.
- Governance tests pass.
- Knox bridge tests pass.
- Learning distillation tests pass.
- Eval harness passes.
- Install/docs/help checks pass.

## Proof Artifacts

Attach to EID-401:

- Final proof bundle path.
- All milestone links.
- Test pass counts.
- Eval summary.
- Release blockers, if any.

## Linear Links

- EID-401: https://linear.app/eidos-agi/issue/EID-401/define-surfari-shipping-gate

## Done Means

Surfari has a clear release gate and proof bundle ready for product review.
