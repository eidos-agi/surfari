# EID-448 Converge Praxis Forge

This folder is the first operational Converge/Praxis slice for Surfari.

It turns the planned 50x50 EID-448 matrix into evidence-backed rows, repair
queues, recursive research loops, approval packets, and a current-state
heatmap. The planned heatmap says what the proof ladder should eventually cover.
The evidence heatmap says what is actually proven, failed, or blocked now.

## Artifacts

- `telos.md`: purpose, success conditions, failure conditions, anti-goals.
- `matrix_chain.json`: staged matrices, unlock rules, proof classes, stop
  conditions.
- `evidence/probes.json`: captured evidence and proof envelopes for rows that
  have been observed.
- `approval_packets/`: front-loaded human approval envelopes.
- `converge_rows.jsonl`: generated target/probe/delta/class/evidence rows.
- `repair_queue.json`: generated next-action queue for fail/blocked rows.
- `research_loops.jsonl`: generated recursive study loops for unclear or policy
  gaps.
- `stage_summary.json`: generated per-matrix pass/fail/block summary.
- `evidence_heatmap.svg`: generated current-state heatmap.

## Regenerate

```bash
python3 scripts/run-eid448-converge.py
```

That command rebuilds the planned matrix, planned heatmap, evidence rows,
repair queue, research loops, stage summary, evidence heatmap, and
`score_report.md`. It also refreshes the local fake-workflow probe and Linear
sync candidates. `pnpm converge:eid448` runs the same command when pnpm 11+ is
available.

## Operating Rule

Problems are recursively studied. A failing row is classified before repair:

- known cause: Docket repair item
- unknown cause: Research loop
- authority boundary: Governor policy gap
- stale target/proof: Praxis drift
- independent governance scope: child eidos candidate

Do not promote planned coverage to proof. A row passes only when its Converge
row has evidence and a proof envelope.
