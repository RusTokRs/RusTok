# `rustok-index` Architecture Documentation

This directory contains the detailed technical architecture documentation for `rustok-index`, RusTok's cross-module relational Index Engine.

---

## Architectural Comparison Matrix

| Indexing Approach | Write Overhead | Cross-Module Filtering | Zero N+1 Queries | Consistency Model | Infrastructure Complexity |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Traditional SQL JOINs** | Low | Slow (Multi-table JOIN bottlenecks) | No (N+1 HTTP calls across microservices) | Immediate | Single DB |
| **EAV Tables (Magento / Legacy CMS)** | High DDL & Row Bloat | Complex self-JOINs & lock contention | No | Immediate | Single DB |
| **External Search Sync (Elasticsearch / Algolia)** | High (Outbox / Event Relay) | Fast | Yes | Eventual (Lag & drift risks) | Heavy JVM/Cloud cluster |
| **`rustok-index` (JSONB + Keyset Engine)** | **Low (Transactional Outbox Inbox)** | **Ultra-Fast (Derived B-Tree / GIN Indexes)** | **Yes (Single REPEATABLE READ query)** | **Immediate (Transactional Outbox)** | **Pure Rust + PostgreSQL (Zero external dependencies)** |

---

## Key Technical Specifications

1. **Schema-Agnostic PostgreSQL JSONB Storage**: Envelopes entity state into benchmarked `JSONB` structures while maintaining independent relational graphs (`index_links`).
2. **Derived Secondary Indexes**: Automatically creates typed PostgreSQL partial B-Tree expression indexes for scalar fields and GIN containment indexes for arrays.
3. **Checksummed Keyset Cursors**: Fast, deterministic keyset pagination (`CursorCodec`) and exact-count execution in single `REPEATABLE READ` snapshot transactions.
4. **Durable Rebuilds & Outbox Inbox**: Fences stale checkpoint writers with advisory locks, deduplicates mutations via `index_inbox`, and logs consistency findings (`index_consistency_findings`).

---

## Reference Documents

- [Live Implementation Plan](./implementation-plan.md)
- [M5/M6 Source Replay Contract](./m5-m6-source-replay-contract.md)
- [M6 Bounded Source-call Timeout](./m6-source-call-timeout.md)
- [M6 Bounded Replay Dry-run](./m6-bounded-replay-dry-run.md)
- [M6 Cooperative Replay-page Interruption](./m6-cooperative-page-interruption.md)
- [M6 Replay Retry Transition Store](./m6-replay-retry-transition-store.md)
- [M6 Replay Dead-letter Admission](./m6-replay-dead-letter-admission.md)
- [M6 Replay Job Leases](./m6-replay-job-leases.md)
- [M6 Bounded Multi-page Replay Runner](./m6-bounded-multipage-runner.md)
- [M6 Replay Runtime Host Composition](./m6-replay-runtime-composition.md)
- [M6 Reconciliation Retry Transition Store](./m6-reconciliation-retry-transition-store.md)
- [M6 Reconciliation Runner Retry Wiring](./m6-reconciliation-runner-retry-wiring.md)
- [M6 Reconciliation Host Scheduler](./m6-reconciliation-host-scheduler.md)
- [M6 Drift Finding Inspection](./m6-drift-finding-inspection.md)
- [M6 Drift Digest Finding Writer](./m6-drift-finding-writer.md)
- [M6 Reconciliation Dead-letter Admission](./m6-reconciliation-dead-letter-admission.md)
- [M6 Reconciliation Dead-letter Inspection](./m6-reconciliation-dead-letter-inspection.md)
- [M6 Reconciliation Dead-letter Requeue](./m6-reconciliation-dead-letter-requeue.md)
- [M4 Source-owned Schema Registry](./m4-source-schema-registry.md)
- [M4 Query Runtime Composition](./m4-query-runtime-composition.md)
- [M4 PostgreSQL Query Port Contract](./m4-postgres-query-port.md)
- [M4 Many-link Aggregate Ordering](./m4-many-link-aggregate-ordering.md)
- [M4 Decimal Aggregate Order Wire](./m4-decimal-aggregate-order-wire.md)
- [M4 Many-link Projection Contract](./m4-many-link-projection.md)
- [M2 Storage Benchmark Contract](./storage-benchmark.md)
- [M2 Replacement Evidence Runbook](./storage-evidence-runbook.md)
- [M3 Retained Partition Capture Runbook](./partition-full-capture.md)
