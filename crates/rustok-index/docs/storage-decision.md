# Index PostgreSQL storage decision

The storage benchmark comparison is evidence, not an automatic model selector. After replacement `100k` and `1m` packets have been generated from the same commit and the comparator reports `decision_ready: true`, maintainers record a manual decision and finalize the ADR.

## Tooling entrypoint

Use the stable command router instead of remembering individual script names:

```bash
# Static repository contracts; does not execute PostgreSQL benchmarks.
node scripts/verify/index-storage-tooling.mjs contract

# Comparator, decision, and ADR fixture suites.
node scripts/verify/index-storage-tooling.mjs fixtures

# Validate an already generated packet.
node scripts/verify/index-storage-tooling.mjs packet \
  --scale 100k \
  --root evidence/index-storage/100k

# Generate a same-commit cross-scale comparison.
node scripts/verify/index-storage-tooling.mjs compare \
  --input evidence/index-storage/100k \
  --input evidence/index-storage/1m \
  --output evidence/index-storage/comparison
```

The router dispatches Node directly without shell evaluation. It exposes the canonical static guards, packet validator, comparator, exact-byte hashing, decision preparation, ADR finalization, and saved-ADR verification paths.

## Prepare the decision

Create a draft from the exact `comparison.json` that will be reviewed:

```bash
node scripts/verify/index-storage-tooling.mjs prepare \
  --comparison evidence/index-storage/comparison/comparison.json \
  --selected typed_eav \
  --owner "Index maintainers" \
  --date 2026-07-24 \
  --output evidence/index-storage/comparison/decision.json
```

`prepare` requires an explicit prototype choice. It does not rank candidates or select a winner. It validates the decision-ready comparison, copies the evidence commit, computes the SHA-256 of the exact comparison-file bytes, creates rejection entries for exactly the two unselected prototypes, and refuses to overwrite an existing decision unless `--force` is provided. The draft is written to a staged file and renamed only after the complete JSON is on disk.

The generated draft contains `TODO(index-storage-decision):` markers. Replace every marker with measured and operational reasoning before finalization. The finalizer rejects any remaining preparation marker.

[`storage-decision.example.json`](storage-decision.example.json) shows the same decision fields and references [`storage-decision.schema.json`](storage-decision.schema.json). Its relative `$schema` is valid because the two files are colocated in the documentation directory. A generated decision under `evidence/index-storage/...` intentionally omits `$schema` rather than recording a false relative path; `$schema` remains an optional finalizer field when it correctly points to a colocated schema file.

The example is intentionally not finalizable until its markers are replaced. The decision must explain:

- why the selected prototype is preferred;
- why each of the other two prototypes was rejected;
- operational trade-offs;
- migration strategy;
- rollback strategy.

`selected_prototype` must be one of `jsonb`, `typed_eav`, or `hot_projection`. `comparison_commit` must match the full Git commit recorded by both scale packets, and `comparison_sha256` must match the exact bytes of the reviewed `comparison.json`.

For an independent digest check:

```bash
node scripts/verify/index-storage-tooling.mjs hash \
  evidence/index-storage/comparison/comparison.json
```

## Finalize the ADR

```bash
node scripts/verify/index-storage-tooling.mjs render \
  --comparison evidence/index-storage/comparison/comparison.json \
  --decision evidence/index-storage/comparison/decision.json \
  --output crates/rustok-index/docs/adr-postgresql-storage.md
```

Finalization snapshots the exact comparison and decision bytes before rendering. The generated ADR records both `Comparison SHA-256` and `Decision SHA-256`, so reviewers can verify the two source documents used to produce it.

The finalizer fails closed unless:

- the comparison is decision-ready;
- every decision-contract flag is true;
- `100k` and `1m` evidence are present and share the same full commit;
- automatic winner selection is explicitly disabled;
- every displayed metric and cross-scale ratio is present and numeric;
- the decision identifies the same comparison commit;
- `comparison_sha256` matches the exact comparison-file bytes;
- no preparation placeholder remains;
- selection, rejection, operations, migration, and rollback rationales are all present.

## Verify the saved ADR

After saving or reviewing the generated Markdown, verify that it still represents the exact source files:

```bash
node scripts/verify/index-storage-tooling.mjs verify-adr \
  --comparison evidence/index-storage/comparison/comparison.json \
  --decision evidence/index-storage/comparison/decision.json \
  --adr crates/rustok-index/docs/adr-postgresql-storage.md
```

`verify-adr` recalculates both digest lines from exact file bytes, snapshots the same comparison and decision bytes, repeats deterministic finalization, and requires the saved ADR to match the regenerated Markdown byte for byte. Any manual edit, formatting change, stale decision, or replaced evidence file is rejected.

The generated ADR includes storage size, read latency, mutation latency, WAL, churn, and VACUUM evidence for all candidates. It never infers or ranks a winner. Its Markdown depends on evidence and decision content, not on the filesystem paths used to invoke the tooling.

## Validation boundary

The tooling router, ADR finalizer, and saved-ADR verifier do not replace benchmark execution, evidence-packet validation, production migration rehearsal, or production observability. They expose the existing contracts consistently and turn an already validated comparison plus an explicit human decision into a reviewable, byte-bound document.
