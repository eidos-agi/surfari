# Converge Control Plane

Converge is the control plane for recursive, evidence-backed improvement loops
in Surfari. It turns product goals into ordered target matrices, deterministic
probes, repair queues, research loops, and Linear work.

The active Linear control issue is `EID-449`. The first seed case is `EID-448`,
the Surfari/Knox Apple Developer profile-session workflow.

## Current Seed

`docs/converge/eid-448/` contains the first seed:

- a 50 test by 50 capability matrix chain
- proof envelopes for observed rows
- generated Converge target/probe rows
- generated repair queue
- generated recursive research loops
- current-state evidence heatmap
- front-loaded approval packet for the Apple Developer Knox profile resume

Regenerate the seed from repo-root with:

```bash
python3 scripts/run-eid448-converge.py
```

If the repo-pinned pnpm version is available, `pnpm converge:eid448` runs the
same command. The direct Python command is the lowest-friction local proof path.
It writes `docs/converge/eid-448/score_report.md` as the current handoff
summary.

Focused probes are also available:

```bash
python3 scripts/probe-eid448-fake-workflows.py
python3 scripts/probe-eid448-browser-fixtures.py
python3 scripts/eid448-linear-sync-candidates.py
```

## Operating Contract

- Planned coverage is not proof.
- A passing row needs evidence and a proof envelope.
- Controlled harness passes do not silently count as real-surface proof.
- Account-sensitive flows stop at login, MFA, passkeys, legal agreements,
  payments, and final submissions.
- Human approvals should be front-loaded into explicit approval packets where
  possible.
- Stable repair rows can become Linear work; unstable rows stay in the local
  research loop until their target and probe are clear.

## Project Gate

There is no Linear project named `Converge` yet. The current Linear connector
could create the control EID, document, comments, and label, but did not expose
a project creation mutation. Until the Linear project shell exists, use the
`Converge` label plus `EID-449` as the control surface.
