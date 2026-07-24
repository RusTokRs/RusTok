# Index storage evidence comparison

## Purpose

This document defines the deterministic comparison step between the replacement
`100k` and `1m` PostgreSQL evidence packets. It does not select a storage model;
the accepted decision remains an explicit ADR after both scale packets have been
inspected.

The replacement packets must come from one commit that includes the complete
module/entity/schema-version identity corrections for typed EAV field rows and
for JSONB/EAV maintenance mutations. Earlier packets remain historical
diagnostics and are not decision-ready inputs.

## Inputs

Each input directory must contain:

- `read-report.json`;
- `mutation-report.json`;
- `maintenance-report.json`;
- `provenance.json`.

The reports must describe the same scale and the same ordered candidate set. The
comparison tool rejects duplicate scales, missing files, invalid JSON, scale
disagreement, and candidate-order disagreement.

## Running the comparison

After extracting the archived packets into `evidence/index-storage/100k` and
`evidence/index-storage/1m`, run:

```bash
node scripts/verify/compare-index-storage-evidence.mjs \
  --input evidence/index-storage/100k \
  --input evidence/index-storage/1m \
  --output evidence/index-storage/comparison
```

The command writes:

- `comparison.json` for machine-readable review and later checks;
- `comparison.md` for the storage ADR review.

A smoke-only invocation is supported for validating the tool, but it emits
`decision_ready=false`. The comparison becomes decision-ready only when both
replacement `100k` and `1m` packets are supplied.

## Reported metrics

For every scale and candidate the report includes:

- source and candidate load duration;
- relation size and exact entity/link cardinality;
- first execution and median execution of later repetitions for every read
  workload;
- shared read/hit blocks, temporary blocks, and distinct plan shapes;
- mutation execution, buffers, and maximum-per-plan-node WAL metrics;
- baseline, after-churn, and after-VACUUM relation size;
- aggregate `pg_stat_user_tables` estimates/counters;
- VACUUM duration;
- `1m / 100k` scale ratios when both packets are present.

The first repetition is separated from later repetitions only as a reproducible
first-run versus warm-run signal. It is not described as a guaranteed operating
system cold-cache measurement because the current harness does not reset the OS
page cache between repetitions.

Ordinary VACUUM may leave relation files unchanged or larger. The JSON output
therefore records a neutral `vacuum_size_delta`, not an assumed reclaimed-space
metric.

## Operational review

The generated comparison intentionally contains only reproducible evidence
metrics. Architecture and operations are evaluated separately in
[`storage-operational-review.md`](./storage-operational-review.md).

That review records:

- the genericity and schema-evolution requirements for canonical storage;
- the full schema identity required by entity and typed EAV field storage;
- index, migration, mutation, query-compilation, diagnostics, rebuild, and
  partitioning complexity for each candidate;
- the hot typed projection's status as a best-case baseline rather than an
  eligible canonical generic model;
- the additional mutation and operational burden typed EAV must justify with a
  decisive measured advantage;
- the production controls required if JSONB is selected.

A benchmark rerun changes the numeric evidence but does not change those
architectural findings unless candidate DDL or the accepted Index ownership
boundary changes.

## Decision boundary

The tool deliberately does not compute a winner or weighted score. The ADR must
combine the generated replacement comparison with the operational review and
still evaluate:

- acceptable latency, size, WAL, buffer, and maintenance trade-offs;
- planner stability at both scales;
- tenant, locale, complete schema identity, source-version, and atomic-link
  invariants;
- production index, partition, migration, rebuild, and diagnostics rules;
- the explicit rejection reason for each alternative.

The canonical proposed decision record is
[`DECISIONS/2026-07-24-index-storage-layout.md`](../../../DECISIONS/2026-07-24-index-storage-layout.md).
