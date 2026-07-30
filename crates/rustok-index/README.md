# rustok-index — Cross-Module Relational Index Engine

`rustok-index` is RusTok's high-performance, cross-module relational Index Engine. It allows domain modules (Product, Content, Flex, Commerce, etc.) to publish generic entity schemas, mutations, and links, which `rustok-index` materializes into optimized PostgreSQL storage to execute complex filtering, projection, sorting, counting, and keyset pagination **without runtime HTTP fan-out or N+1 queries across domain boundaries**.

---

## Architectural Comparison Matrix

| Indexing Approach | Write Overhead | Cross-Module Filtering | Zero N+1 Queries | Consistency Model | Infrastructure Complexity |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Traditional SQL JOINs** | Low | Slow (Multi-table JOIN bottlenecks) | No (N+1 HTTP calls across microservices) | Immediate | Single DB |
| **EAV Tables (Magento / Legacy CMS)** | High DDL & Row Bloat | Complex self-JOINs & lock contention | No | Immediate | Single DB |
| **External Search Sync (Elasticsearch / Algolia)** | High (Outbox / Event Relay) | Fast | Yes | Eventual (Lag & drift risks) | Heavy JVM/Cloud cluster |
| **`rustok-index` (JSONB + Keyset Engine)** | **Low (Transactional Outbox Inbox)** | **Ultra-Fast (Derived B-Tree / GIN Indexes)** | **Yes (Single REPEATABLE READ query)** | **Immediate (Transactional Outbox)** | **Pure Rust + PostgreSQL (Zero external dependencies)** |

---

## Core Responsibilities

- **Generic Schema & Link Registries**: Owns versioned schema contracts (`index_schemas`), JSONB entity envelopes (`index_entities`), and relational link graphs (`index_links`).
- **Incremental Ingestion & Rebuilds**: Deduplicates mutations transactionally via `index_inbox` and executes checkpoint-fenced rebuild jobs (`index_jobs`).
- **PostgreSQL Secondary Index Management**: Dynamically derives typed partial B-Tree expression indexes for scalar fields and JSONB containment GIN indexes for array fields.
- **Deterministic Keyset Pagination**: Encodes checksummed, tenant-bound, order-bound `CursorCodec` cursors for fast, reproducible pagination.
- **Drift Diagnostics & Reconciliation**: Continuously logs and repairs inconsistency findings (`index_consistency_findings`).

---

## Storage Layout Benchmark (M2 Evidence)

During Milestone M2, `rustok-index` evaluated three storage candidates under empirical read, mutation, and maintenance workloads:
1. **JSONB Entity Envelope** (Selected & Accepted via ADR `2026-07-24`)
2. **Normalized Typed EAV** (Rejected — severe DDL bloat & lock contention)
3. **Specialized Hot Projections** (Rejected — excessive write amplification)

Empirical evidence confirmed JSONB achieved superior write throughput, minimal WAL amplification, deterministic query planning, and sub-millisecond keyset reads.

---

## Architecture Boundaries

- **No Domain Dependencies**: `rustok-index` core does NOT depend on `rustok-product`, `rustok-content`, or any domain crate.
- **No Direct Table Access**: Source modules convert state into generic `IndexRecord` and `IndexMutation` payloads; `rustok-index` never reads source tables directly.
- **Separation from Search**: `rustok-search` owns full-text relevance, fuzzy matching, typos, and search UX, consuming `rustok-index` for structured data access.

---

## Documentation & ADRs

- [Module Architecture Guide](./docs/README.md)
- [Index Engine Rewrite ADR](../../DECISIONS/2026-07-23-index-engine-rewrite.md)
- [Accepted Storage Layout ADR](../../DECISIONS/2026-07-24-index-storage-layout.md)
- [M5/M6 Source Replay Contract](./docs/m5-m6-source-replay-contract.md)
- [M4 Query Port Contract](./docs/m4-postgres-query-port.md)
