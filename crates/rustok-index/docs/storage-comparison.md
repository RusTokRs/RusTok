# Index storage evidence comparison

## Purpose

This document defines the deterministic comparison step between the archived
`100k` and `1m` PostgreSQL evidence packets. It does not select a storage model;
the accepted decision remains an explicit ADR after both scale packets have been
inspected.

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
`100k` and `1m` packets are supplied.

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

## Decision boundary

The tool deliberately does not compute a winner or weighted score. The ADR must
still evaluate:

- operational complexity and schema evolution;
- index and migration management;
- acceptable latency, size, WAL, and maintenance trade-offs;
- tenant, locale, source-version, and atomic-link invariants;
- the explicit rejection reason for each alternative.

The canonical proposed decision record is
[`DECISIONS/2026-07-24-index-storage-layout.md`](../../../DECISIONS/2026-07-24-index-storage-layout.md).
