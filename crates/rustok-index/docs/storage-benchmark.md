# PostgreSQL storage benchmark for `rustok-index`

## Status

- Milestone: `M2 - PostgreSQL storage benchmark`
- Read harness: implemented in `ops/benches/src/index_storage`
- Mutation/WAL harness: implemented with transaction rollback isolation
- Persistent churn/VACUUM harness: implemented with committed cycles
- Smoke evidence automation: implemented in `.github/workflows/index-storage-smoke.yml`
- Production migrations: intentionally absent
- Smoke evidence: archived from Actions run `30041091121`
- 100k evidence: archived and inspected from Actions run `30051321255`
- 1m evidence: enabled on `INDEX_BENCH_LARGE_RUNNER` when configured, otherwise `ubuntu-latest`, with a fail-closed 35 GB free-disk check
- Storage decision ADR: Proposed; 100k evidence is populated, acceptance still waits on 1m and the cross-scale comparison

## Goal

Select the physical PostgreSQL representation for the generic Index Engine from
repeatable evidence rather than preference. The benchmark compares three models
while keeping the generated source dataset, entity identity, links, filters,
ordering, pagination, mutation batch, churn cycle count, and PostgreSQL session
constant.

Every executable uses a pool constrained to exactly one physical PostgreSQL
connection. This keeps session settings, temporary execution state, maintenance
statistics, and VACUUM sequencing on one reproducible session per report.

## Candidates

### JSONB entity rows

One row per tenant/schema/entity/locale with a JSONB payload. Candidate indexes
include a general `jsonb_path_ops` GIN index and typed expression indexes for hot
fields. Links are stored in a separate relational table.

### Typed EAV rows

One identity row per entity and normalized field rows with separate boolean,
integer, numeric, text, UUID, and timestamp columns. Multi-value fields use an
ordinal. Links are stored in the same independent relational shape used by the
other candidates.

### Hot typed projection

Dedicated typed Product, Variant, and SalesChannel tables provide the best-case
specialized baseline. Links are still separate so link traversal cost is not
hidden inside payload storage.

This candidate is a comparison baseline, not the presumed production design.

## Deterministic dataset

The source dataset is generated entirely by deterministic PostgreSQL
`generate_series` statements. Stable UUIDs are derived from named MD5 inputs;
no random generator or wall-clock value is used.

Scale presets are based on Product-locale rows:

| Scale | Tenants | Locales | Product-locale rows | Variants per product |
|---|---:|---:|---:|---:|
| `smoke` | 2 | 2 | 400 | 2 |
| `100k` | 10 | 2 | 100,000 | 2 |
| `1m` | 20 | 2 | 1,000,000 | 2 |

The total entity-row count is larger because Variant and SalesChannel rows are
also generated. Locale inputs are canonicalized through
`rustok_index::LocaleKey` before SQL is created.

Before timings are accepted, the runners verify:

- exact source entity/link cardinality;
- exact entity/link cardinality in every candidate;
- identical result-row counts and deterministic result digests for every read
  workload across all candidates;
- identical affected entity/link counts for mutation workloads;
- unchanged entity/link cardinality after every committed churn phase.

## Read workloads

Every candidate executes the same semantic reads:

1. tenant/locale/status equality filter;
2. typed price range with deterministic ordering;
3. multi-value tag membership;
4. Product -> Variant -> SalesChannel two-hop filter;
5. compound keyset pagination by price and entity ID;
6. exact filtered count.

The runner records each SQL statement so evidence can be audited independently
of the summarized metrics.

## Mutation workloads

A separate executable measures write amplification without contaminating the
read report. Every candidate receives the same deterministic tenant/locale batch:

1. update Product source version, price, and rating;
2. delete Product rows and their outgoing Product -> Variant links.

The validation execution checks affected entity and link counts. Every measured
execution then runs under its own PostgreSQL transaction and is rolled back after
`EXPLAIN ANALYZE`, so repetitions and later candidates start from the same state.
The report stores full plans and maximum per-plan-node WAL records, full-page
images, and WAL bytes. These maxima are deliberately named as node maxima; the
full plan remains authoritative.

## Persistent churn and maintenance

A third executable measures committed maintenance behavior. For every candidate,
each cycle performs:

1. a committed Product batch update;
2. deletion of a deterministic tail Product batch and its outgoing links;
3. reinsertion of the deleted Product representation and links from the immutable
   source dataset.

The runner records three snapshots: baseline, after all churn cycles, and after
`VACUUM (ANALYZE)`. Each snapshot contains total schema bytes, exact entity/link
cardinality, and per-table `pg_stat_user_tables` estimates/counters for live and
dead tuples, inserts, updates, deletes, HOT updates, vacuum/autovacuum, and
analyze/autoanalyze. VACUUM duration is recorded separately.

`n_live_tup` and `n_dead_tup` are PostgreSQL estimates rather than exact tuple
counts. Exact logical cardinality is therefore checked independently. Ordinary
VACUUM may reclaim reusable space without shrinking relation files; unchanged
schema bytes after VACUUM are valid evidence rather than a harness failure.

## Evidence captured

For each read candidate the report includes:

- source and prototype load duration;
- total schema relation size through `pg_total_relation_size`;
- PostgreSQL version and relevant planner settings;
- repeated `EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)` plans;
- planning and execution time;
- shared hit/read blocks;
- temporary read/write blocks;
- workload result rows and digests;
- full JSON plan for later plan-shape analysis.

The mutation report additionally includes affected entity/link counts and
maximum observed node-level WAL records, FPI, and bytes.

The maintenance report includes baseline/after-churn/after-VACUUM size,
cardinality and table-stat snapshots plus VACUUM duration. It does not run
`VACUUM FULL`, because production maintenance should not depend on an exclusive
rewrite to remain healthy.

## Running

A dedicated PostgreSQL database is required because the harness drops and
recreates schemas prefixed with `idx_bench_`.

Read/query evidence:

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_bench \
INDEX_BENCH_SCALE=smoke \
cargo run -p rustok-benchmarks --bin index-storage-benchmark --release
```

Mutation/WAL evidence:

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_bench \
INDEX_BENCH_SCALE=smoke \
cargo run -p rustok-benchmarks --bin index-storage-mutation-benchmark --release
```

Persistent churn/VACUUM evidence:

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_bench \
INDEX_BENCH_SCALE=smoke \
INDEX_BENCH_CHURN_CYCLES=5 \
cargo run -p rustok-benchmarks --bin index-storage-maintenance-benchmark --release
```

All three executables must be run at `smoke`, `100k`, and `1m` before the storage
ADR is accepted.

### CI smoke evidence

`.github/workflows/index-storage-smoke.yml` runs all three release executables
against PostgreSQL 16 with the deterministic `smoke` preset. It validates that
each report contains all three prototypes, writes a provenance manifest tied to
the commit and workflow run, and uploads the evidence packet for 90 days.

The workflow is path-scoped to Index, benchmark, verifier, and workflow changes
and can also be started manually. A successful artifact is inspected before the
canonical plan marks smoke evidence complete.

The first inspected packet is Actions run `30041091121`, artifact
`index-storage-smoke-8efd318091098bb5bce0d5f83b8b51653dc4934c`. It contains
`read-report.json`, `mutation-report.json`, `maintenance-report.json`, and
`provenance.json` for PostgreSQL 16, three repetitions, and five churn cycles.
All candidates preserved 1,216 entities and 2,400 links, produced identical read
result digests, validated equal mutation effects, and preserved exact
cardinality after churn and VACUUM.

This smoke packet proves harness sanity only. Its small-scale latency, size, WAL,
and VACUUM values must not select a production candidate. The inspected 100k
packet establishes the first scale baseline; the 1m packet and cross-scale
comparison remain required before a production model is selected.

### Inspected 100k scale evidence

Actions run `30051321255` archived artifact
`index-storage-100k-84a11b147689b226ca161f5a0287990c1e8489d4` for
PostgreSQL 16, three repetitions, and five committed churn cycles. Provenance
records PR merge commit `84a11b147689b226ca161f5a0287990c1e8489d4`.
The packet contains the three JSON reports plus before/after runner resource
snapshots.

The validated dataset contains 100,000 Product-locale rows, 300,080 total entity
rows, and 600,000 links. Every candidate preserved exact cardinality, produced
identical result rows and digests for all six read workloads, affected the same
1,000 Product entities and 2,000 outgoing links in mutation validation, and
returned to exact cardinality after five churn cycles and `VACUUM (ANALYZE)`.
Every read and mutation workload retained one plan shape across its three
repetitions.

| Candidate | Load | Baseline size | Churn growth | Dead tuples after churn | VACUUM |
|---|---:|---:|---:|---:|---:|
| JSONB entity rows | 9.499 s | 385.58 MiB | 6.80 MiB (1.76%) | 20,000 | 800 ms |
| Typed EAV | 17.441 s | 687.23 MiB | 10.97 MiB (1.60%) | 69,934 | 921 ms |
| Hot typed projection | 6.132 s | 295.56 MiB | 4.61 MiB (1.56%) | 20,000 | 728 ms |

Warm-median read execution in milliseconds:

| Candidate | Status equality | Price range | Multi-value tag | Two-hop channel | Keyset page | Exact count |
|---|---:|---:|---:|---:|---:|---:|
| JSONB entity rows | 0.222 | 0.105 | 1.895 | 11,515.678 | 0.563 | 1.483 |
| Typed EAV | 7.074 | 6.102 | 4.742 | 14,989.380 | 20.814 | 4.074 |
| Hot typed projection | 0.073 | 0.071 | 1.394 | 10,305.135 | 0.032 | 0.456 |

The original two-hop workload was pathological for every candidate at this
scale: it used roughly 1.65-2.66 million shared-hit blocks and took 10-15 seconds
even though no shared-read or temporary blocks were recorded. EXPLAIN showed that
the query omitted the known `target_entity = 'variant'` and
`target_entity = 'sales_channel'` discriminators, preventing full use of
`link_target_lookup`. Those predicates are now part of all three candidate SQL
queries and are verifier-locked. The values above remain pre-fix diagnostics; a
same-commit 100k/1m rerun supplies the canonical cross-scale two-hop evidence.

Median mutation execution and maximum-node WAL bytes:

| Candidate | Update 1,000 Products | Update WAL | Delete 1,000 Products + 2,000 links | Delete WAL |
|---|---:|---:|---:|---:|
| JSONB entity rows | 51.060 ms | 1,054,238 B | 27.165 ms | 162,000 B |
| Typed EAV | 62.207 ms | 1,238,933 B | 46.305 ms | 594,000 B |
| Hot typed projection | 43.672 ms | 834,784 B | 24.683 ms | 162,000 B |

Ordinary VACUUM reduced estimated dead tuples to zero for every candidate but did
not shrink relation files; after-VACUUM size deltas were small positive values,
which is valid under the benchmark's neutral size-delta rule.

The inspected run failed closed before `1m` because repository variable
`INDEX_BENCH_LARGE_RUNNER` was not configured. Its 100k resource snapshots showed
93,030,404,096 free root-filesystem bytes before evidence and 88,893,792,256 after.
The scale workflow now prefers the configured runner when present and otherwise
uses `ubuntu-latest`; the reusable job still rejects any runner with less than
35,000,000,000 free bytes before the build. The 1m result remains pending, so the
storage ADR remains Proposed and M3 remains blocked.

Optional settings:

- `INDEX_BENCH_LOCALES=en-US,ru-RU`
- `INDEX_BENCH_REPETITIONS=3`
- `INDEX_BENCH_CHURN_CYCLES=5`
- `INDEX_BENCH_OUTPUT=target/index-storage-benchmark/report.json`
- `INDEX_BENCH_MUTATION_OUTPUT=target/index-storage-benchmark/mutation-report.json`
- `INDEX_BENCH_MAINTENANCE_OUTPUT=target/index-storage-benchmark/maintenance-report.json`

## Decision rules

No candidate is selected from one latency number. The ADR must compare:

- p50/median execution across repeated plans;
- cold versus warm buffer behavior;
- ingestion duration and relation size;
- equality, range, multi-value, link, two-hop, sort, keyset, and count behavior;
- planner stability at both 100k and 1m Product-locale rows;
- update/delete latency, buffers, WAL records/FPI/bytes, and changed row count;
- committed churn, dead-tuple estimates, HOT updates, vacuum duration, and
  pre/post-VACUUM size behavior;
- operational complexity for schema evolution and dynamic fields;
- compatibility with tenant, locale, source-version, and atomic link invariants.

After the ADR is accepted, rejected prototype code and schemas must be deleted.
