# Surfari Plans

Surfari planning uses two durable surfaces:

- Repo docs in this folder define the technical contract.
- Linear tracks execution state, proof, and blockers.

Use the existing Linear Surfari project:

- Surfari project: https://linear.app/eidos-agi/project/surfari-9c25460dc14b

## Plan Template

Each plan should use this shape:

```md
# Title

## Why Now
## Goal
## Current State
## Implementation Plan
## Data/Interface Contract
## Safety Rules
## Test Plan
## Proof Artifacts
## Linear Links
## Done Means
```

## Operating Rules

- Repo docs define technical truth.
- Linear issues track execution state and proof.
- No secrets or raw credentials in docs, Linear, logs, or comments.
- Use `password_path`, `knox_ref`, credential refs, and source artifact refs only.
- Every milestone closes with tests, command output, log paths, artifact paths, or explicit blockers.

## Baseline Proof Commands

```bash
cd /Users/dshanklin/repos-eidos-agi/surfari/cli
cargo test surfari -- --test-threads=1
cargo fmt -- --check
cargo clippy
cargo test
```
