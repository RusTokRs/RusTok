# PostgreSQL storage benchmark for `rustok-index`

## Status

- Milestone: `M2 - PostgreSQL storage benchmark`
- Harness: implemented in `ops/benches/src/index_storage`
- Production migrations: intentionally absent
- Evidence runs: pending repository-owner execution
- Storage decision ADR: pending evidence

## Goal

Select the physical PostgreSQL representation for the generic Index Engine from
repeatable evidence rather than preference. The benchmark compares three models
while keeping the generated source dataset, entity identity, links, filters,
ordering, pagination, and PostgreSQL session constant.

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

## Workloads

Every candidate executes the same semantic workloads:

1. tenant/locale/status equality filter;
2. typed price range with deterministic ordering;
3. multi-value tag membership;
4. Product -> Variant -> SalesChannel two-hop filter;
5. compound keyset pagination by price and entity ID;
6. exact filtered count.

The runner records each SQL statement so evidence can be audited independently
of the summarized metrics.

## Evidence captured

For each candidate the report includes:

- source and prototype load duration;
- total schema relation size through `pg_total_relation_size`;
- PostgreSQL version and relevant planner settings;
- repeated `EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)` plans;
- planning and execution time;
- shared hit/read blocks;
- temporary read/write blocks;
- full JSON plan for later plan-shape analysis.

Vacuum impact and mutation write amplification require additional update/delete
workloads and remain open M2 tasks.

## Running

A dedicated PostgreSQL database is required because the harness drops and
recreates schemas prefixed with `idx_bench_`.

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_bench \
INDEX_BENCH_SCALE=smoke \
cargo run -p rustok-benchmarks --bin index-storage-benchmark --release
```

Evidence scales:

```bash
INDEX_BENCH_SCALE=100k cargo run -p rustok-benchmarks --bin index-storage-benchmark --release
INDEX_BENCH_SCALE=1m cargo run -p rustok-benchmarks --bin index-storage-benchmark --release
```

Optional settings:

- `INDEX_BENCH_LOCALES=en-US,ru-RU`
- `INDEX_BENCH_REPETITIONS=3`
- `INDEX_BENCH_OUTPUT=target/index-storage-benchmark/report.json`

## Decision rules

No candidate is selected from one latency number. The ADR must compare:

- p50/median execution across repeated plans;
- cold versus warm buffer behavior;
- ingestion duration and relation size;
- equality, range, multi-value, link, two-hop, sort, keyset, and count behavior;
- planner stability at both 100k and 1m Product-locale rows;
- write amplification under updates and deletes;
- vacuum/bloat behavior;
- operational complexity for schema evolution and dynamic fields;
- compatibility with tenant, locale, source-version, and atomic link invariants.

After the ADR is accepted, rejected prototype code and schemas must be deleted.
