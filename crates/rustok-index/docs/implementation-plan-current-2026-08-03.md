# Current `rustok-index` implementation plan — 2026-08-03

Status overlay for `implementation-plan.md` rechecked through
`main@dd9020b8d259d9466423c47bd92b2c93c6c39225` and active PR #2986.

The forty-seven commits after this branch merge base cover Commerce diagnostics, Forum GraphQL/admin
work, Pages/Page Builder routes and delivery, event settings, and supporting server configuration.
The latest delta adds bounded Pages historical-route import. These commits do not modify
`crates/rustok-index`, Product Index composition, Index GraphQL transports, Index diagnosis/page
composition, or Index guards changed by this branch.

When this dated overlay conflicts with the older canonical plan, this file is the current source of
truth. Historical architecture and milestone context remain in `implementation-plan.md`.

## Current cursor

`M6 - bound stale Index-only entity and orphan-link diagnosis`

Exact drift diagnosis, Product locale absence proof, missing-only page classification, confidential
continuation, private server keyring composition, sealed one-page service execution, and bounded
GraphQL transport are source complete.

`diagnoseIndexSourcePage` now derives tenant and actor from authenticated context, authorizes before
schema/limit/token parsing, accepts one exact schema plus an optional opaque continuation, delegates
exactly once to `diagnose_source_page_sealed`, and returns only bounded current-page counters,
finding receipts, completion state, and the sealed token.

Execution evidence, stale Index-only discovery, orphan-link diagnosis, finding lifecycle commands,
and repair remain open.

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
- M6 snapshot-pair digest producer and mismatch-only recorder delegation: `source_complete`
- M6 missing-only entity candidate outcome: `source_complete_owner_execution_pending`
- M6 locale-optional persisted entity finding scope:
  `source_complete_owner_execution_pending`
- M6 source-version-fenced PostgreSQL drift snapshot reader:
  `source_complete_owner_execution_pending`
- M6 guarded exact-entity drift diagnosis operator:
  `source_complete_owner_execution_pending`
- M6 bounded GraphQL exact-entity diagnosis transport:
  `source_complete_owner_execution_pending`
- M6 bounded source-page missing-entity diagnosis:
  `source_complete_owner_execution_pending`
- M6 authenticated and confidential source continuation codec:
  `source_complete_owner_execution_pending`
- M6 server-owned source continuation keyring and sealed page boundary:
  `source_complete_owner_execution_pending`
- M6 bounded GraphQL sealed source-page diagnosis transport:
  `source_complete_owner_execution_pending`
- M6 explicit source absence watermark registry, Product provider, and reader fence:
  `source_complete_owner_execution_pending`
- M6 Product locale absence PostgreSQL harness:
  `source_ready_owner_execution_pending`
- M7 Product/ProductVariant/SalesChannel bounded replay graph:
  `source_complete_owner_execution_pending`

## M5 incremental ingestion

- [x] Add a source replay registry with bounded failure classification.
- [x] Add inbox deduplication and monotonic source versions.
- [x] Add a database-neutral mutation-source event registry and commit-before-ack orchestration.
- [ ] Register production owner event routes and compose a concrete broker consumer/acknowledger.
- [ ] Add batch transactions, bounded backoff, dead-letter state, and lag metrics around production
      event routes.
- [ ] Retain crash-between-commit-and-ack and redelivery evidence.

## M6 rebuild, reconciliation, diagnosis, and repair

- [x] Add bounded source scan/targeted-load contracts and stable replay event identities.
- [x] Add durable replay jobs, leases, heartbeats, attempt fences, and checkpoint progression.
- [x] Add bounded multi-page replay, durable cancellation, and bounded multi-pass reconciliation.
- [x] Add source-call timeouts, no-write dry-run, and cooperative page interruption safe points.
- [ ] Bind interruption to active runner lease/cancellation state and already-pending futures.
- [ ] Add targeted, full, and shadow rebuild modes.
- [x] Add bounded retry/dead-letter transitions, authorized requeue, and generic host scheduling.
- [ ] Retain multi-host scheduler, restart, graceful-shutdown, and command-transport evidence.
- [ ] Add locale/partition replay checkpoint dimensions.
- [x] Add bounded drift-finding persistence and inspection.
- [x] Add real-migration finding writer and locale-scope harnesses.
- [ ] Run and admit retained finding writer and locale-scope evidence.
- [x] Add one exact snapshot-pair digest producer with deterministic SHA-256 digests and
      mismatch-only recorder delegation.
- [x] Add the source-version-fenced PostgreSQL drift snapshot reader over one read-only
      repeatable-read materialized snapshot.
- [x] Compose exact diagnosis behind request-bound `modules:manage` authority.
- [x] Add exact retained absence/tombstone watermark proof with canonical replay-source owner parity.
- [x] Register Product locale absence for Product schema versions 1 and 2.
- [x] Add the source-ready Product locale absence and concurrent translation-change scenarios.
- [ ] Run and admit `product_locale_absence_postgres` evidence.
- [x] Expose exact diagnosis through bounded GraphQL `diagnoseIndexEntity`.
- [ ] Retain exact GraphQL authorization, malformed-input ordering, result, receipt, and dependency
      execution evidence.
- [x] Add a missing-only selector that records only source `Upsert` plus materialized `Missing`.
- [x] Add one internal source-page diagnosis runtime with page size at most 32, retained-delete
      skipping, sequential missing-only diagnosis, and no scheduler or repair capability.
- [x] Keep the raw owner cursor server-internal.
- [x] Add `IndexSourceContinuationCodec` with AES-256-GCM, fresh nonce, exact tenant/schema/source
      binding, authenticated lifetime, clock-skew bound, and active plus retained rotation keys.
- [x] Reject tampering, unsupported version, scope mismatch, expiry, oversized input, invalid claims,
      and unavailable key material before returning raw cursor state.
- [x] Compose a private server-owned keyring from bounded `SecretRef` values.
- [x] Bound keyring JSON to 16 KiB, key IDs to 64 bytes, references to 256 bytes, key count to 16,
      lifetime to 900 seconds, and decoded secret material to exactly 32 bytes.
- [x] Add `diagnose_source_page_sealed`, opening before scan-request construction and sealing before
      service return.
- [x] Return only counters, bounded receipts, completion, and one opaque token from the sealed
      outcome.
- [x] Add one bounded source-page transport over the sealed method only.
- [x] Derive transport tenant/actor from authenticated context and authorize before module/entity/
      version, limit, or token parsing.
- [x] Keep caller-selected tenant, actor, source identity, raw cursor, entity-ID list, batch,
      checkpoint, scheduler, lifecycle, and repair fields out of the transport.
- [x] Delegate exactly once to `diagnose_source_page_sealed` and expose only bounded counters,
      finding receipts, completion, and the opaque token.
- [ ] Retain authorization, secret resolution, rotation, expiry, sealed-result, and dependency-error
      execution evidence.
- [ ] Add bounded stale Index-only entity diagnosis without unbounded ID collection.
- [ ] Add bounded orphan-link diagnosis without exposing raw graph payloads.
- [ ] Add resolve/ignore lifecycle commands with actor/reason audit and fail-closed authorization.
- [ ] Add targeted repair with before/after admitted evidence.

## M7 production graph and cutover

- [x] Add Product, ProductVariant, and SalesChannel schemas and bounded current-state sources.
- [x] Add stable replay identities and retained deletes.
- [x] Add Product-to-ProductVariant graph materialization.
- [ ] Add production owner event routes and concrete incremental consumer wiring.
- [ ] Persist and enforce per-tenant schema readiness.
- [ ] Complete durable Product-to-SalesChannel relation semantics and retained evidence.
- [ ] Admit tombstone purge, freshness/outage/restart/backlog recovery, and delete/recreate evidence.
- [ ] Admit live PostgreSQL/reference query equivalence and one full partition packet.
- [ ] Keep authoritative consumer and production partition cutover forbidden until admission.

## Next implementation step

Add a database-neutral bounded candidate contract for stale Index-only entities and orphan links.

The next slice must not start with an unbounded table scan or collect arbitrary IDs in memory. It
must define:

- one exact tenant and schema scope;
- a bounded page limit and opaque server-owned continuation;
- stable ordering and snapshot/fence semantics;
- separate typed outcomes for stale entity and orphan link candidates;
- no raw indexed record, owner record, link payload, SQL, or database cause in public outcomes;
- no finding lifecycle mutation or repair authority;
- authorization before untrusted scope/cursor parsing for any future transport.

The first implementation should remain internal and database-neutral until the PostgreSQL candidate
reader and bounded evidence contract are separately reviewed. Do not add a scheduler, cross-page
accumulator, automatic resolution, or repair in that slice.

## Owner verification for the completed sealed transport slice

```bash
cargo test -p rustok-index source_continuation -- --nocapture
cargo test -p rustok-index drift_digest -- --nocapture
cargo test -p rustok-index source_absence -- --nocapture
cargo test -p rustok-distribution product_index --features mod-product -- --nocapture
cargo test -p rustok-server index_source_continuation_runtime -- --nocapture
cargo test -p rustok-server index_drift_diagnosis -- --nocapture
cargo test -p rustok-server index_drift_source_page_diagnosis -- --nocapture
cargo test -p rustok-server index_replay_runtime_composition -- --nocapture

RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-distribution \
  --features mod-product \
  --test product_locale_absence_postgres \
  -- --nocapture --test-threads=1

node scripts/verify/verify-index-source-continuation.mjs
node scripts/verify/verify-index-source-continuation-server.mjs
node scripts/verify/verify-index-drift-diagnosis-graphql-transport.mjs
node scripts/verify/verify-index-drift-source-page-diagnosis.mjs
node scripts/verify/verify-index-drift-source-page-graphql-transport.mjs
node scripts/verify/verify-index-product-absence-postgres-harness.mjs
node scripts/verify/verify-index-source-absence-watermark.mjs
node scripts/verify/verify-index-server-reconciliation-guard.mjs
node scripts/verify/verify-index-drift-snapshot-reader.mjs
node scripts/verify/verify-index-drift-digest-producer.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-server --all-targets --features mod-product
cargo check -p rustok-distribution --all-targets --features mod-product
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, verifiers, formatting, Cargo checks, PostgreSQL, cryptographic integration, or GraphQL runs,
workflows, or CI were executed by the implementation agent.
