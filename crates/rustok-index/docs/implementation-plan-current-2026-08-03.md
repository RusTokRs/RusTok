# Current `rustok-index` implementation plan — 2026-08-03

Status overlay rechecked against current `main` after Product locale owner ledger merge #3111 and
active branch `agent/index-m5-product-variant-refresh-ledger-20260806`.

When this dated overlay conflicts with the older canonical plan, this file is the current source of
truth. Historical architecture and milestone context remain in `implementation-plan.md`.

## Current cursor

`M6 - execute and admit concrete repair evidence`

Previous source cursor: `M6 - retain concrete repair execution evidence`.

The database-neutral stale/orphan candidate contract, PostgreSQL bounded reader, application
confirmation boundary, serializable finding persistence, fail-closed finding lifecycle commands,
generic targeted-repair orchestration, durable repair reservations/receipts, authorization-gated
prepared-command recovery, and concrete recovery-aware repair compositions for missing entities and
orphan links are source complete.

The environment-gated PostgreSQL repair execution packet is now
`source_ready_owner_execution_pending`. It uses real `IndexModule` migrations and production schema,
mutation, finding, repair, recovery, evidence, and owner adapters through these targets:

- `drift_repair_recovery_postgres_test`;
- `drift_repair_concrete_execution_postgres_test`.

The clean-commit retained-evidence admission boundary is
`source_complete_owner_execution_pending`. It locks one PostgreSQL metadata target, both scenario
commands, current source hashes, required case names, bounded database/toolchain metadata, complete
credential-redacted stdout/stderr, and an atomic terminal pass packet. The packet and logs remain
absent until the repository owner executes the capture runner.

An independent M5 slice registers Social Graph as the first exact production mutation route. It adds
one bounded authoritative replay source over `social_graph_relations`, reuses the existing Iggy
consumer source identity, and atomically materializes the immutable mutation event registry with the
staged source catalog. This does not advance or bypass the concrete-repair evidence cursor.

A second independent M5 slice adds exact source-refresh orchestration for thin owner change events.
One event identifies an exact `EntityKey`, owner event UUID, and positive minimum source version. The
worker resolves the immutable route and source, performs one bounded exact load, requires an upsert
or tombstone at or beyond the event fence, rebinds the canonical mutation to the broker event UUID,
durably applies it, and only then acknowledges.

A third independent M5 slice makes Product locale owner changes recoverable before the public wire
contract exists. Product lifecycle publication retains the exact root envelope UUID and writes
bounded append-only `(tenant, product, locale, source_version)` refresh rows in the same transaction.
Live rows use final trigger-owned Product revisions; removed locales and hard deletes use retained
tombstone revisions. A bounded Product-owned source exposes the ledger without importing Index.

A fourth independent M5 slice adds forward parent identity to ProductVariant tombstones and one
append-only ProductVariant refresh ledger. Product lifecycle publication now records exact live and
parent-aware retained variant identities in the same owner transaction as the root event and locale
ledger. Live rows use final trigger-owned variant revisions; deletes use retained tombstone revisions.
Historical parentless tombstones remain replayable but are excluded from incremental publication
because their Product causation cannot be reconstructed safely. Typed Product/ProductVariant events,
route registration, relay/consumer composition, and runtime evidence remain open. These source-only
slices do not advance or bypass the repair evidence cursor.

Concrete missing-entity repair:

- accepts only the exact confirmed missing-entity target through a pre-reservation target gate;
- brackets the exact materialized identity with two authoritative source/absence reads;
- requires an absence version strictly newer than the live indexed version;
- applies one typed delete through the established mutation inbox and schema validation;
- uses the durable repair command UUID as the mutation event and delivery identity;
- requires an exact tombstone at the admitted absence version before recording `Repaired`.

Concrete orphan-link repair:

- accepts only the exact confirmed orphan-link target through a pre-reservation target gate;
- revalidates source key/version, link name, ordinal, linked target, and target absence proof;
- brackets one repeatable-read materialized snapshot with stable source and target authority reads;
- delegates exact edge removal to a typed persistence owner rather than SQL in the repair owner;
- preserves source entity version, payload, and every unrelated link;
- binds the exact removal digest and inbox delivery to the durable repair command UUID;
- accepts an absent edge as convergence only with the exact command-bound applied delivery.

Both concrete paths:

- create one immutable revision-0 active recovery decision for each new reservation;
- require active recovery state before retry, owner mutation, and receipt completion;
- serialize pause/abandon against the owner call with the exact command advisory fence;
- fail legacy decision-less, paused, and abandoned prepared commands closed;
- remain unmounted from runtime extensions and public transports.

Public transport, automatic iteration, time-derived leases, and admitted retained execution results
remain open.

## Rechecked status

- M0 reset and M1 domain/application core: `complete`
- M2 JSONB storage decision and replacement benchmark packet: `complete`
- M3 production storage/evidence tooling: `source_complete_owner_execution_pending`
- M4 query planning/compiler/runtime and privacy shadow: `source_complete_owner_execution_pending`
- M5 inbox deduplication and monotonic source versions: `complete`
- M5 mutation event registry and commit-before-ack orchestration:
  `generic_source_complete_social_graph_route_runtime_execution_pending`
- M5 exact source-refresh event worker:
  `source_complete_owner_event_publication_and_runtime_wiring_pending`
- M5 Product locale append-only refresh owner ledger and bounded owner source:
  `owner_source_complete_wire_and_consumer_pending`
- M5 ProductVariant parent-aware tombstones, append-only refresh ledger, and bounded owner source:
  `owner_source_complete_wire_and_consumer_pending`
- M5 Social Graph bounded replay source and exact production event route:
  `source_complete_runtime_execution_pending`
- M5/M6 bounded source replay contract: `source_complete_owner_execution_pending`
- M6 replay jobs, checkpoints, multi-page execution, cancellation, retry/dead-letter, and generic
  host scheduling: `source_complete_owner_execution_pending`
- M6 bounded replay interruption, source timeout, and no-write dry-run: `source_complete`
- M6 reconciliation retry, dead-letter, recovery, and generic host scheduling: `source_complete_owner_execution_pending`
- M6 bounded drift-finding inspection and persistence:
  `source_complete_owner_execution_pending`
- M6 snapshot-pair digest producer and mismatch-only recorder delegation: `source_complete`
- M6 locale-optional persisted entity finding scope:
  `source_complete_owner_execution_pending`
- M6 missing-only entity candidate outcome: `source_complete`
- M6 bounded source-page missing-entity diagnosis: `source_complete`
- M6 source-version-fenced PostgreSQL drift snapshot reader:
  `source_complete_owner_execution_pending`
- M6 guarded exact-entity drift diagnosis operator and bounded GraphQL transport:
  `source_complete_owner_execution_pending`
- M6 bounded GraphQL sealed source-page diagnosis transport:
  `source_complete_owner_execution_pending`
- M6 explicit source absence watermark registry and Product locale provider:
  `source_complete_owner_execution_pending`
- M6 explicit source absence watermark registry, Product provider, and reader fence:
  `source_complete_owner_execution_pending`
- M6 Product locale absence PostgreSQL harness (`product_locale_absence_postgres`, real-migration Product locale-absence scenario):
  `source_ready_owner_execution_pending`
- M6 bounded stale-entity and orphan-link candidate contract: `source_complete`
- M6 PostgreSQL drift candidate reader: `source_complete`
- M6 bounded candidate confirmation and PostgreSQL materialized observer: `source_complete`
- M6 confirmed-candidate finding persistence: `source_complete`
- M6 drift finding lifecycle commands: `source_complete`
- M6 generic targeted drift repair boundary and durable receipt store: `source_complete`
- M6 concrete missing-entity evidence reader and idempotent mutation owner:
  `source_complete_recovery_aware_owner_execution_pending`
- M6 prepared repair pause/resume/abandon recovery policy:
  `source_complete_owner_execution_pending`
- M6 - add prepared repair recovery policy:
  `source_complete_recovery_policy_pending`
- M6 concrete orphan-link evidence reader and command-bound edge-removal owner:
  `source_complete_recovery_aware_owner_execution_pending`
- M6 concrete repair PostgreSQL execution harness:
  `source_ready_owner_execution_pending`
- M6 concrete repair retained evidence admission tooling:
  `source_complete_owner_execution_pending`
- M7 Product/ProductVariant/SalesChannel bounded replay graph:
  `source_complete_owner_execution_pending`

## M5 incremental ingestion

- [x] Add a source replay registry with bounded failure classification.
- [x] Add inbox deduplication and monotonic source versions.
- [x] Add a database-neutral mutation-source event registry and commit-before-ack orchestration.
- [x] Add exact one-key source-refresh orchestration with minimum owner revision fencing.
- [x] Register the Social Graph production event route and bounded replay source.
- [x] Reuse the concrete Social Graph Iggy consumer/acknowledger, bounded retry/backoff, DLQ and
      poison receipts, graceful shutdown, and lag/outcome metrics.
- [x] Materialize selected PostgreSQL sources and mutation event routes atomically.
- [x] Retain exact Product locale identities and owner revisions in an append-only transactional
      ledger causally linked to the existing Product lifecycle envelope.
- [x] Expose the Product locale ledger through one bounded tenant/sequence owner source.
- [x] Retain ProductVariant parent identity for new tombstones and append exact live/delete variant
      revisions to a Product-scoped owner ledger in the root Product transaction.
- [x] Expose the ProductVariant ledger through one bounded tenant/sequence owner source.
- [ ] Define reviewed Product locale and ProductVariant refresh event families and update committed
      release digests through the canonical generator.
- [ ] Relay Product locale and ProductVariant rows as typed write-once events with the retained
      `refresh_id` envelope identities.
- [ ] Register Product locale and ProductVariant production routes and concrete consumers.
- [ ] Add equivalent concrete consumer policy for every remaining selected owner route.
- [ ] Retain crash-between-commit-and-ack and redelivery evidence against the generic registered
      route boundary.

## M6 replay and scheduling

- [x] Add bounded source scan/targeted-load contracts and stable replay event identities.
- [x] Add durable replay jobs, leases, heartbeats, attempt fences, and checkpoint progression.
- [x] Add bounded multi-page replay, durable cancellation, and bounded multi-pass reconciliation.
- [x] Add source-call timeouts, no-write dry-run, and cooperative page interruption safe points.
- [x] Add bounded retry/dead-letter transitions, authorized requeue, and generic host scheduling.
- [ ] Bind interruption to active runner lease/cancellation state and already-pending futures.
- [ ] Retain multi-host scheduler, restart, graceful-shutdown, and command-transport evidence.
- [ ] Add locale/partition replay checkpoint dimensions.
- [ ] Add targeted, full, and shadow rebuild modes.

## M6 exact diagnosis and missing discovery

- [x] Add bounded drift-finding persistence and inspection.
- [x] Add bounded drift-finding inspection and persistence for already-computed digest mismatches. Retain `drift_finding_writer_postgres_test` evidence.
- [x] Add one exact snapshot-pair digest producer with deterministic SHA-256 digests.
- [x] Add the source-version-fenced PostgreSQL exact drift snapshot reader.
- [x] Extend persisted entity finding scope to locale-free (`drift_finding_locale_scope_postgres_test`, `drift_snapshot_reader_postgres_test`).
- [ ] Run and admit `drift_snapshot_reader_postgres_test` evidence.
- [ ] Add targeted repair with before/after admitted evidence.
- [x] Add explicit retained absence/tombstone watermark proof with canonical source-owner parity.
- [x] Register Product locale absence for Product schema versions 1 and 2.
- [x] Expose exact diagnosis through bounded GraphQL `diagnoseIndexEntity`.
- [x] Add the missing-only selector and one bounded internal source-page diagnosis runtime.
- [x] Keep the raw owner cursor server-internal and expose only authenticated encrypted continuation.
- [x] Compose the private server `SecretRef` keyring and sealed one-page service boundary.
- [x] Expose one bounded source-page GraphQL mutation over the sealed method only.
- [ ] Run and admit retained exact GraphQL, Product absence, PostgreSQL, continuation, and rotation
      evidence.

## M6 stale entity, orphan-link, lifecycle, and targeted repair

- [x] Add a database-neutral bounded candidate contract with one exact tenant/schema scope and page
      size at most 32.
- [x] Require fence and cursor together, immutable fence identity, advancing cursor, bounded pages,
      exact candidate scope, and strict deterministic order.
- [x] Expose only typed stale entity identity/version and typed orphan source/link/target identity.
- [x] Add `PostgresIndexDriftCandidateReader` over `index_entities` and `index_links`.
- [x] Run one read-only repeatable-read transaction per page and capture one scope-bound PostgreSQL
      transaction-snapshot fence.
- [x] Use only bounded `limit + 1` keyset SQL and one deterministic stale-to-orphan transition.
- [x] Add `IndexDriftCandidateConfirmer` over one candidate only.
- [x] Observe exact materialized state before and after provisional confirmation.
- [x] Confirm stale/orphan candidates only through stable authoritative source/delete/absence evidence.
- [x] Add the PostgreSQL exact materialized observer.
- [x] Derive deterministic finding identity and SHA-256 evidence from confirmed candidates.
- [x] Revalidate write-time entity/version/link/target state in one serializable transaction.
- [x] Create, refresh, reopen, or suppress through the established Index finding contract.
- [x] Add fail-closed resolve/ignore commands with authorization capability and immutable audit rows.
- [x] Add typed targeted-repair command, authorization capability, evidence and owner ports.
- [x] Verify typed repair targets against exact persisted check/finding/evidence commitments.
- [x] Add one active-command-per-finding serializable reservation and idempotent terminal receipt.
- [x] Preserve finding and lifecycle rows; successful repair does not silently resolve a finding.
- [x] Keep repair unmounted from server extensions, public transports, schedulers, and page loops.
- [x] Compose one concrete admitted evidence reader for confirmed missing-entity findings.
- [x] Compose one concrete idempotent missing-entity delete owner through `PostgresMutationStore`.
- [x] Bind missing-entity retry identity to the durable repair command UUID.
- [x] Add fail-closed prepared-command pause/resume/abandon recovery and lifecycle coordination.
- [x] Retain immutable command-scoped recovery decisions and require active state at owner/completion.
- [x] Compose one exact orphan-link evidence reader with stable source and target authority reads.
- [x] Compose one command-bound exact edge-removal persistence owner behind the recovery boundary.
- [x] Preserve source entity version/payload and unrelated links during orphan repair.
- [x] Require exact applied inbox proof before admitting an absent edge as convergence.
- [x] Add env-gated real-migration PostgreSQL targets for concrete repair crash, retry, recovery,
      commitment-change, and concurrency scenarios.
- [x] Lock a clean-commit capture contract with source hashes, bounded PostgreSQL/toolchain metadata,
      required case names, complete credential-redacted logs, and atomic admission verification.
- [ ] Run and admit the concrete repair PostgreSQL targets with retained database/version/result
      metadata.
- [ ] Add time-derived lease expiry only with retained owner-liveness and crash-window evidence.

## M7 production graph and cutover

- [x] Add Product, ProductVariant, and SalesChannel schemas and bounded current-state sources.
- [x] Add stable replay identities and retained deletes.
- [x] Add Product-to-ProductVariant graph materialization.
- [x] Add the generic exact-source refresh worker required by Product/ProductVariant notifications.
- [x] Add Product locale append-only owner refresh publication and bounded owner source.
- [x] Add ProductVariant parent-aware tombstones, append-only owner refresh publication, and bounded
      owner source for live and retained-delete identities.
- [ ] Add Product/ProductVariant typed events, production routes, relays, and concrete incremental
      consumers.
- [ ] Persist and enforce per-tenant schema readiness.
- [ ] Complete durable Product-to-SalesChannel relation semantics and retained evidence.
- [ ] Admit tombstone purge, freshness/outage/restart/backlog recovery, and delete/recreate evidence.
- [ ] Admit live PostgreSQL/reference query equivalence and one full partition packet.
- [ ] Keep authoritative consumer and production partition cutover forbidden until admission.

## Next implementation step

Execute and admit the source-ready concrete repair PostgreSQL packet before exposing repair through a
public command surface or automatic iterator.

The owner must run the locked capture command from a clean commit. The retained packet must include:

- PostgreSQL server version and database URL class without credentials;
- exact commit SHA and the metadata plus both scenario target command lines;
- migration up/down and repair/recovery trigger results;
- missing-entity and orphan-link post-owner crash/retry results;
- pause-before-owner and abandon-before-completion race results;
- command UUID reuse, stale revision, duplicate decision, and completion immutability results;
- changed source/link/target/absence commitment results;
- normal full-mutation versus exact edge-owner serialization results;
- complete credential-redacted stdout/stderr and final pass status.

Independent source-only work may continue on the Product/ProductVariant typed relay and remaining
M5/M7 owner routes, but public repair authorization transport, automatic iteration, time-derived
leases, and lifecycle auto-resolution remain blocked until admission.

## Owner verification for current source boundaries

```bash
node scripts/verify/verify-index-product-locale-refresh-ledger.mjs
node scripts/verify/verify-index-product-variant-refresh-ledger.mjs
cargo check -p rustok-product --all-targets

cargo test -p rustok-index source_refresh_event --lib -- --nocapture
node scripts/verify/verify-index-source-refresh-event.mjs

cargo test -p rustok-social-graph --features index-consumer index_source -- --nocapture
cargo test -p rustok-index source_factory -- --nocapture
node scripts/verify/verify-index-social-graph-mutation-route.mjs

RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  node scripts/evidence/capture-index-repair-postgres.mjs

node scripts/verify/verify-index-repair-retained-evidence.mjs
node scripts/verify/verify-index-repair-execution-postgres-harness.mjs
node scripts/verify/verify-index-prepared-repair-recovery.mjs
node scripts/verify/verify-index-missing-entity-repair-composition.mjs
node scripts/verify/verify-index-orphan-link-repair-composition.mjs
node scripts/verify/verify-index-targeted-drift-repair.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-social-graph --features index-consumer --all-targets
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, formatting, Cargo checks, migrations, PostgreSQL/SQLite/Iggy scenarios,
workflows, or CI were executed by the implementation agent.
