# Index PostgreSQL storage decision

The storage benchmark comparison is evidence, not an automatic model selector. After replacement `100k` and `1m` packets have been generated from the same commit and the comparator reports `decision_ready: true`, maintainers record the decision in a separate JSON file and render the ADR.

## Decision input

Start from [`storage-decision.example.json`](storage-decision.example.json). It references [`storage-decision.schema.json`](storage-decision.schema.json), so editors and external validation environments can check the same structural contract used by the renderer.

```json
{
  "$schema": "./storage-decision.schema.json",
  "status": "proposed",
  "decision_date": "2026-07-24",
  "owner": "Index maintainers",
  "comparison_commit": "0123456789abcdef0123456789abcdef01234567",
  "selected_prototype": "typed_eav",
  "selection_rationale": "Explain why this model is preferred using measured and operational evidence.",
  "rejection_rationales": {
    "jsonb": "Explain why JSONB was not selected.",
    "hot_projection": "Explain why hot projection was not selected."
  },
  "operational_tradeoffs": "Document indexing, schema evolution, relation growth, WAL, VACUUM and observability implications.",
  "migration_strategy": "Document table creation, backfill, verification and persistence-port cutover.",
  "rollback_strategy": "Document how the previous persistence path remains recoverable during cutover."
}
```

`selected_prototype` must be one of:

- `jsonb`
- `typed_eav`
- `hot_projection`

`rejection_rationales` must contain exactly the other two prototypes. The `comparison_commit` must match the full Git commit recorded by both scale packets.

## Render the ADR

```bash
node scripts/verify/render-index-storage-adr.mjs \
  --comparison evidence/index-storage/comparison/comparison.json \
  --decision evidence/index-storage/comparison/decision.json \
  --output crates/rustok-index/docs/adr-postgresql-storage.md
```

The renderer fails closed unless:

- the comparison is decision-ready;
- every decision-contract flag is true;
- `100k` and `1m` evidence are present and share the same full commit;
- automatic winner selection is explicitly disabled;
- every displayed metric and cross-scale ratio is present and numeric;
- the decision identifies the same comparison commit;
- selection, rejection, operations, migration and rollback rationales are all present.

The generated ADR includes storage size, read latency, mutation latency, WAL, churn and VACUUM evidence for all candidates. It never infers or ranks a winner. Its Markdown depends on evidence and decision content, not on the filesystem path used to invoke the renderer.

## Validation boundary

The ADR renderer does not replace benchmark execution, evidence-packet validation, production migration rehearsal or production observability. It only turns an already validated comparison plus an explicit human decision into a reviewable document.
