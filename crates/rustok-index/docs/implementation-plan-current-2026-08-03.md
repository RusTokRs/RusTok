# Current `rustok-index` implementation plan — 2026-08-03

Status overlay for `implementation-plan.md` audited through
`main@2c24dbf93cfa1ecb436755c7986232f05e4ddab5`.

When the older canonical plan's current-state bullets conflict with this dated overlay, this
overlay is the rechecked source of truth. Historical architecture, ownership, and milestone
details remain in `implementation-plan.md`.

## Current cursor

`M6 - locale-complete persisted findings and authoritative snapshot capture`

The database-neutral snapshot-pair digest producer is source complete and delegates unequal
locale-bearing entity digests to the existing finding writer. A production snapshot reader,
locale-free persisted entity scope, lifecycle commands, targeted repair, transports, and retained
evidence remain open.

## Rechecked status

- M0 reset and M1 domain/application core: `complete`
- M2 JSONB storage decision and replacement benchmark packet: `complete`
- M3 production storage/evidence tooling: `source_complete_owner_execution_pending`
- M4 query planning/compiler/runtime and privacy shadow: `source_complete_owner_execution_pending`
- M5 inbox deduplication and monotonic source versions: `complete`
- M5 mutation event registry and commit-before-ack orchestration:
  `source_complete_broker_wiring_pending`
- M5/M6 bounded source replay contract: `source_complete_owner_execution_pending`
- M6 one-page replay and durable checkpoint progression: `source_complete`
- M6 replay jobs, attempt fencing, multi-page execution, cancellation, and reconciliation:
  `source_complete_owner_execution_pending`
- M6 replay operator composition and authorization: `source_complete_owner_execution_pending`
- M6 bounded replay interruption, source timeout, and no-write dry-run:
  `source_complete_owner_execution_pending`
- M6 reconciliation retry, dead-letter, recovery, and generic host scheduling:
  `source_complete_owner_execution_pending`
- M6 bounded drift-finding inspection and persistence:
  `source_complete_owner_execution_pending`
- M6 snapshot-pair digest producer and mismatch-only recorder delegation:
  `source_complete_snapshot_reader_pending`
- M7 Product/ProductVariant/SalesChannel bounded replay graph:
  `source_complete_owner_execution_pending`

## M5 incremental ingestion

- [x] Add a source replay registry with bounded failure classification.
- [x] Add inbox deduplication and monotonic source versions.
- [x] Add a database-neutral mutation-source event registry and commit-before-ack orchestration.
- [ ] Register production owner event routes and compose a concrete broker consumer/acknowledger.
- [ ] Add batch transactions, retry classification, bounded backoff, dead-letter state, and lag
      metrics around production event routes.
- [ ] Retain crash-between-commit-and-ack and redelivery evidence.

## M6 rebuild, reconciliation, and repair

- [x] Add bounded source scan/targeted-load contracts and stable replay event identities.
- [x] Add durable replay jobs, leases, heartbeats, attempt fences, and checkpoint progression.
- [x] Add bounded multi-page replay, durable cancellation, and bounded multi-pass reconciliation.
- [x] Add production source-call timeouts, bounded no-write dry-run, and cooperative page
      interruption safe points.
- [ ] Bind interruption to active runner lease/cancellation state and already-pending futures.
- [ ] Add targeted, full, and shadow rebuild modes.
- [x] Add bounded retry/backoff transitions, failed-scope dead-letter admission/inspection,
      authorized requeue, and generic host scheduling ownership.
- [ ] Retain multi-host scheduler, retry/dead-letter, restart, graceful-shutdown, and command-
      transport evidence.
- [ ] Add locale/partition replay checkpoint dimensions.
- [x] Add bounded drift-finding inspection and persistence for already-computed digest mismatches.
- [x] Add a real-migration PostgreSQL harness for deterministic finding-key serialization and
      create/refresh/reopen/suppression lifecycle preservation.
- [ ] Run and admit `drift_finding_writer_postgres_test` evidence.
- [x] Add a database-neutral producer for one exact source/materialized snapshot pair with one
      bounded consistency token, exact scope revalidation, typed-state validation, deterministic
      SHA-256 production, and mismatch-only recorder delegation.
- [x] Adapt locale-bearing producer mismatches to `PostgresIndexDriftFindingWriter` with bounded
      retryable/permanent failure classification.
- [ ] Extend persisted entity finding scope to locale-free `EntityKey` values without changing
      existing locale-bearing finding identities.
- [ ] Add one production snapshot reader under a defined PostgreSQL snapshot or owner watermark.
- [ ] Add missing/stale entity and orphan-link diagnosis without unbounded ID collection.
- [ ] Add resolve/ignore lifecycle commands with actor/reason audit and fail-closed authorization.
- [ ] Add targeted repair with before/after admitted evidence.

## M7 production graph and cutover

- [x] Add Product, ProductVariant, and SalesChannel schemas and bounded current-state sources.
- [x] Add stable Product/ProductVariant/SalesChannel replay identities and retained deletes.
- [x] Add Product-to-ProductVariant graph materialization.
- [ ] Add production owner event routes and concrete incremental consumer wiring.
- [ ] Persist and enforce per-tenant schema readiness.
- [ ] Complete durable Product-to-SalesChannel relation semantics and retained evidence.
- [ ] Admit tombstone purge, freshness/outage/restart/backlog recovery, and delete/recreate evidence.
- [ ] Admit live PostgreSQL/reference query equivalence and one full partition packet.
- [ ] Keep authoritative consumer and production partition cutover forbidden until admission.

## Next implementation step

Make persisted entity finding scope locale-optional while preserving the existing finding-key bytes
for every locale-bearing scope. Add a distinct deterministic no-locale key component, update writer
and inspector validation, and retain SQLite/static coverage. Only then compose the first production
snapshot reader so generic no-locale schemas cannot fail after an otherwise valid comparison.

## Owner verification for this slice

```bash
cargo test -p rustok-index drift_digest -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-drift-digest-producer.mjs

RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test drift_finding_writer_postgres_test \
  -- --nocapture --test-threads=1

node scripts/verify/verify-index-drift-finding-postgres-harness.mjs
git diff --check
```

No tests, verifiers, formatting, Cargo checks, PostgreSQL runs, workflows, or CI were executed by
the implementation agent.
