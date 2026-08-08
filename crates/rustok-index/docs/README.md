# `rustok-index` Architecture Documentation

This directory contains the detailed technical architecture documentation for `rustok-index`, RusTok's cross-module relational Index Engine.

---

## Architectural Comparison Matrix

| Indexing Approach | Write Overhead | Cross-Module Filtering | Zero N+1 Queries | Consistency Model | Infrastructure Complexity |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Traditional SQL JOINs** | Low | Slow (Multi-table JOIN bottlenecks) | No (N+1 HTTP calls across microservices) | Immediate | Single DB |
| **EAV Tables (Magento / Legacy CMS)** | High DDL & Row Bloat | Complex self-JOINs & lock contention | No (N+1 HTTP calls across microservices) | Immediate | Single DB |
| **External Search Sync (Elasticsearch / Algolia)** | High DDL & Row Bloat | Fast | Yes | Eventual (Lag & drift risks) | Heavy JVM/Cloud cluster |
| **`rustok-index` (JSONB + Keyset Engine)** | **Low (Transactional Outbox Inbox)** | **Ultra-Fast (Derived B-Tree / GIN Indexes)** | **Yes (Single REPEATABLE READ query)** | **Immediate (Transactional Outbox)** | **Pure Rust + PostgreSQL (Zero external dependencies)** |

---

## Key Technical Specifications

1. **Schema-Agnostic PostgreSQL JSONB Storage**: Envelopes entity state into benchmarked `JSONB` structures while maintaining independent relational graphs (`index_links`).
2. **Derived Secondary Indexes**: Automatically creates typed PostgreSQL partial B-Tree expression indexes for scalar fields and GIN containment indexes for arrays.
3. **Derived and Sealed Cursor Boundaries**: Query keyset cursors are checksummed and scope-bound; owner source cursors use a separate authenticated, confidential, tenant/schema/source-bound codec plus a private server `SecretRef` keyring, sealed one-page service boundary, and bounded GraphQL transport. Drift discovery uses a separate exact-scope candidate contract, read-only PostgreSQL `txid` visibility fence, bounded two-phase keyset reader, double-observed owner/materialized confirmation, serializable idempotent finding persistence, authorization-gated lifecycle audit, durable targeted-repair reservations/receipts, concrete missing-entity and orphan-link repair paths through command-bound inbox identities, and an immutable authorization-gated recovery ledger for ambiguous `prepared` commands.
4. **Durable Rebuilds & Persisted Readiness**: Fences stale checkpoint writers with advisory locks, deduplicates mutations via `index_inbox`, logs consistency findings (`index_consistency_findings`), and fail-closes authoritative cutover unless the explicit tenant schema set matches active persisted `index_schemas` contracts exactly.

---

## Reference Documents

- [Current Implementation Plan — 2026-08-08](./implementation-plan-current-2026-08-08.md)
- [Historical Milestone Plan](./implementation-plan.md)
- [Source Module Integration Contract](./module-source-integration.md)
- [M5 Mutation Event Commit/Ack Contract](./m5-mutation-event-ack-contract.md)
- [M5 Exact Source Refresh Event Worker](./m5-source-refresh-event.md)
- [M5 Product Locale Refresh Owner Ledger](../../rustok-product/docs/index-locale-refresh-ledger.md)
- [M5 Product Refresh Canonical Writer](../../rustok-product/docs/index-refresh-canonical-writer.md)
- [M5 Product Refresh Durable Relay Step](../../rustok-product/docs/index-refresh-relay-step.md)
- [M5 Social Graph Production Mutation Route](./m5-social-graph-mutation-route.md)
- [M5/M6 Source Replay Contract](./m5-m6-source-replay-contract.md)
- [M6 Explicit Source Absence Watermark](./m6-explicit-source-absence-watermark.md)
- [M6 Confidential Source Continuation Codec](./m6-source-continuation-codec.md)
- [M6 Server-owned Source Continuation Keyring](./m6-source-continuation-server-keyring.md)
- [M6 Bounded Stale-entity and Orphan-link Candidates](./m6-bounded-drift-candidates.md)
- [M6 PostgreSQL Drift Candidate Reader](./m6-postgres-drift-candidate-reader.md)
- [M6 Drift Candidate Confirmation](./m6-drift-candidate-confirmation.md)
- [M6 Confirmed Candidate Finding Persistence](./m6-confirmed-candidate-finding-persistence.md)
- [M6 Drift Finding Lifecycle](./m6-drift-finding-lifecycle.md)
- [M6 Targeted Drift Repair](./m6-targeted-drift-repair.md)
- [M6 Concrete Missing-entity Repair](./m6-missing-entity-repair-composition.md)
- [M6 Concrete Orphan-link Repair](./m6-orphan-link-repair-composition.md)
- [M6 Prepared Repair Recovery](./m6-prepared-repair-recovery.md)
- [M6 Concrete Repair PostgreSQL Harness](./m6-repair-execution-postgres-harness.md)
- [M6 Concrete Repair Retained Evidence Admission](./m6-repair-retained-evidence-admission.md)
- [M6 Product Locale Absence PostgreSQL Harness](./m6-product-locale-absence-postgres-harness.md)
- [M6 GraphQL Exact-entity Diagnosis Transport](../../../apps/server/docs/index-drift-diagnosis-graphql-transport.md)
- [M6 One-page Missing-entity Diagnosis](../../../apps/server/docs/index-drift-source-page-diagnosis.md)
- [M6 Sealed Source-page GraphQL Transport](../../../apps/server/docs/index-drift-source-page-graphql-transport.md)
- [M6 Bounded Source-call Timeout](./m6-source-call-timeout.md)
- [M6 Bounded Replay Dry-run](./m6-bounded-replay-dry-run.md)
- [M6 Targeted Replay Mutation Application](./m6-targeted-replay-mutation-application.md)
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
- [M6 Bounded Drift Digest Producer](./m6-drift-digest-producer.md)
- [M6 Locale-Optional Drift-Finding Scope](./m6-drift-finding-locale-scope.md)
- [M6 PostgreSQL Drift Snapshot Reader](./m6-postgres-drift-snapshot-reader.md)
- [M6 Reconciliation Dead-letter Admission](./m6-reconciliation-dead-letter-admission.md)
- [M6 Reconciliation Dead-letter Inspection](./m6-reconciliation-dead-letter-inspection.md)
- [M6 Reconciliation Dead-letter Requeue](./m6-reconciliation-dead-letter-requeue.md)
- [M7 Canonical Product Graph Source](./m7-product-graph-source.md)
- [M7 Tenant Schema Readiness Gate](./m7-schema-readiness.md)
- [M7 Product-SalesChannel Relation Admission](./m7-product-sales-channel-relation-admission.md)
- [M7 Product-SalesChannel Owner Ledger](../../rustok-product/docs/index-sales-channel-relation-ledger.md)
- [M7 Product-SalesChannel Freshness Witness](../../rustok-product/docs/index-sales-channel-relation-freshness.md)
- [M7 Product-SalesChannel Cross-owner Resolver](./m7-product-sales-channel-resolver.md)
- [M7 Product Graph Projection Ledger](../../rustok-product/docs/index-graph-projection-ledger.md)
- [M7 Product Attribute Term Contract](./m7-product-attribute-term-contract.md)
- [M7 Product Storefront Index Parity Gate](./m7-product-storefront-parity-gate.md)
- [M7 Product Storefront Localized Query Architecture](./m7-product-storefront-localized-query-architecture.md)
- [M4 Source-owned Schema Registry](./m4-source-schema-registry.md)
- [M4 Single-current Schema Supersession](./m4-single-current-schema-supersession.md)
- [M4 Query Runtime Composition](./m4-query-runtime-composition.md)
- [M4 PostgreSQL Query Port Contract](./m4-postgres-query-port.md)
- [M4 Many-link Aggregate Ordering](./m4-many-link-aggregate-ordering.md)
- [M4 Decimal Aggregate Order Wire](./m4-decimal-aggregate-wire.md)
- [M4 Many-link Projection Contract](./m4-many-link-projection.md)
- [M2 Storage Benchmark Contract](./storage-benchmark.md)
- [M2 Replacement Evidence Runbook](./storage-evidence-runbook.md)
- [M3 Retained Partition Capture Runbook](./partition-full-capture.md)
