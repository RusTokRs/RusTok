# PostgreSQL storage benchmark for `rustok-index`

## Status

- Milestone: `M2 - PostgreSQL storage benchmark` (`complete`)
- Selected-layout read harness: JSONB-only in `ops/benches/src/index_storage`
- Selected-layout mutation/WAL harness: JSONB-only with transaction rollback isolation
- Selected-layout persistent churn/VACUUM harness: JSONB-only with committed cycles
- PostgreSQL session metadata contract: implemented across all three reports
- Smoke evidence automation: implemented in `.github/workflows/index-storage-smoke.yml`
- Production migrations: intentionally absent pending M3 implementation
- Smoke evidence: historical harness-sanity packet from Actions run `30041091121`
- 100k evidence: historical diagnostic packet from Actions run `30051321255`
- Replacement evidence: validated same-commit 100k and 1m packets from Actions run `30222913450` on `eae5f74241e9431bffe2fd8c43cd046fc1c1f679`
- 1m evidence: validated on `ubuntu-latest` after the fail-closed 35 GB free-disk check
- Storage decision ADR: Accepted; JSONB is the canonical generic entity storage model

## Goal

The completed M2 benchmark selected the physical PostgreSQL representation for
the generic Index Engine from repeatable evidence rather than preference. The
archived decision run compared three models while keeping the generated source
dataset, entity identity, links, filters, ordering, pagination, mutation batch,
churn cycle count, and PostgreSQL session constant.

After acceptance, the typed-EAV and hot-projection implementations were removed.
The remaining JSONB runner is a selected-layout regression harness. The archived
three-candidate packets and comparison remain the authoritative selection
evidence; new JSONB-only runs cannot replace or silently revise that decision.

## PostgreSQL session and metadata contract

Every executable uses a pool constrained to exactly one physical PostgreSQL
connection. Read, mutation, and maintenance evidence therefore run in three
separate sessions, with one reproducible session retained for the complete
lifetime of each report.

Before generated benchmark SQL executes, the shared connection setup pins:

- `standard_conforming_strings = on`;
- `TimeZone = 'UTC'`;
- `DateStyle = 'ISO, YMD'`;
- `extra_float_digits = 3`.

Each runner also disables JIT for the measured session. Other planner and memory
settings are not silently synthesized: they are observed from the active
connection and archived with the evidence.

Every `read-report.json`, `mutation-report.json`, and `maintenance-report.json`
contains an exact `database` object with:

- `version`;
- `server_version_num`;
- `shared_buffers`;
- `effective_cache_size`;
- `work_mem`;
- `random_page_cost`;
- `jit`;
- `standard_conforming_strings`;
- `timezone`;
- `date_style`;
- `extra_float_digits`.

The runner captures this object after session setup and re-reads it after all
workloads, churn, and maintenance operations. Any field drift fails the evidence
run before the report is serialized. The official packet preflight then requires
the exact field set in all three reports and exact equality between the read,
mutation, and maintenance sessions before the byte-preserved validator or
comparator core is imported.

For cross-scale comparison, the official comparator wrapper compares the ten
planner/session fields from `server_version_num` through `extra_float_digits`
between same-commit packets and records that methodology in the generated
comparison. Output produced by invoking the comparator core directly is
incomplete and cannot make an ADR decision-ready.

## Candidates

### JSONB entity rows

One row per tenant/schema/entity/locale with a JSONB payload. Candidate indexes
include a general `jsonb_path_ops` GIN index and typed expression indexes for hot
fields. Links are stored in a separate relational table. Reads, mutations, and
maintenance constrain module, entity, and schema version.

### Typed EAV rows

One identity row per entity and normalized field rows with separate boolean,
integer, numeric, text, UUID, and timestamp columns. Multi-value fields use an
ordinal. Every field row carries the complete module/entity/schema-version
identity, includes it in primary and lookup keys, and references the matching
entity envelope. Links are stored in the same independent relational shape used
by the other candidates.

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

Static verification additionally locks complete module/entity/schema-version
identity in JSONB/EAV entity maintenance and in typed EAV field keys, joins,
mutations, and maintenance paths.

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

Every report contains database/server metadata observed from its own active
PostgreSQL benchmark session and proven stable through the end of that report.
The official packet preflight requires the read, mutation, and maintenance
metadata objects to match exactly.

For each read candidate the report additionally includes:

- source and prototype load duration;
- total schema relation size through `pg_total_relation_size`;
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

All three executables must be run at replacement `100k` and `1m` from the same
commit before the storage ADR is accepted.

### CI smoke evidence

`.github/workflows/index-storage-smoke.yml` runs all three release executables
against PostgreSQL 16 with the deterministic `smoke` preset. It validates that
each report contains all three prototypes, writes a provenance manifest tied to
the commit and workflow run, and uploads the evidence packet for 90 days.

The workflow is path-scoped to Index, benchmark, verifier, and workflow changes
and can also be started manually. A successful artifact is inspected before the
canonical plan marks evidence complete.

The first inspected packet is Actions run `30041091121`, artifact
`index-storage-smoke-8efd318091098bb5bce0d5f83b8b51653dc4934c`. It contains
`read-report.json`, `mutation-report.json`, `maintenance-report.json`, and
`provenance.json` for PostgreSQL 16, three repetitions, and five churn cycles.
All candidates preserved 1,216 entities and 2,400 links, produced identical read
result digests, validated equal mutation effects, and preserved exact
cardinality after churn and VACUUM.

This smoke packet proved the original harness could execute coherently. It is now
historical harness-sanity evidence because later query and identity corrections
changed the SQL. Its latency, size, WAL, and VACUUM values must not select a
production candidate.

### Historical inspected 100k evidence

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
queries and are verifier-locked.

A later audit also found that typed EAV field rows omitted module and schema
version and that JSONB/EAV maintenance entity mutations relied on `entity_name`
alone. Those paths now use the complete identity and are verifier-locked. The
values above therefore remain pre-fix diagnostics; replacement same-commit
100k/1m packets supply the canonical comparison.

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
35,000,000,000 free bytes before the build.

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
- first-run versus warm buffer behavior;
- ingestion duration and relation size;
- equality, range, multi-value, link, two-hop, sort, keyset, and count behavior;
- planner stability at both replacement 100k and 1m Product-locale rows;
- update/delete latency, buffers, WAL records/FPI/bytes, and changed row count;
- committed churn, dead-tuple estimates, HOT updates, vacuum duration, and
  pre/post-VACUUM size behavior;
- operational complexity for schema evolution and dynamic fields;
- compatibility with tenant, locale, complete schema identity, source-version,
  and atomic link invariants.

After the ADR is accepted, rejected prototype code and schemas must be deleted.
