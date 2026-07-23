# PostgreSQL storage benchmark for `rustok-index`

## Status

- Milestone: `M2 - PostgreSQL storage benchmark`
- Read harness: implemented in `ops/benches/src/index_storage`
- Mutation/WAL harness: implemented with transaction rollback isolation
- Persistent churn/VACUUM harness: implemented with committed cycles
- Production migrations: intentionally absent
- Evidence runs: pending repository-owner execution
- Storage decision ADR: pending evidence

## Goal

Select the physical PostgreSQL representation for the generic Index Engine from
repeatable evidence rather than preference. The benchmark compares three models
while keeping the generated source dataset, entity identity, links, filters,
ordering, pagination, mutation batch, churn cycle count, and PostgreSQL session
constant.

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
