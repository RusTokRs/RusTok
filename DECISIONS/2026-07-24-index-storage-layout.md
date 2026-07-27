# ADR: Index PostgreSQL storage model

- Status: **accepted**
- Decision date: **2026-07-27**
- Owner: **RusToK platform / Index module**
- Evidence commit: `eae5f74241e9431bffe2fd8c43cd046fc1c1f679`
- Comparison SHA-256: `7d10a3de9f62cf315d578794d1b69caa9a45d72847d1480cd24f9a9da4e9bbd8`
- Decision SHA-256: `ae77267776a38264c9432618459fb80559962842877596fc156ebd4e3a12e883`
- Packet contract: `v2`
- Result digest contract: `ordered_length_prefixed_json_v1`
- PostgreSQL image: `postgres:16`

## Context

The Index module evaluated JSONB, typed EAV, and hot projection storage using same-commit 100k and 1m PostgreSQL evidence. Candidate query results were checked against the normalized source oracle, and the comparison explicitly disabled automatic winner selection.

## Decision

Use **jsonb** as the PostgreSQL persistence model for the next Index storage milestone.

## Rationale

Select JSONB entity rows as the canonical generic Index storage. JSONB is the simplest candidate that satisfies the owner-published, schema-agnostic Index boundary without source-specific tables or base-table DDL for every new field. The replacement 1m packet shows a materially better overall operational profile than typed EAV: 4.25 GiB versus 8.14 GiB of schema storage, 127,965 ms versus 427,256 ms load time, 0.08 ms versus 152.45 ms warm keyset latency, 10.68 ms versus 100.72 ms warm exact-count latency, 149.59 ms versus 188.79 ms update latency, 1,029,145 versus 1,346,681 median maximum-node WAL bytes for the update workload, and 3,124 ms versus 16,490 ms VACUUM duration. JSONB also keeps one logical entity in one source-versioned row while preserving independent relational links. Hot projection is faster and smaller in several measurements, but it is not eligible as the canonical generic representation because it hard-codes entity-specific columns and rollout code.

## Storage and maintenance evidence

| Prototype | 100k schema | 1m schema | Growth | EAV fields 100k / 1m | Churn growth 100k / 1m | VACUUM 100k / 1m |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| jsonb | 449.72 MiB | 4.25 GiB | 9.67x | n/a / n/a | 1.57% / 0.19% | 943 ms / 3124 ms |
| typed_eav | 832.13 MiB | 8.14 GiB | 10.01x | 1,400,160 / 14,000,320 | 1.29% / 0.17% | 804 ms / 16490 ms |
| hot_projection | 361.54 MiB | 3.53 GiB | 9.99x | n/a / n/a | 1.37% / 0.17% | 672 ms / 2043 ms |

## Read/query evidence

| Prototype | Workload | Warm median 100k | Warm median 1m | Growth | Plan shapes 100k / 1m |
| --- | --- | ---: | ---: | ---: | ---: |
| jsonb | status_equality | 2.25 ms | 0.06 ms | 0.03x | 1 / 1 |
| jsonb | price_range_sort | 1.63 ms | 0.08 ms | 0.05x | 1 / 1 |
| jsonb | multi_value_tag | 1.54 ms | 11.61 ms | 7.56x | 1 / 1 |
| jsonb | two_hop_channel_filter | 32.02 ms | 255.55 ms | 7.98x | 1 / 1 |
| jsonb | keyset_page | 0.06 ms | 0.08 ms | 1.42x | 1 / 1 |
| jsonb | exact_count | 1.94 ms | 10.68 ms | 5.51x | 1 / 1 |
| typed_eav | status_equality | 0.45 ms | 0.48 ms | 1.08x | 1 / 1 |
| typed_eav | price_range_sort | 0.48 ms | 0.53 ms | 1.11x | 1 / 1 |
| typed_eav | multi_value_tag | 0.48 ms | 0.53 ms | 1.09x | 1 / 1 |
| typed_eav | two_hop_channel_filter | 31.81 ms | 251.46 ms | 7.90x | 1 / 1 |
| typed_eav | keyset_page | 18.54 ms | 152.45 ms | 8.22x | 1 / 1 |
| typed_eav | exact_count | 11.46 ms | 100.72 ms | 8.79x | 1 / 1 |
| hot_projection | status_equality | 0.07 ms | 0.08 ms | 1.06x | 1 / 1 |
| hot_projection | price_range_sort | 0.10 ms | 0.07 ms | 0.74x | 1 / 1 |
| hot_projection | multi_value_tag | 1.30 ms | 0.59 ms | 0.45x | 1 / 1 |
| hot_projection | two_hop_channel_filter | 26.65 ms | 231.83 ms | 8.70x | 1 / 1 |
| hot_projection | keyset_page | 0.08 ms | 0.08 ms | 0.97x | 1 / 1 |
| hot_projection | exact_count | 1.86 ms | 10.14 ms | 5.44x | 1 / 1 |

## Mutation and WAL evidence

| Prototype | Workload | Median execution 100k / 1m | Growth | Median WAL 100k / 1m | WAL growth |
| --- | --- | ---: | ---: | ---: | ---: |
| jsonb | update_product_batch | 42.59 ms / 149.59 ms | 3.51x | 1.02 MiB / 1005.02 KiB | 0.97x |
| jsonb | delete_product_batch | 21.69 ms / 132.95 ms | 6.13x | 158.20 KiB / 158.20 KiB | 1.00x |
| typed_eav | update_product_batch | 74.22 ms / 188.79 ms | 2.54x | 1.26 MiB / 1.28 MiB | 1.02x |
| typed_eav | delete_product_batch | 55.41 ms / 169.76 ms | 3.06x | 580.08 KiB / 580.08 KiB | 1.00x |
| hot_projection | update_product_batch | 36.48 ms / 142.32 ms | 3.90x | 815.22 KiB / 791.67 KiB | 0.97x |
| hot_projection | delete_product_batch | 19.52 ms / 133.10 ms | 6.82x | 158.20 KiB / 158.20 KiB | 1.00x |

## Rejected alternatives

### typed_eav

Reject typed EAV because the corrected replacement evidence does not provide a decisive benefit that offsets its higher physical and operational complexity. At 1m it uses 8.14 GiB versus JSONB at 4.25 GiB, loads in 427,256 ms versus 127,965 ms, expands each logical entity into an envelope plus many field rows, produces substantially worse keyset and exact-count latency, records higher update WAL, and requires 16,490 ms VACUUM versus 3,124 ms. Its faster equality and tag probes do not justify the additional joins, ordinals, deduplication, mutation fan-out, field-row diagnostics, and rebuild surface for the canonical engine.

### hot_projection

Reject hot projection as canonical storage despite its best measured size and latency. The prototype encodes Product, Variant, and SalesChannel as dedicated typed tables; every new owner-published entity or indexed field would require Index-core DDL, migrations, query code, and operational rollout. That violates the accepted generic Index ownership boundary and prevents ordinary module/schema registration from making a new entity queryable. A future optional derived hot projection or cache may be proposed only through a separate ADR with its own consistency, rebuild, and evidence contract.

## Operational trade-offs

The production model uses a constrained relational envelope plus validated JSONB payloads and independent relational links. Registry validation remains mandatory because PostgreSQL cannot enforce every dynamic field type inside JSONB. Every read, mutation, maintenance, rebuild, and diagnostic path must constrain tenant, module, entity, schema version, entity ID, and locale. Typed filtering and ordering use deterministic schema-managed expression indexes; a general GIN index is optional and may be created only for measured containment workloads. The index manager owns stable names, predicates, concurrent creation, readiness, reindexing, retirement, and per-schema diagnostics. Payload updates rewrite one entity row and can create WAL and tuple churn, so operators must track table/index bytes, WAL, dead tuples, autovacuum lag, and query-plan drift. The initial canonical tables remain unpartitioned because partitioning was not part of the M2 evidence; all primary and secondary keys remain tenant-leading so a later measured shadow migration may introduce tenant-hash partitioning without changing logical identity. Production health must not depend on VACUUM FULL or other exclusive rewrites.

## Migration strategy

M3 creates a generic entity table keyed by tenant, module, entity, schema version, entity ID, and locale, with monotonic source_version, schema fingerprint/version metadata, and a JSONB payload. A separate link table stores complete source and target identities, link name, and ordinal. Inbox deduplication, entity upsert/delete, and complete outgoing-link replacement execute in one transaction and reject stale source versions. Secondary indexes are derived from registry-declared filterable/sortable fields and built concurrently through an observable index-management job; no source-domain table or bespoke entity table is introduced. Initial population uses paginated IndexSource scan/load into shadow storage, followed by exact entity/link cardinality, digest, schema-fingerprint, and checkpoint parity checks. Cutover occurs only after the shadow store is query-equivalent and caught up through the mutation inbox. The archived comparison and accepted decision are retained before removing the typed EAV and hot benchmark prototypes.

## Rollback strategy

There is no production Index persistence to preserve before M3, so the first rollout is additive and remains disabled until shadow backfill and parity verification succeed. During rollout, source modules remain authoritative and the previous storage adapter/configuration and checkpoint are retained for a bounded verification window. A failed cutover disables the JSONB adapter, restores the prior routing/configuration, discards or quarantines the shadow tables, and rebuilds from owner-provided IndexSource streams; it never falls back to typed EAV or hot projection and never reads source-module tables directly. After the verification window, rollback continues to mean recreate a clean JSONB store from authoritative sources and replay the durable inbox from the last accepted checkpoint rather than attempting an in-place reverse migration.

## Evidence limitations

- The first repetition is only a first-run signal, not a guaranteed operating-system cold-cache measurement.
- The benchmark evidence does not replace production observability, migration rehearsal, or failure-mode testing.
- This ADR records a manual decision; the renderer does not infer or rank a winning prototype.

